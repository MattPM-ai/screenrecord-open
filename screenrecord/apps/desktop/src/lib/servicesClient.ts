/**
 * ============================================================================
 * SERVICES CLIENT - TypeScript API
 * ============================================================================
 * 
 * PURPOSE: Type-safe frontend API for checking backend service status
 * 
 * FUNCTIONALITY:
 * - Get status of all bundled backend services (MongoDB, InfluxDB, collector, report, chat agent)
 * 
 * ============================================================================
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * Service status type
 * Matches Rust ServiceStatus struct
 */
export type ServiceStatus = {
  name: string;
  running: boolean;
  pid: number | null;
  port: number | null;
  error: string | null;
};

/**
 * All services status type
 * Matches Rust AllServicesStatus struct
 */
export type AllServicesStatus = {
  mongodb: ServiceStatus;
  influxdb: ServiceStatus;
  collector: ServiceStatus;
  report: ServiceStatus;
  chat_agent: ServiceStatus;
  frontend: ServiceStatus;
};

/**
 * Get status of all backend services
 */
export async function getAllServicesStatus(): Promise<AllServicesStatus> {
  try {
    const status = await invoke<AllServicesStatus>('get_all_services_status');
    return status;
  } catch (e: any) {
    throw new Error(`Failed to get services status: ${e?.message || e}`);
  }
}




