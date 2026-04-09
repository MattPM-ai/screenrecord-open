/**
 * Segment Detail Panel Component
 * 
 * Displays detailed information about a selected timeline segment
 * in a modal/panel format with full event data.
 */

"use client";

import { X, Clock, Monitor, Activity } from "lucide-react";
import type { TimelineSegment } from "@/lib/activitywatchClient";
import { formatDuration } from "@/lib/activityProcessor";

interface SegmentDetailPanelProps {
  segment: TimelineSegment;
  onClose: () => void;
}

export function SegmentDetailPanel({ segment, onClose }: SegmentDetailPanelProps) {
  
  // Get status color
  const getStatusColor = (type: TimelineSegment["type"]) => {
    switch (type) {
      case "active":
        return "text-green-700 bg-green-50";
      case "idle":
        return "text-yellow-700 bg-yellow-50";
      case "afk":
        return "text-red-700 bg-red-50";
      case "unknown":
        return "text-gray-700 bg-gray-50";
      default:
        return "text-gray-700 bg-gray-50";
    }
  };

  return (
    <div className="absolute inset-0 bg-black bg-opacity-50 z-40 flex items-center justify-center p-4">
      <div className="bg-white rounded-lg shadow-xl max-w-2xl w-full max-h-[90vh] overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b border-gray-200">
          <h2 className="text-xl font-semibold text-gray-900">Activity Segment Details</h2>
          <button
            onClick={onClose}
            className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
            aria-label="Close"
          >
            <X className="w-5 h-5 text-gray-600" />
          </button>
        </div>

        {/* Content */}
        <div className="p-6 overflow-y-auto">
          <div className="space-y-6">
            {/* Status Badge */}
            <div className="flex items-center gap-3">
              <Activity className="w-6 h-6 text-gray-600" />
              <div>
                <div className="text-sm text-gray-500 font-medium mb-1">Status</div>
                <div className={`inline-flex items-center px-3 py-1 rounded-full text-sm font-semibold ${getStatusColor(segment.type)}`}>
                  {segment.type.toUpperCase()}
                </div>
              </div>
            </div>

            {/* Time Range */}
            <div className="flex items-start gap-3">
              <Clock className="w-6 h-6 text-gray-600 mt-0.5" />
              <div className="flex-1">
                <div className="text-sm text-gray-500 font-medium mb-1">Time Range</div>
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="text-sm text-gray-600">Start:</span>
                    <span className="font-semibold text-gray-900">
                      {segment.start.toLocaleTimeString()}
                    </span>
                    <span className="text-xs text-gray-500">
                      ({segment.start.toLocaleDateString()})
                    </span>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-sm text-gray-600">End:</span>
                    <span className="font-semibold text-gray-900">
                      {segment.end.toLocaleTimeString()}
                    </span>
                    <span className="text-xs text-gray-500">
                      ({segment.end.toLocaleDateString()})
                    </span>
                  </div>
                  <div className="flex items-center gap-2 mt-2">
                    <span className="text-sm text-gray-600">Duration:</span>
                    <span className="font-semibold text-gray-900">
                      {formatDuration(segment.duration, "verbose")}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            {/* Application Info */}
            {segment.app && (
              <div className="flex items-start gap-3">
                <Monitor className="w-6 h-6 text-gray-600 mt-0.5" />
                <div className="flex-1">
                  <div className="text-sm text-gray-500 font-medium mb-1">Application</div>
                  <div className="font-semibold text-gray-900 text-lg mb-2">
                    {segment.app}
                  </div>
                  {segment.title && (
                    <>
                      <div className="text-sm text-gray-500 font-medium mb-1">Window Title</div>
                      <div className="text-gray-700 bg-gray-50 p-3 rounded-lg wrap-break-word">
                        {segment.title}
                      </div>
                    </>
                  )}
                </div>
              </div>
            )}

            {/* No Application Data */}
            {!segment.app && (
              <div className="text-center py-4 text-gray-500">
                <p className="text-sm">No application data available for this segment</p>
                <p className="text-xs mt-1">This may occur during idle or AFK periods</p>
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-3 p-4 border-t border-gray-200 bg-gray-50">
          <button
            onClick={onClose}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors font-medium"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

