"use client";

/**
 * ============================================================================
 * SERVICE STARTUP SCREEN
 * ============================================================================
 * 
 * Displays a loading screen showing the progress of backend service startup.
 * Shows which services are starting, ready, or failed.
 * 
 * ============================================================================
 */

import { useEffect, useRef, useState } from "react";
import { getAllServicesStatus, didServicesStartupComplete, type AllServicesStatus } from "@/lib/servicesClient";
import { CheckCircle2, Loader2, XCircle, AlertCircle } from "lucide-react";
import { cn } from "@repo/ui";
import { listen } from "@tauri-apps/api/event";

type ServiceStatus = "starting" | "ready" | "failed" | "unknown";

interface ServiceInfo {
  name: string;
  displayName: string;
  status: ServiceStatus;
  error?: string;
  message?: string;
}

const SERVICE_ORDER = [
  { key: "mongodb", displayName: "MongoDB Database" },
  { key: "influxdb", displayName: "InfluxDB Database" },
  { key: "collector", displayName: "Collector Service" },
  { key: "report", displayName: "Report Service" },
  { key: "chat_agent", displayName: "Chat Agent" },
  { key: "frontend", displayName: "Report Frontend" },
] as const;

const MAX_WAIT_TIME_MS = 60000; // 60 seconds max wait
const POLL_INTERVAL_MS = 1000; // Check every second

