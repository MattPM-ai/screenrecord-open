"use client";

import { useEffect, useState, useRef } from "react";
import {
  getHealth,
  getServerInfo,
  startServer,
  stopServer,
  getCurrentStatus,
  getDailyMetrics,
  getAppUsageBreakdown,
  getEventsByDateRange,
  type CurrentStatus,
  type DailyMetrics,
  type AppUsage,
  type TimelineSegment,
} from "@/lib/activitywatchClient";
import {
  generateTimelineSegments,
  applyCategoriesToApps,
  getDefaultCategories,
} from "@/lib/activityProcessor";
import { Button, cn } from "@repo/ui";
import { Play, Pause, Loader, Camera, Settings as SettingsIcon } from "lucide-react";
import { message } from '@tauri-apps/plugin-dialog';
import { CurrentStatusCard } from "./components/CurrentStatusCard";
import { DailyMetricsBar } from "./components/DailyMetricsBar";
import { TimelineVisualization } from "./components/TimelineVisualization";
import { AppUsageList } from "./components/AppUsageList";
import { DateSelector } from "./components/DateSelector";
import { SegmentDetailPanel } from "./components/SegmentDetailPanel";
// PrivacyNotice removed - not needed for fully local app
import { RecordingViewer } from "./components/RecordingViewer";
import { CollectorStatus } from "./components/CollectorStatus";
import { getCurrentWindow } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import {
  getCollectorStatus,
  SyncStatistics,
} from "@/lib/collectorClient";
import {
  startRecording,
  stopRecording,
  getRecordingConfig,
} from "@/lib/recordingClient";
import { ServiceStartupScreen } from "./components/ServiceStartupScreen";

// Dashboard settings type
type DashboardSettings = {
  auto_refresh_enabled: boolean;
  refresh_interval_seconds: number;
  refresh_on_focus: boolean;
};

// Default dashboard settings
const DEFAULT_DASHBOARD_SETTINGS: DashboardSettings = {
  auto_refresh_enabled: true,
  refresh_interval_seconds: 10,
  refresh_on_focus: true,
};

type ServerStatus =
  | "unknown"
  | "healthy"
  | "stopped"
  | "starting"
  | "initializing";

