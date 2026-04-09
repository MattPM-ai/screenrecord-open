// Simple client to ensure ActivityWatch is running and call health endpoint
import { invoke } from '@tauri-apps/api/core';

export type ServerInfo = { base_url: string; port: number; version?: string | null };

export async function getServerInfo(): Promise<ServerInfo | null> {
  try {
    const info = await invoke<ServerInfo | null>('get_server_info');
    return info;
  } catch {
    return null;
  }
}

export async function startServer(): Promise<ServerInfo> {
  return await invoke<ServerInfo>('start_server');
}

export async function getHealth(): Promise<{ ok: boolean; details?: string }>{
  try {
    const status = await invoke<{ healthy: boolean; message?: string }>('get_server_status');
    return { ok: !!status.healthy, details: status.message };
  } catch (e: any) {
    return { ok: false, details: String(e?.message || e) };
  }
}

export async function stopServer(): Promise<void> {
  await invoke('stop_server');
}

// Bucket information returned from the server
export type BucketInfo = {
  id: string;
  bucket_type: string;
  client: string;
  hostname: string;
  created: string;  // ISO 8601 timestamp
  event_count: number | null;
};

// Event information from a bucket
export type EventInfo = {
  id: number | null;
  timestamp: string;  // ISO 8601 timestamp
  duration: number;   // Duration in seconds
  data: Record<string, any>;  // Event-specific data
};

// Response containing bucket events
export type BucketEventsResponse = {
  bucket_id: string;
  events: EventInfo[];
  total_count: number;
};

// Current real-time status of user activity
export type CurrentStatus = {
  last_update: string;
  current_app: string | null;
  current_title: string | null;
  afk_status: "active" | "idle" | "afk" | "not-afk" | "unknown";
  time_in_state: number;
  last_input_time: string | null;
};

// Events grouped by type for a date range
export type DateRangeEventsResponse = {
  window_events: EventInfo[];
  afk_events: EventInfo[];
  input_events: EventInfo[];
};

// Aggregated metrics for a specific day
export type DailyMetrics = {
  date: string;
  total_active_seconds: number;
  total_idle_seconds: number;
  total_afk_seconds: number;
  utilization_ratio: number;
  app_switches: number;
};

// Application usage statistics
export type AppUsage = {
  app_name: string;
  total_seconds: number;
  window_titles: string[];
  event_count: number;
  category: "productive" | "neutral" | "unproductive" | null;
};

// Timeline segment for visualization
export type TimelineSegment = {
  start: Date;
  end: Date;
  duration: number;
  type: "active" | "idle" | "afk" | "unknown";
  app?: string;
  title?: string;
};

// Fetch all buckets from the ActivityWatch server
export async function getBuckets(): Promise<BucketInfo[]> {
  try {
    const buckets = await invoke<BucketInfo[]>('get_buckets');
    return buckets;
  } catch (e: any) {
    throw new Error(`Failed to fetch buckets: ${e?.message || e}`);
  }
}

// Fetch recent events from a specific bucket
export async function getBucketEvents(
  bucketId: string,
  limit?: number
): Promise<BucketEventsResponse> {
  try {
    const response = await invoke<BucketEventsResponse>('get_bucket_events', {
      bucketId: bucketId,
      limit: limit || 20,  // Default to 20 events
    });
    return response;
  } catch (e: any) {
    throw new Error(`Failed to fetch events for bucket ${bucketId}: ${e?.message || e}`);
  }
}

// Fetch current real-time status
export async function getCurrentStatus(): Promise<CurrentStatus> {
  try {
    const status = await invoke<CurrentStatus>('get_current_status');
    return status;
  } catch (e: any) {
    throw new Error(`Failed to fetch current status: ${e?.message || e}`);
  }
}

// Fetch daily metrics for a specific date
export async function getDailyMetrics(date: string): Promise<DailyMetrics> {
  try {
    const metrics = await invoke<DailyMetrics>('get_daily_metrics', {
      date: date,  // Expected format: YYYY-MM-DD
    });
    return metrics;
  } catch (e: any) {
    throw new Error(`Failed to fetch daily metrics: ${e?.message || e}`);
  }
}

// Fetch app usage breakdown for a date range
export async function getAppUsageBreakdown(
  startTime: string,
  endTime: string
): Promise<AppUsage[]> {
  try {
    const apps = await invoke<AppUsage[]>('get_app_usage_breakdown', {
      startTime: startTime,
      endTime: endTime,
    });
    return apps;
  } catch (e: any) {
    throw new Error(`Failed to fetch app usage: ${e?.message || e}`);
  }
}

// Fetch events by date range
export async function getEventsByDateRange(
  startTime: string,
  endTime: string
): Promise<DateRangeEventsResponse> {
  try {
    const events = await invoke<DateRangeEventsResponse>('get_events_by_date_range', {
      startTime: startTime,
      endTime: endTime,
    });
    return events;
  } catch (e: any) {
    throw new Error(`Failed to fetch events by date range: ${e?.message || e}`);
  }
}

