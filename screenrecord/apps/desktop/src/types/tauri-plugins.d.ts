/**
 * Type declarations for Tauri plugins
 * These are stubs for build-time type checking.
 * The actual modules are only available in Tauri runtime.
 */

declare module '@tauri-apps/plugin-updater' {
  export interface Update {
    version: string;
    currentVersion: string;
    body?: string;
    date?: string;
    downloadAndInstall(onEvent?: (event: any) => void): Promise<void>;
  }

  export function check(): Promise<Update | null>;
}

declare module '@tauri-apps/plugin-process' {
  export function relaunch(): Promise<void>;
}




