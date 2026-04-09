/**
 * ============================================================================
 * COLLECTOR CLIENT - TypeScript API
 * ============================================================================
 * 
 * PURPOSE: Type-safe frontend API for interacting with collector Tauri commands
 * 
 * FUNCTIONALITY:
 * - Get status and statistics
 * - Configure collector settings (start/stop handled internally)
 * - Test connections
 * - Update JWT tokens
 * 
 * ============================================================================
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * Collector configuration type
 * Matches Rust CollectorConfig struct
 */
export type CollectorConfig = {
  enabled: boolean;
  server_url: string;
  auth_url: string;
  user_name: string;
  user_id: string;
  org_name: string;
  org_id: string;
  account_id: string;
  batch_max_size: number;
  batch_max_interval_seconds: number;
  retry_max_attempts: number;
  retry_backoff_base_ms: number;
  retry_backoff_multiplier: number;
  retry_max_delay_seconds: number;
  offline_queue_max_batches: number;
  websocket_keepalive_seconds: number;
  connection_timeout_seconds: number;
  flush_on_afk: boolean;
  auto_reconnect: boolean;
  app_jwt_token?: string;
};

/**
 * Transmission status enum
 */
export type TransmissionStatus =
  | { type: 'Disconnected' }
  | { type: 'Connecting' }
  | { type: 'Authenticating' }
  | { type: 'Connected' }
  | { type: 'Error'; message: string };

/**
 * Synchronization statistics
 */
export type SyncStatistics = {
  total_events_sent: number;
  total_batches_sent: number;
  last_sync_time: string | null;
  pending_events: number;
  connection_status: TransmissionStatus;
  last_error: string | null;
  retry_attempts: number;
};

/**
 * Default configuration values
 */
export const DEFAULT_COLLECTOR_CONFIG: CollectorConfig = {
  enabled: false,
  server_url: 'ws://localhost:8080/time-series',
  auth_url: 'http://localhost:8080/mock-auth',
  user_name: 'Local',
  user_id: '0',
  org_name: 'Local',
  org_id: '0',
  account_id: '0',
  batch_max_size: 100,
  batch_max_interval_seconds: 60,
  retry_max_attempts: 5,
  retry_backoff_base_ms: 1000,
  retry_backoff_multiplier: 2.0,
  retry_max_delay_seconds: 60,
  offline_queue_max_batches: 1000,
  websocket_keepalive_seconds: 30,
  connection_timeout_seconds: 10,
  flush_on_afk: true,
  auto_reconnect: true,
  app_jwt_token: undefined,
};

/**
 * Get current collector status and statistics
 */
export async function getCollectorStatus(): Promise<SyncStatistics> {
  try {
    const stats = await invoke<SyncStatistics>('get_collector_status');
    return stats;
  } catch (error: any) {
    throw new Error(`Failed to get collector status: ${error?.message || error}`);
  }
}

/**
 * Update collector configuration
 * Restarts collector if currently running
 */
export async function updateCollectorConfig(config: CollectorConfig): Promise<void> {
  try {
    await invoke('update_collector_config', { config });
  } catch (error: any) {
    throw new Error(`Failed to update configuration: ${error?.message || error}`);
  }
}

/**
 * Get current collector configuration
 * Returns saved configuration or defaults if none exists
 */
export async function getCollectorConfig(): Promise<CollectorConfig> {
  try {
    const config = await invoke<CollectorConfig>('get_collector_config');
    return config;
  } catch (error: any) {
    // Return defaults if config doesn't exist or fails to load
    console.warn('Failed to load collector config, using defaults:', error);
    return DEFAULT_COLLECTOR_CONFIG;
  }
}

/**
 * Test connection to collector server
 * Attempts connection and authentication without starting the collector
 */
export async function testConnection(config: CollectorConfig): Promise<string> {
  try {
    const result = await invoke<string>('test_collector_connection', { config });
    return result;
  } catch (error: any) {
    throw new Error(`Connection test failed: ${error?.message || error}`);
  }
}

/**
 * Update app JWT token in collector config cache
 * This allows updating the token without restarting the collector
 * Useful when token is refreshed after expiration
 */
export async function updateCollectorAppJwtToken(token: string | null): Promise<void> {
  try {
    await invoke('update_collector_app_jwt_token', { token: token || null });
  } catch (error: any) {
    throw new Error(`Failed to update collector app JWT token: ${error?.message || error}`);
  }
}

/**
 * Get status badge color based on transmission status
 */
export function getStatusColor(status: TransmissionStatus): string {
  switch (status.type) {
    case 'Connected':
      return 'green';
    case 'Connecting':
    case 'Authenticating':
      return 'yellow';
    case 'Disconnected':
      return 'gray';
    case 'Error':
      return 'red';
    default:
      return 'gray';
  }
}

/**
 * Validate collector configuration
 * Returns error message if invalid, null if valid
 */
export function validateConfig(config: CollectorConfig): string | null {
  if (config.enabled) {
    if (!config.user_name || config.user_name.trim() === '') {
      return 'User name is required when collector is enabled';
    }
    if (!config.user_id || config.user_id.trim() === '') {
      return 'User ID is required when collector is enabled';
    }
    if (!config.org_name || config.org_name.trim() === '') {
      return 'Organization name is required when collector is enabled';
    }
    if (!config.org_id || config.org_id.trim() === '') {
      return 'Organization ID is required when collector is enabled';
    }
    if (!config.account_id || config.account_id.trim() === '') {
      return 'Account ID is required when collector is enabled';
    }
    if (config.user_name.length > 64) {
      return 'User name must be 64 characters or less';
    }
    if (config.user_id.length > 64) {
      return 'User ID must be 64 characters or less';
    }
    if (config.org_name.length > 64) {
      return 'Organization name must be 64 characters or less';
    }
    if (config.org_id.length > 64) {
      return 'Organization ID must be 64 characters or less';
    }
    if (config.account_id.length > 64) {
      return 'Account ID must be 64 characters or less';
    }
  }

  if (!config.server_url.startsWith('ws://') && !config.server_url.startsWith('wss://')) {
    return 'Server URL must start with ws:// or wss://';
  }
  if (!config.auth_url.startsWith('http://') && !config.auth_url.startsWith('https://')) {
    return 'Auth URL must start with http:// or https://';
  }

  if (config.batch_max_size < 10 || config.batch_max_size > 10000) {
    return 'Batch size must be between 10 and 10,000';
  }
  if (config.batch_max_interval_seconds < 10 || config.batch_max_interval_seconds > 3600) {
    return 'Batch interval must be between 10 and 3,600 seconds';
  }
  if (config.retry_max_attempts < 1 || config.retry_max_attempts > 20) {
    return 'Retry attempts must be between 1 and 20';
  }

  return null;
}

