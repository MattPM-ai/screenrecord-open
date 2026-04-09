/**
 * Activity Data Processing Utilities
 * 
 * This module provides functions for transforming raw ActivityWatch events
 * into display-ready data structures for the dashboard.
 */

import type { EventInfo, TimelineSegment, AppUsage } from "./activitywatchClient";

// Default app categories for common applications
const DEFAULT_APP_CATEGORIES: Record<string, "productive" | "neutral" | "unproductive"> = {
  // Development Tools - Productive
  "Code": "productive",
  "Visual Studio Code": "productive",
  "Cursor": "productive",
  "IntelliJ IDEA": "productive",
  "PyCharm": "productive",
  "WebStorm": "productive",
  "Sublime Text": "productive",
  "Atom": "productive",
  "vim": "productive",
  "emacs": "productive",
  "Terminal": "productive",
  "iTerm2": "productive",
  "Hyper": "productive",
  "Warp": "productive",
  
  // Browsers - Neutral (depends on usage)
  "Google Chrome": "neutral",
  "Firefox": "neutral",
  "Safari": "neutral",
  "Microsoft Edge": "neutral",
  "Brave": "neutral",
  "Opera": "neutral",
  
  // Communication - Neutral
  "Slack": "neutral",
  "Discord": "neutral",
  "Microsoft Teams": "neutral",
  "Zoom": "neutral",
  "Skype": "neutral",
  "Mail": "neutral",
  "Outlook": "neutral",
  "Thunderbird": "neutral",
  
  // Productivity Tools - Productive
  "Notion": "productive",
  "Obsidian": "productive",
  "Evernote": "productive",
  "Bear": "productive",
  "Drafts": "productive",
  "Ulysses": "productive",
  "Roam Research": "productive",
  "Figma": "productive",
  "Sketch": "productive",
  "Adobe Photoshop": "productive",
  "Adobe Illustrator": "productive",
  
  // Entertainment - Unproductive
  "Netflix": "unproductive",
  "YouTube": "unproductive",
  "Spotify": "unproductive",
  "Apple Music": "unproductive",
  "iTunes": "unproductive",
  "VLC": "unproductive",
  "QuickTime Player": "unproductive",
  "Steam": "unproductive",
  "Epic Games Launcher": "unproductive",
  "Minecraft": "unproductive",
  
  // Social Media - Unproductive
  "Twitter": "unproductive",
  "Facebook": "unproductive",
  "Instagram": "unproductive",
  "TikTok": "unproductive",
  "Reddit": "unproductive",
  "WhatsApp": "unproductive",
  "Telegram": "unproductive",
};

/**
 * Merge window events with AFK status events to create timeline segments
 * 
 * This function combines window activity data (app name, title) with AFK status
 * to produce a unified timeline showing what the user was doing and whether they
 * were active or idle during that time.
 * 
 * @param windowEvents - Events from window watcher (app, title)
 * @param afkEvents - Events from AFK watcher (status: active/afk)
 * @returns Array of timeline segments with merged data
 */
export function mergeWindowAndAFKEvents(
  windowEvents: EventInfo[],
  afkEvents: EventInfo[]
): TimelineSegment[] {
  const segments: TimelineSegment[] = [];
  
  // Sort events by timestamp
  const sortedAfk = [...afkEvents].sort((a, b) => 
    new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
  );
  
  const sortedWindow = [...windowEvents].sort((a, b) => 
    new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
  );
  
  // Create segments from AFK events (primary timeline)
  for (const afkEvent of sortedAfk) {
    const start = new Date(afkEvent.timestamp);
    const end = new Date(start.getTime() + afkEvent.duration * 1000);
    
    // Determine AFK status
    const status = afkEvent.data.status as string;
    let type: "active" | "idle" | "afk" | "unknown" = "unknown";
    if (status === "not-afk") {
      type = "active";
    } else if (status === "afk") {
      type = "afk";
    } else if (status === "idle") {
      type = "idle";
    }
    
    // Find overlapping window event
    const overlappingWindow = sortedWindow.find(w => {
      const wStart = new Date(w.timestamp);
      const wEnd = new Date(wStart.getTime() + w.duration * 1000);
      
      // Check if there's any overlap
      return (wStart <= end && wEnd >= start);
    });
    
    segments.push({
      start,
      end,
      duration: afkEvent.duration,
      type,
      app: overlappingWindow?.data.app as string | undefined,
      title: overlappingWindow?.data.title as string | undefined,
    });
  }
  
  return segments;
}

/**
 * Generate timeline segments from raw events, filling gaps and normalizing data
 * 
 * @param windowEvents - Window activity events
 * @param afkEvents - AFK status events
 * @returns Display-ready timeline segments
 */