export function ServiceStartupScreen({
  onReady,
  onError,
}: {
  onReady: () => void;
  onError?: (error: string) => void;
}) {
  const [services, setServices] = useState<ServiceInfo[]>(
    SERVICE_ORDER.map((s) => ({
      name: s.key,
      displayName: s.displayName,
      status: "starting" as ServiceStatus,
      message: `Launching ${s.displayName}...`,
    }))
  );
  const [startTime] = useState(Date.now());
  const [overallStatus, setOverallStatus] = useState<"starting" | "ready" | "timeout">("starting");
  const pollIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    let pollInterval: NodeJS.Timeout;
    let timeout: NodeJS.Timeout;
    let mounted = true;
    let unlistenProgress: (() => void) | null = null;
    pollIntervalRef.current = null;
    timeoutRef.current = null;

    // Listen for progress events from the startup script
    listen<{ service: string; status: string; message?: string }>(
      "service-progress",
      (event) => {
        if (!mounted) return;

        const { service, status, message } = event.payload;
        console.log("[ServiceStartupScreen] Received service-progress event:", { service, status, message });

        setServices((prev) =>
          prev.map((s) => {
            if (s.name === service) {
              let newStatus: ServiceStatus = "starting";
              let newMessage: string | undefined = `Launching ${s.displayName}...`;
              
              if (status === "ready") {
                newStatus = "ready";
                newMessage = undefined; // Clear message when ready
              } else if (status === "failed") {
                newStatus = "failed";
                newMessage = message || `Failed to start ${s.displayName}`;
              } else if (status === "skipped") {
                newStatus = "ready"; // Treat skipped as ready
                newMessage = undefined;
              }

              return {
                ...s,
                status: newStatus,
                message: newMessage,
                error: status === "failed" ? message : undefined,
              };
            }
            return s;
          })
        );

        // If all services are ready, trigger onReady
        if (service === "all" && status === "ready") {
          console.log("[ServiceStartupScreen] Received all:ready event, calling onReady()");
          setOverallStatus("ready");
          pollIntervalRef.current = null;
          timeoutRef.current = null;
          clearInterval(pollInterval);
          clearTimeout(timeout);
          setTimeout(() => {
            if (mounted) {
              console.log("[ServiceStartupScreen] Executing onReady() callback");
              onReady();
            }
          }, 500);
        }
      }
    ).then((unlisten) => {
      unlistenProgress = unlisten;
    });

    const checkServices = async () => {
      if (!mounted) return;
      
      try {
        // Poll completion flag first (reliable when events don't reach webview, e.g. Windows + CREATE_NO_WINDOW)
        const startupComplete = await didServicesStartupComplete();
        if (startupComplete && overallStatus === "starting") {
          console.log("[ServiceStartupScreen] did_services_startup_complete=true, calling onReady()");
          setOverallStatus("ready");
          pollIntervalRef.current = null;
          timeoutRef.current = null;
          clearInterval(pollInterval);
          clearTimeout(timeout);
          setTimeout(() => {
            if (mounted) onReady();
          }, 500);
          return;
        }

        const status = await getAllServicesStatus();

        if (!mounted) return;

        // Update service statuses (fallback if events don't arrive)
        setServices((prev) =>
          prev.map((service) => {
            const serviceStatus = status[service.name as keyof AllServicesStatus];
            if (!serviceStatus) {
              return service;
            }

            // Only update if status is still "starting" (don't override progress events)
            if (service.status !== "starting") {
              return service;
            }

            let newStatus: ServiceStatus = "starting";
            let newMessage: string | undefined = `Launching ${service.displayName}...`;
            
            // Only mark as ready if actually running
            if (serviceStatus.running) {
              newStatus = "ready";
              newMessage = undefined; // Clear message when ready
            }
            // Don't mark as failed just because of an error - keep showing "Launching..."
            // Only progress events from the script should mark services as failed
            // This prevents showing "connection failed" errors while services are still starting

            return {
              ...service,
              status: newStatus,
              message: newMessage,
              // Don't set error here - only set it from progress events
            };
          })
        );

        // Check if all services are ready
        const allReady = SERVICE_ORDER.every((s) => {
          const serviceStatus = status[s.key as keyof AllServicesStatus];
          return serviceStatus?.running === true;
        });

        if (allReady && overallStatus === "starting") {
          console.log("[ServiceStartupScreen] Poll detected all services ready, calling onReady()");
          setOverallStatus("ready");
          pollIntervalRef.current = null;
          timeoutRef.current = null;
          clearInterval(pollInterval);
          clearTimeout(timeout);
          setTimeout(() => {
            if (mounted) {
              console.log("[ServiceStartupScreen] Executing onReady() callback from poll");
              onReady();
            }
          }, 500);
        }
      } catch (error: any) {
        console.error("Failed to check services status:", error);
        // Continue polling even on error
      }
    };

    // Start polling (as fallback)
    pollInterval = setInterval(checkServices, POLL_INTERVAL_MS);
    pollIntervalRef.current = pollInterval;
    checkServices(); // Initial check

    // Set timeout
    timeout = setTimeout(() => {
      if (mounted && overallStatus === "starting") {
        setOverallStatus("timeout");
        clearInterval(pollInterval);
        setServices((currentServices) => {
          const failedServices = currentServices.filter((s) => s.status === "failed");
          if (failedServices.length > 0) {
            onError?.(
              `Some services failed to start: ${failedServices.map((s) => s.displayName).join(", ")}`
            );
          } else {
            onError?.("Services are taking longer than expected to start. Please check the logs.");
          }
          return currentServices;
        });
      }
    }, MAX_WAIT_TIME_MS);
    timeoutRef.current = timeout;

    return () => {
      mounted = false;
      clearInterval(pollInterval);
      clearTimeout(timeout);
      unlistenProgress?.();
    };
  }, [onReady, onError, overallStatus]);

  // Fallback: if UI state shows all services ready but we never got "all:ready" event (e.g. Windows pipe buffering), advance
  useEffect(() => {
    console.log("[ServiceStartupScreen] Fallback check:", {
      overallStatus,
      servicesStatuses: services.map((s) => ({ name: s.name, status: s.status })),
      allReady: services.every((s) => s.status === "ready"),
    });
    if (overallStatus !== "starting") {
      console.log("[ServiceStartupScreen] Fallback skipped: overallStatus is", overallStatus);
      return;
    }
    const allReadyInState = services.every((s) => s.status === "ready");
    if (!allReadyInState) {
      console.log("[ServiceStartupScreen] Fallback skipped: not all services ready");
      return;
    }
    console.log("[ServiceStartupScreen] Fallback triggered: all services ready, calling onReady()");
    setOverallStatus("ready");
    if (pollIntervalRef.current) {
      clearInterval(pollIntervalRef.current);
      pollIntervalRef.current = null;
    }
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    const t = setTimeout(() => onReady(), 500);
    return () => clearTimeout(t);
  }, [services, overallStatus, onReady]);

  const allReady = services.every((s) => s.status === "ready");
  const hasFailures = services.some((s) => s.status === "failed");
  const elapsedSeconds = Math.floor((Date.now() - startTime) / 1000);

  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-background">
      <div className="flex flex-col items-center space-y-8 w-full max-w-md px-6">
        {/* Logo and Title */}
        <div className="flex flex-col items-center space-y-6">
          <div className="w-10 h-10 flex items-center justify-center">
            <svg
              width="40"
              height="90"
              viewBox="0 0 153.06 343.77"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                transform="translate(-30.47 55.406)"
                d="m104.94-53.634a10.799 10.799 0 0 0-7.3721 5.9518l-60.033 128.37a53.776 53.776 0 0 0-1.2753 42.607l62.584 157.79a9.1583 9.1583 0 0 0 17.039-0.0312l61.894-157.73a54.353 54.353 0 0 0-1.2681-42.678l-59.357-128.29a10.799 10.799 0 0 0-12.21-5.9917zm-1.6924 197.56h7.5075a8.0688 9.9149 0 0 1 8.0688 9.915v50.733a8.0688 9.9149 0 0 1-8.0688 9.915h-7.5075a8.0688 9.9149 0 0 1-8.0688-9.915v-50.733a8.0688 9.9149 0 0 1 8.0688-9.915z"
                fill="currentColor"
                className="text-foreground"
              />
            </svg>
          </div>
          <h1 className="text-2xl font-semibold mt-2">ScreenJournal Tracker</h1>
          <p className="text-sm text-muted-foreground">
            {allReady
              ? "All services ready"
              : hasFailures
              ? "Some services failed to start"
              : "Starting backend services..."}
          </p>
        </div>

        {/* Service Status List */}
        <div className="w-full space-y-2">
          {services.map((service) => (
            <div
              key={service.name}
              className={cn(
                "flex items-center justify-between p-3 rounded-lg border transition-colors",
                service.status === "ready"
                  ? "bg-green-50 dark:bg-green-950/20 border-green-200 dark:border-green-900"
                  : service.status === "failed"
                  ? "bg-red-50 dark:bg-red-950/20 border-red-200 dark:border-red-900"
                  : "bg-muted border-border"
              )}
            >
              <div className="flex items-center space-x-3 flex-1 min-w-0">
                {service.status === "ready" ? (
                  <CheckCircle2 className="w-5 h-5 text-green-600 dark:text-green-400 flex-shrink-0" />
                ) : service.status === "failed" ? (
                  <XCircle className="w-5 h-5 text-red-600 dark:text-red-400 flex-shrink-0" />
                ) : (
                  <Loader2 className="w-5 h-5 text-muted-foreground animate-spin flex-shrink-0" />
                )}
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">{service.displayName}</p>
                  {service.message && service.status === "starting" && (
                    <p className="text-xs text-muted-foreground mt-1 truncate">
                      {service.message}
                    </p>
                  )}
                  {service.error && service.status === "failed" && (
                    <p className="text-xs text-red-600 dark:text-red-400 mt-1 truncate">
                      {service.error}
                    </p>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>

        {/* Status Message */}
        {overallStatus === "timeout" && (
          <div className="flex items-center space-x-2 text-amber-600 dark:text-amber-400">
            <AlertCircle className="w-4 h-4" />
            <p className="text-sm">
              Services are taking longer than expected. You can continue, but some features may not
              be available.
            </p>
          </div>
        )}

        {/* Elapsed Time */}
        <p className="text-xs text-muted-foreground">Elapsed: {elapsedSeconds}s</p>
      </div>
    </div>
  );
}

