/**
 * App Usage List Component
 * 
 * Displays a list of applications with time spent, sorted by duration.
 * Allows categorization (productive/neutral/unproductive) and shows window titles.
 * Supports expand/collapse for detailed view of each app.
 */

"use client";

import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { AppUsage } from "@/lib/activitywatchClient";
import { formatDuration } from "@/lib/activityProcessor";

interface AppUsageListProps {
  apps: AppUsage[];
  onCategoryChange?: (appName: string, category: "productive" | "neutral" | "unproductive") => void;
}

export function AppUsageList({ apps, onCategoryChange }: AppUsageListProps) {
  const [expandedApps, setExpandedApps] = useState<Set<string>>(new Set());
  const [sortBy, setSortBy] = useState<"time" | "name" | "category">("time");
  const [filterCategory, setFilterCategory] = useState<"all" | "productive" | "neutral" | "unproductive">("all");

  // Toggle app expansion
  const toggleApp = (appName: string) => {
    const newExpanded = new Set(expandedApps);
    if (newExpanded.has(appName)) {
      newExpanded.delete(appName);
    } else {
      newExpanded.add(appName);
    }
    setExpandedApps(newExpanded);
  };

  // Get category badge color
  const getCategoryColor = (category: AppUsage["category"]) => {
    switch (category) {
      case "productive":
        return "bg-blue-100 text-blue-700 hover:bg-blue-200";
      case "neutral":
        return "bg-gray-100 text-gray-700 hover:bg-gray-200";
      case "unproductive":
        return "bg-orange-100 text-orange-700 hover:bg-orange-200";
      default:
        return "bg-gray-100 text-gray-700 hover:bg-gray-200";
    }
  };

  // Handle category change with cycling
  const handleCategoryClick = (appName: string, currentCategory: AppUsage["category"]) => {
    const categories: Array<"productive" | "neutral" | "unproductive"> = ["productive", "neutral", "unproductive"];
    const currentIndex = currentCategory ? categories.indexOf(currentCategory) : 1; // Default to neutral
    const nextIndex = (currentIndex + 1) % categories.length;
    const nextCategory = categories[nextIndex] as "productive" | "neutral" | "unproductive";
    
    if (nextCategory) {
      onCategoryChange?.(appName, nextCategory);
    }
  };

  // Handle keyboard activation for app toggle
  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>, appName: string) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      toggleApp(appName);
    }
  };

  // Sort apps
  const sortedApps = [...apps].sort((a, b) => {
    switch (sortBy) {
      case "time":
        return b.total_seconds - a.total_seconds;
      case "name":
        return a.app_name.localeCompare(b.app_name);
      case "category":
        const catA = a.category || "neutral";
        const catB = b.category || "neutral";
        return catA.localeCompare(catB);
      default:
        return 0;
    }
  });

  // Filter apps by category
  const filteredApps = sortedApps.filter(app => {
    if (filterCategory === "all") return true;
    return app.category === filterCategory;
  });

  // Calculate total time for filtered apps
  const totalTime = filteredApps.reduce((sum, app) => sum + app.total_seconds, 0);

  if (apps.length === 0) {
    return (
      <div className="bg-white rounded-lg p-6 shadow-md">
        <h3 className="text-lg font-semibold mb-4">App Usage</h3>
        <p className="text-gray-500 text-center py-8">No app usage data available</p>
      </div>
    );
  }

  return (
    <div className="bg-white rounded-lg pt-6 pb-4 shadow-md overflow-hidden">
      <div className="flex items-center justify-between mb-4 px-6">
        <h3 className="text-lg font-semibold">App Usage</h3>
        {/* Uneeded due to being shown in the Daily Metrics Bar */}
        {/* <div className="text-sm text-gray-600">
          Total: {formatDuration(totalTime, "compact")}
        </div> */}
      </div>

      {/* Controls */}
      <div className="flex flex-wrap gap-2 mb-4 px-6">
        {/* Sort Controls */}
        <div className="flex items-center gap-2">
          <span className="text-xs text-gray-600 font-medium">Sort by:</span>
          <button
            onClick={() => setSortBy("time")}
            className={`px-2 py-1 text-xs rounded ${
              sortBy === "time" ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-700"
            }`}
          >
            Time
          </button>
          <button
            onClick={() => setSortBy("name")}
            className={`px-2 py-1 text-xs rounded ${
              sortBy === "name" ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-700"
            }`}
          >
            Name
          </button>
          <button
            onClick={() => setSortBy("category")}
            className={`px-2 py-1 text-xs rounded ${
              sortBy === "category" ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-700"
            }`}
          >
            Category
          </button>
        </div>

        {/* Filter Controls */}
        <div className="flex items-center gap-2">
          <span className="text-xs text-gray-600 font-medium">Filter:</span>
          <button
            onClick={() => setFilterCategory("all")}
            className={`px-2 py-1 text-xs rounded ${
              filterCategory === "all" ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-700"
            }`}
          >
            All
          </button>
          <button
            onClick={() => setFilterCategory("productive")}
            className={`px-2 py-1 text-xs rounded ${
              filterCategory === "productive" ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-700"
            }`}
          >
            Productive
          </button>
          <button
            onClick={() => setFilterCategory("neutral")}
            className={`px-2 py-1 text-xs rounded ${
              filterCategory === "neutral" ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-700"
            }`}
          >
            Neutral
          </button>
          <button
            onClick={() => setFilterCategory("unproductive")}
            className={`px-2 py-1 text-xs rounded ${
              filterCategory === "unproductive" ? "bg-blue-100 text-blue-700" : "bg-gray-100 text-gray-700"
            }`}
          >
            Unproductive
          </button>
        </div>
      </div>

      {/* App List */}
      <div className="space-y-2 max-h-[600px] overflow-y-auto">
        {filteredApps.map((app) => (
          <div
            key={app.app_name}
            className="border border-gray-200 rounded-[1rem] hover:bg-gray-50 transition-colors mx-3"
          >
            <div
              role="button"
              tabIndex={0}
              aria-expanded={expandedApps.has(app.app_name)}
              aria-label={`Toggle details for ${app.app_name}`}
              onClick={() => toggleApp(app.app_name)}
              onKeyDown={(e) => handleKeyDown(e, app.app_name)}
              className="w-full flex items-center justify-between p-3 text-left"
            >
              <div className="flex items-center gap-3 flex-1 min-w-0">
                {expandedApps.has(app.app_name) ? (
                  <ChevronDown className="w-5 h-5 shrink-0 text-gray-500" />
                ) : (
                  <ChevronRight className="w-5 h-5 shrink-0 text-gray-500" />
                )}
                <div className="flex-1 min-w-0">
                  <div className="font-semibold text-gray-900 truncate">{app.app_name}</div>
                  <div className="text-sm text-gray-600">
                    {formatDuration(app.total_seconds, "compact")} · {app.event_count} events
                  </div>
                </div>
              </div>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleCategoryClick(app.app_name, app.category);
                }}
                className={`px-3 py-1 text-xs font-medium rounded-full transition-colors ${getCategoryColor(app.category)}`}
              >
                {app.category || "neutral"}
              </button>
            </div>

            {/* Expanded Window Titles */}
            {expandedApps.has(app.app_name) && app.window_titles.length > 0 && (
              <div className="px-3 pb-3 pl-11 space-y-1">
                <div className="text-xs font-medium text-gray-600 mb-2">
                  Window Titles ({app.window_titles.length}):
                </div>
                {app.window_titles.slice(0, 10).map((title, idx) => (
                  <div key={idx} className="text-sm text-gray-700 truncate">
                    • {title}
                  </div>
                ))}
                {app.window_titles.length > 10 && (
                  <div className="text-sm text-gray-500 italic">
                    +{app.window_titles.length - 10} more...
                  </div>
                )}
              </div>
            )}
          </div>
        ))}
      </div>

      {filteredApps.length === 0 && (
        <p className="text-gray-500 text-center py-8">No apps match the current filter</p>
      )}
    </div>
  );
}