export default function Home() {
  // Service startup state
  const [servicesReady, setServicesReady] = useState<boolean>(false);
  const [serviceStartupError, setServiceStartupError] = useState<string | null>(null);

  // Server state
  const [status, setStatus] = useState<ServerStatus>("unknown");
  const [baseUrl, setBaseUrl] = useState<string>("");
  const [error, setError] = useState<string>("");
  const isStartingRef = useRef<boolean>(false);

  // Dashboard state
  const [selectedDate, setSelectedDate] = useState<Date>(new Date());
  const [currentStatus, setCurrentStatus] = useState<CurrentStatus | null>(null);
  const [dailyMetrics, setDailyMetrics] = useState<DailyMetrics | null>(null);
  const [timelineSegments, setTimelineSegments] = useState<TimelineSegment[]>([]);
  const [appUsage, setAppUsage] = useState<AppUsage[]>([]);
  const [appCategories, setAppCategories] = useState<Map<string, "productive" | "neutral" | "unproductive">>(new Map());
  const [loadingTimeline, setLoadingTimeline] = useState<boolean>(false);
  const [selectedSegment, setSelectedSegment] = useState<TimelineSegment | null>(null);
  const [hoveredSegment, setHoveredSegment] = useState<TimelineSegment | null>(null);

  // Privacy notice removed - not needed for fully local app

  // Recording viewer state
  const [showRecordingViewer, setShowRecordingViewer] = useState<boolean>(false);

  // Collector state
  const [collectorStatus, setCollectorStatus] = useState<SyncStatistics | null>(null);

  // Dashboard refresh state
  const [dashboardSettings, setDashboardSettings] = useState<DashboardSettings>(DEFAULT_DASHBOARD_SETTINGS);
  const [isRefreshing, setIsRefreshing] = useState<boolean>(false);
  const [lastRefreshTime, setLastRefreshTime] = useState<Date | null>(null);
  const lastFocusRefresh = useRef<number>(0);

  // Data loading error state
  const [dataError, setDataError] = useState<string | null>(null);

  // Server status refresh
  const refreshStatus = async () => {
    try {
      const h = await getHealth();
      if (h.ok) {
        setStatus("healthy");
        setError("");
        isStartingRef.current = false;
        const info = await getServerInfo();
        if (info) {
          setBaseUrl(info.base_url);
        }
      } else {
        setStatus((prevStatus) => {
          if (isStartingRef.current) {
            if (prevStatus === "initializing") {
              return "starting";
            }
            return "starting";
          } else {
            return "stopped";
          }
        });
        if (!isStartingRef.current) {
          setBaseUrl("");
          setError(h.details || "");
        }
      }
    } catch (e) {
      setStatus((prevStatus) => {
        if (isStartingRef.current) {
          if (prevStatus === "initializing") {
            return "starting";
          }
          return "starting";
        } else {
          return "stopped";
        }
      });
      if (!isStartingRef.current) {
        setBaseUrl("");
        setError(String((e as any)?.message || e));
      }
    }
  };

  // Refresh current status
  const refreshCurrentStatus = async () => {
    if (status !== "healthy") return;

    try {
      const statusData = await getCurrentStatus();
      setCurrentStatus(statusData);
    } catch (e) {
      console.error("Failed to fetch current status:", e);
    }
  };

  // Refresh all data (server status + current status + daily data)
  const refreshAll = async () => {
    setIsRefreshing(true);
    try {
      // Refresh server status
      await refreshStatus();

      // If server is healthy, refresh current status and daily data
      if (status === "healthy") {
        await Promise.all([
          refreshCurrentStatus(),
          refreshDailyData(selectedDate, false)
        ]);
      }

      setLastRefreshTime(new Date());
    } catch (e) {
      console.error("Failed to refresh:", e);
    } finally {
      setIsRefreshing(false);
    }
  };

  // Refresh daily data (metrics, timeline, app usage)
  const refreshDailyData = async (date: Date, silent: boolean = false) => {
    if (status !== "healthy") return;

    if (!silent) {
      setLoadingTimeline(true);
    }

    try {
      // Format date for API calls
      const dateStr = date.toISOString().split('T')[0] || ""; // YYYY-MM-DD
      const startTime = new Date(date);
      startTime.setHours(0, 0, 0, 0);
      const endTime = new Date(date);
      endTime.setHours(23, 59, 59, 999);

      // Fetch metrics, events, and app usage in parallel
      const [metrics, events, apps] = await Promise.all([
        getDailyMetrics(dateStr),
        getEventsByDateRange(startTime.toISOString(), endTime.toISOString()),
        getAppUsageBreakdown(startTime.toISOString(), endTime.toISOString()),
      ]);

      // Process data
      setDailyMetrics(metrics);

      // Generate timeline segments from events
      const segments = generateTimelineSegments(events.window_events, events.afk_events);
      setTimelineSegments(segments);

      // Apply categories to apps
      const categorizedApps = applyCategoriesToApps(apps, appCategories);
      setAppUsage(categorizedApps);

      // Clear any previous errors on successful data load
      setDataError(null);

    } catch (e) {
      const errorMessage = e instanceof Error ? e.message : String(e);
      console.error("Failed to fetch daily data:", errorMessage);
      setDataError(`Unable to load activity data: ${errorMessage}`);
      // Reset state on error
      setDailyMetrics(null);
      setTimelineSegments([]);
      setAppUsage([]);
    } finally {
      if (!silent) {
        setLoadingTimeline(false);
      }
    }
  };

  // Load app categories from storage
  const loadAppCategories = () => {
    try {
      const stored = localStorage.getItem("app_categories");
      if (stored) {
        const parsed = JSON.parse(stored);
        setAppCategories(new Map(Object.entries(parsed)));
      } else {
        // Initialize with defaults
        setAppCategories(getDefaultCategories());
      }
    } catch (e) {
      console.error("Failed to load app categories:", e);
      setAppCategories(getDefaultCategories());
    }
  };

  // Save app category
  const saveAppCategory = (appName: string, category: "productive" | "neutral" | "unproductive") => {
    const newCategories = new Map(appCategories);
    newCategories.set(appName, category);
    setAppCategories(newCategories);

    // Save to localStorage
    try {
      const obj = Object.fromEntries(newCategories.entries());
      localStorage.setItem("app_categories", JSON.stringify(obj));

      // Re-apply categories to current app usage
      const categorizedApps = applyCategoriesToApps(appUsage, newCategories);
      setAppUsage(categorizedApps);
    } catch (e) {
      console.error("Failed to save app category:", e);
    }
  };

  // Load dashboard settings from storage
  const loadDashboardSettings = () => {
    try {
      const stored = localStorage.getItem("dashboard_settings");
      if (stored) {
        const parsed = JSON.parse(stored) as DashboardSettings;
        setDashboardSettings(parsed);
      } else {
        setDashboardSettings(DEFAULT_DASHBOARD_SETTINGS);
      }
    } catch (e) {
      console.error("Failed to load dashboard settings:", e);
      setDashboardSettings(DEFAULT_DASHBOARD_SETTINGS);
    }
  };

  // Save dashboard settings to storage
  const saveDashboardSettings = (settings: DashboardSettings) => {
    try {
      setDashboardSettings(settings);
      localStorage.setItem("dashboard_settings", JSON.stringify(settings));
    } catch (e) {
      console.error("Failed to save dashboard settings:", e);
    }
  };

  // Privacy notice removed - not needed for fully local app

  /**
   * Open settings in a new window
   * Prevents multiple settings windows by checking if one already exists
   */
  const openSettingsWindow = async () => {
    try {
      // Check if settings window already exists
      const existingWindow = await WebviewWindow.getByLabel("settings");
      if (existingWindow) {
        // Focus existing window
        await existingWindow.setFocus();
        return;
      }

      // Create new settings window
      const settingsWindow = new WebviewWindow("settings", {
        url: "/settings",
        title: "Settings",
        width: 640,
        height: 700,
        minWidth: 500,
        minHeight: 400,
        resizable: true,
        center: true,
      });

      // Handle window creation errors
      settingsWindow.once("tauri://error", (e) => {
        console.error("Failed to create settings window:", e);
      });
    } catch (error) {
      console.error("Error opening settings window:", error);
    }
  };


  // Server status polling (every 2 seconds)
  useEffect(() => {
    refreshStatus();
    const interval = setInterval(refreshStatus, 2000);
    return () => clearInterval(interval);
  }, []);

  // Current status polling (every 2 seconds when server is healthy)
  useEffect(() => {
    if (status === "healthy") {
      refreshCurrentStatus();
      const interval = setInterval(refreshCurrentStatus, 2000);
      return () => clearInterval(interval);
    }
  }, [status]);

  // Daily data refresh when date changes or server becomes healthy
  useEffect(() => {
    if (status === "healthy") {
      refreshDailyData(selectedDate);
    }
  }, [status, selectedDate]);

  // Load app categories, dashboard settings, and check privacy notice on mount
  useEffect(() => {
    loadAppCategories();
    loadDashboardSettings();
    // Privacy notice check removed - not needed for fully local app
  }, []);


  // Close splash screen after minimum display time (1.2 seconds)
  useEffect(() => {
    const MINIMUM_SPLASH_TIME_MS = 1200;
    const FADE_DURATION_MS = 300;
    const startTime = Date.now();

    const closeSplash = async () => {
      const splashElement = document.getElementById('splash-screen');
      if (!splashElement) {
        console.warn('Splash screen element not found');
        return;
      }

      const elapsed = Date.now() - startTime;
      const remaining = MINIMUM_SPLASH_TIME_MS - elapsed;

      if (remaining > 0) {
        // Wait for the remaining time before closing
        await new Promise(resolve => setTimeout(resolve, remaining));
      }

      // Add hidden class to trigger fade out
      splashElement.classList.add('splash-hidden');

      // Remove element from DOM after fade completes
      setTimeout(() => {
        splashElement.remove();
      }, FADE_DURATION_MS);
    };

    closeSplash();
  }, []);

  // Collector status polling (every 5 seconds)
  useEffect(() => {
    const fetchCollectorStatus = async () => {
      try {
        const stats = await getCollectorStatus();
        setCollectorStatus(stats);
      } catch (err) {
        // Collector not initialized or error, set to null
        setCollectorStatus(null);
      }
    };

    // Initial fetch
    fetchCollectorStatus();

    // Poll every 5 seconds
    const interval = setInterval(fetchCollectorStatus, 5000);
    return () => clearInterval(interval);
  }, []);

  // Auto-refresh interval (when enabled)
  useEffect(() => {
    if (!dashboardSettings.auto_refresh_enabled || status !== "healthy") {
      return;
    }

    const interval = setInterval(() => {
      // Silent refresh - don't show loading indicator
      refreshDailyData(selectedDate, true);
    }, dashboardSettings.refresh_interval_seconds * 1000);

    return () => clearInterval(interval);
  }, [dashboardSettings.auto_refresh_enabled, dashboardSettings.refresh_interval_seconds, status, selectedDate]);

  // Refresh on window focus (when enabled)
  useEffect(() => {
    if (!dashboardSettings.refresh_on_focus || status !== "healthy") {
      return;
    }

    let unlisten: (() => void) | undefined;

    const setupFocusListener = async () => {
      const appWindow = getCurrentWindow();
      unlisten = await appWindow.onFocusChanged(({ payload: focused }) => {
        if (focused) {
          // Debounce focus refreshes
          const now = Date.now();
          const FOCUS_REFRESH_DEBOUNCE_MS = 2000;
          if (now - lastFocusRefresh.current > FOCUS_REFRESH_DEBOUNCE_MS) {
            refreshDailyData(selectedDate, true);
            lastFocusRefresh.current = now;
          }
        }
      });
    };

    setupFocusListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [dashboardSettings.refresh_on_focus, status, selectedDate]);

  // Listen for storage events from settings window
  useEffect(() => {
    const handleStorageChange = (event: StorageEvent) => {
      // Reload dashboard settings when changed in settings window
      if (event.key === "dashboard_settings" && event.newValue) {
        try {
          const newSettings = JSON.parse(event.newValue) as DashboardSettings;
          setDashboardSettings(newSettings);
        } catch (e) {
          console.error("Failed to parse dashboard settings:", e);
        }
      }
      
      // Refresh collector status when collector config is saved
      if (event.key === "collector_config_saved") {
        const fetchCollectorStatus = async () => {
          try {
            const stats = await getCollectorStatus();
            setCollectorStatus(stats);
          } catch (err) {
            setCollectorStatus(null);
          }
        };
        fetchCollectorStatus();
      }
    };

    window.addEventListener("storage", handleStorageChange);
    return () => window.removeEventListener("storage", handleStorageChange);
  }, []);

  const handleServerStart = async () => {
    try {
      isStartingRef.current = true;
      setStatus("initializing");
      setError("");
      const info = await startServer();
      setBaseUrl(info.base_url);
      await refreshStatus();
      
      // Start recording if enabled in config
      try {
        const recordingConfig = await getRecordingConfig();
        if (recordingConfig.enabled) {
          await startRecording();
        }
      } catch (recordingError) {
        // Log recording errors but don't block server operation
        console.warn("Failed to start recording:", recordingError);
      }
    } catch (e) {
      isStartingRef.current = false;
      setStatus("stopped");
      message(String((e as any)?.message || e), {
        title: 'Unable to start AW-Server',
        kind: 'error'
      });
    }
  };

  const handleServerStop = async () => {
    try {
      // Stop recording first (gracefully handle if not recording)
      try {
        await stopRecording();
      } catch (recordingError) {
        // Log but don't block - may not be recording
        console.warn("Failed to stop recording:", recordingError);
      }
      
      await stopServer();
      isStartingRef.current = false;
      setStatus("stopped");
      setBaseUrl("");
      setError("");
      // Reset dashboard state
      setCurrentStatus(null);
      setDailyMetrics(null);
      setTimelineSegments([]);
      setAppUsage([]);
      setSelectedSegment(null);
    } catch (e) {
      message(String((e as any)?.message || e), {
        title: 'Unable to stop AW-Server',
        kind: 'error'
      });
    }
  };

  // Show service startup screen if services aren't ready yet
  if (!servicesReady) {
    return (
      <ServiceStartupScreen
        onReady={() => {
          setServicesReady(true);
        }}
        onError={(error) => {
          setServiceStartupError(error);
          // Still allow the app to continue even if some services fail
          setServicesReady(true);
        }}
      />
    );
  }

  return (
    <main className="flex h-screen overflow-hidden">
      {/* Left Column - Server Status (fixed 300px) */}
      <div className="w-[300px] min-w-[300px] h-screen overflow-y-auto flex flex-col">
        {error && error !== "Server offline" ? (
          <p className="text-red-500 text-sm mb-2">Error: {error}</p>
        ) : null}
        <div className={cn("h-48 max-h-48 relative w-full  bg-linear-180 via-white via-80% to-white-600 transition-colors duration-200",
          status === "healthy" ?
            "from-green-500" :
            status === "stopped" ?
              "from-red-500" :
              "from-orange-500")}
        >
          <div className="h-1/2 rounded-b-lg relative">
            <button
              onClick={status === "healthy" ? handleServerStop : handleServerStart}
              disabled={status !== "healthy" && status !== "stopped"}
              className={`h-32 aspect-square rounded-full absolute bottom-0 left-0 right-0 mx-auto translate-y-1/2 z-10 drop-shadow-xl hover:shadow-inner transition-colors duration-100 bg-primary cursor-pointer`}
            >
              {status === "healthy" ? (
                <Pause className="size-11 text-white absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2" />
              ) : status === "stopped" ? (
                <Play className="size-11 text-white absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2" />
              ) : (
                <Loader className="size-11 text-white absolute top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 animate-spin" />
              )}
            </button>
          </div>
          <div className="h-1/2 absolute bottom-0 left-0 right-0 bg-white rounded-t-lg z-0" />
        </div>
        <div className="flex flex-col gap-2 mb-3 p-4 ">
            {/* Daily Metrics */}
            <DailyMetricsBar
              metrics={dailyMetrics}
            />
        </div>
        <div className="absolute bottom-3 left-4 flex items-center gap-2">
          <CollectorStatus statistics={collectorStatus} />
          <button
            onClick={openSettingsWindow}
            className="flex items-center gap-1 p-1.5 bg-gray-100 rounded-full cursor-pointer hover:bg-gray-200 transition-colors duration-200"
            title="Settings"
          >
            <SettingsIcon className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Right Column - Dashboard (fills remaining space) */}
      <div className="flex-1 h-screen overflow-y-auto relative p-4 bg-gray-50 border-l border-gray-200 min-w-[450px]">
        {/* Data Error Banner */}
        {dataError && (
          <div className="w-full bg-red-50 border border-red-300 rounded-lg p-4 mb-4">
            <div className="flex items-start justify-between">
              <div className="flex-1">
                <h3 className="text-red-900 font-semibold mb-1 flex items-center gap-2">
                  <span className="text-xl">⚠️</span>
                  Unable to Load Activity Data
                </h3>
                <p className="text-red-700 text-sm mb-3">{dataError}</p>
                <div className="flex gap-2">
                  <Button
                    onClick={async () => {
                      await handleServerStop();
                      setTimeout(() => handleServerStart(), 1000);
                    }}
                    variant="outline"
                    className="text-sm"
                  >
                    Restart Server
                  </Button>
                  <Button
                    onClick={() => setDataError(null)}
                    variant="outline"
                    className="text-sm"
                  >
                    Dismiss
                  </Button>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* Dashboard Content (when server is healthy) */}
        {status === "healthy" ? (
          <div className="space-y-6">
            {/* Dashboard Header with Date Selector and View Screenshots */}
            <div className="flex items-center justify-between flex-wrap gap-4">
              {/* <Button
                onClick={() => setShowScreenshotViewer(true)}
                variant="outline"
              >
                <Camera className="w-4 h-4 mr-2" />
                View Screenshots
              </Button> */}
            </div>

            {/* Current Status Card
            <CurrentStatusCard
              status={currentStatus}
              isOnline={status === "healthy"}
            /> */}

            {/* Timeline */}
            <div className="flex flex-col md:flex-row gap-4">
              <DateSelector
                selectedDate={selectedDate}
                onDateChange={setSelectedDate}
              />
              <div className="flex-1">
                {loadingTimeline ? (
                  <div className="bg-white rounded-lg p-8 shadow-md text-center h-full flex flex-col items-center justify-center">
                    <Loader className="w-8 h-8 animate-spin text-blue-600 mx-auto mb-2" />
                    <p className="text-gray-600">Loading timeline data...</p>
                  </div>
                ) : (
                  <TimelineVisualization
                    segments={timelineSegments}
                    selectedDate={selectedDate}
                    onSegmentClick={setSelectedSegment}
                    onSegmentHover={setHoveredSegment}
                  />
                )}
              </div>
            </div>


            {/* App Usage */}
            <AppUsageList
              apps={appUsage}
              onCategoryChange={saveAppCategory}
            />
          </div>
        ) : (
          /* Placeholder when server is not healthy */
          <div className="flex flex-col items-center justify-center h-full text-center">
            <div className="text-gray-300 mb-4">
              <svg className="w-24 h-24 mx-auto" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
              </svg>
            </div>
            <h2 className="text-xl font-semibold text-gray-600 mb-2">Server Not Running</h2>
            <p className="text-gray-500 max-w-md">
              Start the server from the panel on the left to view your activity dashboard.
            </p>
          </div>
        )}

        {/* Segment Detail Panel (overlay within dashboard) */}
        {selectedSegment && (
          <SegmentDetailPanel
            segment={selectedSegment}
            onClose={() => setSelectedSegment(null)}
          />
        )}

        {/* Recording Viewer (overlay within dashboard) */}
        {showRecordingViewer && (
          <RecordingViewer
            startTime={new Date(selectedDate.setHours(0, 0, 0, 0)).toISOString()}
            endTime={new Date(selectedDate.setHours(23, 59, 59, 999)).toISOString()}
            onClose={() => setShowRecordingViewer(false)}
          />
        )}
      </div>

      {/* Privacy Notice removed - not needed for fully local app */}

    </main>
  );
}
