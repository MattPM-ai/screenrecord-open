/**
 * Current Status Card Component
 * 
 * Displays real-time activity status including current app, AFK state,
 * and time since last input. Updates every 2 seconds via parent polling.
 */

"use client";

import { Clock, Monitor, Keyboard, Mouse, CheckCircle, XCircle } from "lucide-react";
import type { CurrentStatus } from "@/lib/activitywatchClient";
import { formatDuration } from "@/lib/activityProcessor";

interface CurrentStatusCardProps {
  status: CurrentStatus | null;
  isOnline: boolean;
}

export function CurrentStatusCard({ status, isOnline }: CurrentStatusCardProps) {
  // Determine status color and label
  const getStatusInfo = () => {
    if (!isOnline || !status) {
      return {
        color: "bg-gray-500",
        textColor: "text-gray-700",
        borderColor: "border-gray-300",
        label: "Offline",
      };
    }

    const afkStatus = status.afk_status.toLowerCase();
    
    if (afkStatus === "not-afk" || afkStatus === "active") {
      return {
        color: "bg-green-500",
        textColor: "text-green-700",
        borderColor: "border-green-300",
        label: "Active",
      };
    } else if (afkStatus === "idle") {
      return {
        color: "bg-yellow-500",
        textColor: "text-yellow-700",
        borderColor: "border-yellow-300",
        label: "Idle",
      };
    } else if (afkStatus === "afk") {
      return {
        color: "bg-red-500",
        textColor: "text-red-700",
        borderColor: "border-red-300",
        label: "AFK",
      };
    }

    return {
      color: "bg-gray-500",
      textColor: "text-gray-700",
      borderColor: "border-gray-300",
      label: "Unknown",
    };
  };

  const statusInfo = getStatusInfo();

  return (
    <div className={`border-2 ${statusInfo.borderColor} rounded-lg p-4 bg-white shadow-md transition-all duration-300`}>
      {/* Status Pill */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <div className={`${statusInfo.color} text-white px-4 py-2 rounded-full font-semibold text-sm flex items-center gap-2`}>
            {statusInfo.label}
            {isOnline && status ? (
              <div className="w-2 h-2 bg-white rounded-full animate-pulse" />
            ) : null}
          </div>
        </div>
        
        {/* Sync Status Badge */}
        <div className="flex items-center gap-1 text-xs text-gray-600">
          {isOnline ? (
            <>
              <CheckCircle className="w-4 h-4 text-green-600" />
              <span>Online</span>
            </>
          ) : (
            <>
              <XCircle className="w-4 h-4 text-red-600" />
              <span>Offline</span>
            </>
          )}
        </div>
      </div>

      {/* Current App & Title */}
      {status && isOnline && (
        <>
          <div className="space-y-3">
            {/* Current Application */}
            <div className="flex items-start gap-2">
              <Monitor className="w-5 h-5 text-gray-600 shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <div className="text-xs text-gray-500 font-medium">Current App</div>
                <div className="text-sm font-semibold text-gray-900 truncate">
                  {status.current_app || "No active window"}
                </div>
              </div>
            </div>

            {/* Window Title */}
            {status.current_title && (
              <div className="flex items-start gap-2">
                <div className="w-5 h-5 shrink-0" />
                <div className="flex-1 min-w-0">
                  <div className="text-xs text-gray-500 font-medium">Window Title</div>
                  <div className="text-sm text-gray-700 line-clamp-2">
                    {status.current_title}
                  </div>
                </div>
              </div>
            )}

            {/* Time in Current State */}
            <div className="flex items-center gap-2">
              <Clock className="w-5 h-5 text-gray-600 shrink-0" />
              <div className="flex-1">
                <div className="text-xs text-gray-500 font-medium">Time in State</div>
                <div className="text-sm font-semibold text-gray-900">
                  {formatDuration(status.time_in_state, "compact")}
                </div>
              </div>
            </div>

            {/* Last Input Time */}
            {status.last_input_time && (
              <div className="flex items-center gap-2">
                <div className="flex gap-1">
                  <Keyboard className="w-4 h-4 text-gray-600" />
                  <Mouse className="w-4 h-4 text-gray-600" />
                </div>
                <div className="flex-1">
                  <div className="text-xs text-gray-500 font-medium">Last Input</div>
                  <div className="text-sm text-gray-700">
                    {new Date(status.last_input_time).toLocaleTimeString()}
                  </div>
                </div>
              </div>
            )}
          </div>
        </>
      )}

      {/* Offline Message */}
      {(!status || !isOnline) && (
        <div className="text-center py-4">
          <p className="text-sm text-gray-500">
            Waiting for activity data...
          </p>
        </div>
      )}
    </div>
  );
}

