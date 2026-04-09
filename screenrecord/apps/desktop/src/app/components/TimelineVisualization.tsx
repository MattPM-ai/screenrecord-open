/**
 * Timeline Visualization Component
 * 
 * Displays a horizontal timeline bar showing activity segments throughout the day.
 * Segments are colored by status (active/idle/afk) and show app information on hover.
 * Supports clicking segments for detailed view.
 */

"use client";

import { useState } from "react";
import type { TimelineSegment } from "@/lib/activitywatchClient";
import { formatDuration } from "@/lib/activityProcessor";

interface TimelineVisualizationProps {
  segments: TimelineSegment[];
  selectedDate: Date;
  onSegmentClick?: (segment: TimelineSegment) => void;
  onSegmentHover?: (segment: TimelineSegment | null) => void;
}

export function TimelineVisualization({
  segments,
  selectedDate,
  onSegmentClick,
  onSegmentHover,
}: TimelineVisualizationProps) {
  const [hoveredSegment, setHoveredSegment] = useState<TimelineSegment | null>(null);
  const [mousePosition, setMousePosition] = useState({ x: 0, y: 0 });

  // Get color for segment type
  const getSegmentColor = (type: TimelineSegment["type"]) => {
    switch (type) {
      case "active":
        return "bg-green-500 hover:bg-green-600";
      case "idle":
        return "bg-yellow-500 hover:bg-yellow-600";
      case "afk":
        return "bg-red-500 hover:bg-red-600";
      case "unknown":
        return "bg-gray-300 hover:bg-gray-400";
      default:
        return "bg-gray-300 hover:bg-gray-400";
    }
  };

  // Calculate segment position and width as percentage of 24-hour day
  const calculateSegmentStyle = (segment: TimelineSegment) => {
    const dayStart = new Date(selectedDate);
    dayStart.setHours(0, 0, 0, 0);
    const dayEnd = new Date(selectedDate);
    dayEnd.setHours(23, 59, 59, 999);
    
    const totalDayMs = dayEnd.getTime() - dayStart.getTime();
    const segmentStart = segment.start.getTime() - dayStart.getTime();
    const segmentDuration = segment.end.getTime() - segment.start.getTime();
    
    const left = (segmentStart / totalDayMs) * 100;
    const width = (segmentDuration / totalDayMs) * 100;
    
    return {
      left: `${Math.max(0, left)}%`,
      width: `${Math.max(0.1, width)}%`,
    };
  };

  // Handle segment hover
  const handleSegmentHover = (segment: TimelineSegment | null, event?: React.MouseEvent) => {
    setHoveredSegment(segment);
    if (event) {
      setMousePosition({ x: event.clientX, y: event.clientY });
    }
    onSegmentHover?.(segment);
  };

  // Generate time labels (00:00, 06:00, 12:00, 18:00, 24:00)
  const timeLabels = [
    { time: "00:00", position: 0 },
    { time: "06:00", position: 25 },
    { time: "12:00", position: 50 },
    { time: "18:00", position: 75 },
    { time: "24:00", position: 100 },
  ];

  if (segments.length === 0) {
    return (
      <div className="bg-gray-50 rounded-lg p-8 text-center">
        <p className="text-gray-500">No activity data for this day</p>
      </div>
    );
  }

  return (
    <div className="bg-white rounded-lg p-5 shadow-md h-full">
      <h3 className="text-lg font-semibold mb-4">Daily Timeline</h3>
      
      {/* Timeline Container */}
      <div className="relative">
        {/* Time Labels */}
        <div className="relative h-6 mb-2">
          {timeLabels.map((label) => (
            <div
              key={label.time}
              className="absolute text-xs text-gray-500"
              style={{ left: `${label.position}%`, transform: "translateX(-50%)" }}
            >
              {label.time}
            </div>
          ))}
        </div>

        {/* Timeline Bar */}
        <div className="relative h-21 bg-gray-100 rounded-lg overflow-hidden">
          {/* Hour Grid Lines */}
          {Array.from({ length: 24 }, (_, i) => (
            <div
              key={i}
              className="absolute top-0 bottom-0 w-px bg-gray-200"
              style={{ left: `${(i / 24) * 100}%` }}
            />
          ))}

          {/* Activity Segments */}
          {segments.map((segment, index) => {
            const style = calculateSegmentStyle(segment);
            return (
              <div
                key={index}
                className={`absolute top-0 bottom-0 ${getSegmentColor(segment.type)} transition-all cursor-pointer`}
                style={style}
                onMouseEnter={(e) => handleSegmentHover(segment, e)}
                onMouseLeave={() => handleSegmentHover(null)}
                onMouseMove={(e) => setMousePosition({ x: e.clientX, y: e.clientY })}
                onClick={() => onSegmentClick?.(segment)}
                title={segment.app || segment.type}
              />
            );
          })}
        </div>

        {/* Legend */}
        <div className="flex items-center gap-4 mt-4 text-sm">
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 bg-green-500 rounded"></div>
            <span className="text-gray-700">Active</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 bg-yellow-500 rounded"></div>
            <span className="text-gray-700">Idle</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 bg-red-500 rounded"></div>
            <span className="text-gray-700">AFK</span>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 bg-gray-300 rounded"></div>
            <span className="text-gray-700">Unknown</span>
          </div>
        </div>
      </div>

      {/* Hover Tooltip */}
      {hoveredSegment && (
        <div
          className="fixed z-50 bg-gray-900 text-white px-3 py-2 rounded-lg shadow-lg text-sm pointer-events-none"
          style={{
            left: `${mousePosition.x + 10}px`,
            top: `${mousePosition.y + 10}px`,
          }}
        >
          <div className="font-semibold mb-1">
            {hoveredSegment.start.toLocaleTimeString()} - {hoveredSegment.end.toLocaleTimeString()}
          </div>
          <div className="text-gray-300 mb-1">
            Duration: {formatDuration(hoveredSegment.duration, "compact")}
          </div>
          {hoveredSegment.app && (
            <div className="text-gray-300 mb-1">
              App: {hoveredSegment.app}
            </div>
          )}
          {hoveredSegment.title && (
            <div className="text-gray-300 text-xs line-clamp-2">
              {hoveredSegment.title}
            </div>
          )}
          <div className="text-gray-400 text-xs mt-1 capitalize">
            Status: {hoveredSegment.type}
          </div>
        </div>
      )}
    </div>
  );
}

