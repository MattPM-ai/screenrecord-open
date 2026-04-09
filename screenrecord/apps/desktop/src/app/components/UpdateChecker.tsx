"use client";

/**
 * UpdateChecker Component
 * 
 * Checks for application updates on mount and displays a notification
 * banner when an update is available. Supports download progress tracking
 * and automatic application relaunch after installation.
 * 
 * Features:
 * - Automatic update check on app launch (with 5s delay)
 * - Download progress indicator
 * - Dismissible notification banner
 * - Error handling with retry option
 * 
 * Dependencies:
 * - @/lib/updaterClient: Update checking and installation
 * - @repo/ui: Button component
 * - lucide-react: Icons
 */

import { useEffect, useState } from "react";
import { checkForUpdate, downloadAndInstall, type UpdateStatus } from "@/lib/updaterClient";
import { Download, X, RefreshCw } from "lucide-react";
import { Button } from "@repo/ui";

/** Possible states for the update checker */
type UpdateState = "idle" | "checking" | "available" | "downloading" | "error";

/**
 * UpdateChecker Component
 * 
 * Renders a floating notification in the bottom-right corner when an
 * update is available. Provides download and dismiss actions.
 */
export function UpdateChecker() {
  const [state, setState] = useState<UpdateState>("idle");
  const [updateInfo, setUpdateInfo] = useState<UpdateStatus | null>(null);
  const [progress, setProgress] = useState<number>(0);
  const [error, setError] = useState<string>("");
  const [dismissed, setDismissed] = useState(false);
  const [isTauri, setIsTauri] = useState(false);

  // Check if we're in Tauri runtime (only in browser, not SSR)
  useEffect(() => {
    if (typeof window !== 'undefined' && typeof (window as any).__TAURI_INTERNALS__ !== 'undefined') {
      setIsTauri(true);
    }
  }, []);

  /**
   * Check for updates on mount
   * Delays 5 seconds to avoid blocking app startup
   * Only runs in Tauri runtime (not in Next.js dev server)
   */
  useEffect(() => {
    // Only run in Tauri runtime
    if (!isTauri) {
      return; // Not in Tauri context, skip update checking
    }

    const checkUpdate = async () => {
      // Wait 5 seconds after app launch before checking
      await new Promise((resolve) => setTimeout(resolve, 5000));
      
      setState("checking");
      try {
        const status = await checkForUpdate();
        setUpdateInfo(status);
        setState(status.available ? "available" : "idle");
        
        if (status.available) {
          console.log(`[UPDATE] New version available: ${status.version}`);
        }
      } catch (err) {
        // Silent fail for background check - don't bother user
        // Common causes: no release published yet, network issues, invalid JSON
        console.debug("[UPDATE] Check skipped:", err);
        setState("idle");
      }
    };

    checkUpdate();
  }, [isTauri]);

  // Don't render anything if not in Tauri context
  if (!isTauri) {
    return null;
  }

  /**
   * Handle download button click
   * Downloads the update with progress tracking and relaunches
   */
  const handleDownload = async () => {
    setState("downloading");
    setProgress(0);
    setError("");
    
    try {
      let downloaded = 0;
      await downloadAndInstall((chunk, total) => {
        downloaded += chunk;
        const percent = Math.round((downloaded / total) * 100);
        setProgress(percent);
      });
      // Note: App will relaunch after downloadAndInstall completes
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      console.error("[UPDATE] Download failed:", errorMessage);
      setError(errorMessage);
      setState("error");
    }
  };

  /**
   * Handle dismiss button click
   * Hides the notification until next app launch
   */
  const handleDismiss = () => {
    setDismissed(true);
  };

  // Don't render anything if no update, dismissed, or still checking
  if (dismissed || state === "idle" || state === "checking") {
    return null;
  }

  return (
    <div className="fixed bottom-4 right-4 z-50 max-w-sm">
      <div className="bg-white border border-gray-200 rounded-lg shadow-lg p-4">
        {/* Header */}
        <div className="flex items-start justify-between mb-2">
          <div className="flex items-center gap-2">
            <div className="p-1.5 bg-blue-100 rounded-full">
              <Download className="w-4 h-4 text-blue-600" />
            </div>
            <h3 className="font-semibold text-gray-900">Update Available</h3>
          </div>
          <button
            onClick={handleDismiss}
            className="text-gray-400 hover:text-gray-600 transition-colors"
            aria-label="Dismiss update notification"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content - Update Available State */}
        {state === "available" && updateInfo && (
          <>
            <p className="text-sm text-gray-600 mb-3">
              Version{" "}
              <span className="font-mono font-medium">{updateInfo.version}</span>{" "}
              is ready to install.
            </p>
            {updateInfo.body && (
              <p className="text-xs text-gray-500 mb-3 line-clamp-2">
                {updateInfo.body}
              </p>
            )}
            <div className="flex gap-2">
              <Button
                onClick={handleDownload}
                className="flex-1 text-sm"
              >
                <Download className="w-3.5 h-3.5 mr-1.5" />
                Update Now
              </Button>
              <Button
                onClick={handleDismiss}
                variant="outline"
                className="text-sm"
              >
                Later
              </Button>
            </div>
          </>
        )}

        {/* Content - Downloading State */}
        {state === "downloading" && (
          <>
            <p className="text-sm text-gray-600 mb-3">
              Downloading update...
            </p>
            <div className="w-full bg-gray-200 rounded-full h-2 mb-2">
              <div
                className="bg-blue-600 h-2 rounded-full transition-all duration-300"
                style={{ width: `${progress}%` }}
              />
            </div>
            <p className="text-xs text-gray-500 text-center">{progress}%</p>
          </>
        )}

        {/* Content - Error State */}
        {state === "error" && (
          <>
            <p className="text-sm text-red-600 mb-3">
              Failed to download update: {error}
            </p>
            <div className="flex gap-2">
              <Button
                onClick={handleDownload}
                variant="outline"
                className="flex-1 text-sm"
              >
                <RefreshCw className="w-3.5 h-3.5 mr-1.5" />
                Retry
              </Button>
              <Button
                onClick={handleDismiss}
                variant="outline"
                className="text-sm"
              >
                Dismiss
              </Button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