export function generateTimelineSegments(
  windowEvents: EventInfo[],
  afkEvents: EventInfo[]
): TimelineSegment[] {
  // Merge window and AFK data
  let segments = mergeWindowAndAFKEvents(windowEvents, afkEvents);
  
  // Sort by start time
  segments.sort((a, b) => a.start.getTime() - b.start.getTime());
  
  // Fill gaps with "unknown" status
  const filledSegments: TimelineSegment[] = [];
  
  for (let i = 0; i < segments.length; i++) {
    const current = segments[i];
    
    if (i > 0 && current && segments[i - 1]) {
      const previous = segments[i - 1]!;
      const gap = current.start.getTime() - previous.end.getTime();
      
      // If there's a gap > 1 second, add unknown segment
      if (gap > 1000) {
        filledSegments.push({
          start: previous.end,
          end: current.start,
          duration: gap / 1000,
          type: "unknown",
        });
      }
    }
    
    if (current) {
      filledSegments.push(current);
    }
  }
  
  // Merge consecutive segments with same app and status
  const mergedSegments: TimelineSegment[] = [];
  
  for (const segment of filledSegments) {
    if (mergedSegments.length === 0) {
      mergedSegments.push(segment);
      continue;
    }
    
    const last = mergedSegments[mergedSegments.length - 1];
    
    // Merge if same app and type
    if (last && last.app === segment.app && last.type === segment.type && last.title === segment.title) {
      last.end = segment.end;
      last.duration = (last.end.getTime() - last.start.getTime()) / 1000;
    } else {
      mergedSegments.push(segment);
    }
  }
  
  return mergedSegments;
}

/**
 * Detect anomalies in activity data that may require user review
 * 
 * @param segments - Timeline segments to analyze
 * @returns Array of anomalous segments with reasons
 */
export function detectAnomalies(
  segments: TimelineSegment[]
): Array<{ segment: TimelineSegment; reason: string }> {
  const anomalies: Array<{ segment: TimelineSegment; reason: string }> = [];
  
  for (const segment of segments) {
    // Extremely long single-app sessions (> 8 hours)
    if (segment.duration > 8 * 60 * 60) {
      anomalies.push({
        segment,
        reason: "Unusually long session (>8 hours) - possible overnight activity",
      });
    }
    
    // Very short segments (< 1 second) might indicate noise
    if (segment.duration < 1 && segment.type !== "unknown") {
      anomalies.push({
        segment,
        reason: "Very short event (<1 second) - possible data noise",
      });
    }
    
    // Active sessions during typical sleep hours (2 AM - 5 AM)
    const hour = segment.start.getHours();
    if (segment.type === "active" && hour >= 2 && hour < 5 && segment.duration > 60) {
      anomalies.push({
        segment,
        reason: "Active during typical sleep hours (2-5 AM)",
      });
    }
  }
  
  return anomalies;
}

/**
 * Format duration in seconds to human-readable string
 * 
 * @param seconds - Duration in seconds
 * @param format - Output format: 'compact', 'verbose', or 'seconds'
 * @returns Formatted duration string
 */
export function formatDuration(
  seconds: number,
  format: "compact" | "verbose" | "seconds" = "compact"
): string {
  if (format === "seconds") {
    return `${seconds.toFixed(0)}s`;
  }
  
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);
  
  if (format === "verbose") {
    const parts: string[] = [];
    if (hours > 0) parts.push(`${hours} hour${hours !== 1 ? 's' : ''}`);
    if (minutes > 0) parts.push(`${minutes} minute${minutes !== 1 ? 's' : ''}`);
    if (secs > 0 || parts.length === 0) parts.push(`${secs} second${secs !== 1 ? 's' : ''}`);
    return parts.join(', ');
  }
  
  // Compact format
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  } else {
    return `${secs}s`;
  }
}

/**
 * Categorize an app as productive, neutral, or unproductive
 * 
 * @param appName - Name of the application
 * @param userCategories - User-defined category overrides
 * @returns Category classification
 */
export function categorizeApp(
  appName: string,
  userCategories?: Map<string, "productive" | "neutral" | "unproductive">
): "productive" | "neutral" | "unproductive" {
  // Check user-defined categories first
  if (userCategories?.has(appName)) {
    return userCategories.get(appName)!;
  }
  
  // Check default categories
  if (DEFAULT_APP_CATEGORIES[appName]) {
    return DEFAULT_APP_CATEGORIES[appName];
  }
  
  // Default to neutral if uncategorized
  return "neutral";
}

/**
 * Apply categories to app usage data
 * 
 * @param apps - App usage data
 * @param userCategories - User-defined category overrides
 * @returns App usage with categories applied
 */
export function applyCategoriesToApps(
  apps: AppUsage[],
  userCategories?: Map<string, "productive" | "neutral" | "unproductive">
): AppUsage[] {
  return apps.map(app => ({
    ...app,
    category: categorizeApp(app.app_name, userCategories),
  }));
}

/**
 * Get default app categories map
 */
export function getDefaultCategories(): Map<string, "productive" | "neutral" | "unproductive"> {
  return new Map(Object.entries(DEFAULT_APP_CATEGORIES));
}

