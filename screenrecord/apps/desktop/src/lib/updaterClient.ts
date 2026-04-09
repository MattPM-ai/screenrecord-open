/**
 * Updater Client
 * 
 * Provides functions to check for and install application updates
 * using the Tauri updater plugin. Handles update detection, download
 * progress tracking, and application relaunch.
 * 
 * Dependencies:
 * - @tauri-apps/plugin-updater: Update checking and installation
 * - @tauri-apps/plugin-process: Application relaunch after update
 * 
 * Note: These plugins only work in Tauri runtime, not in Next.js dev server
 */

// Dynamically import Tauri plugins to avoid SSR issues
// These will only be loaded in Tauri runtime, not during SSR
// Using 'any' type to avoid TypeScript errors during Next.js build
// since these modules are only available in Tauri runtime
let updaterModule: any = null;
let processModule: any = null;

// Check if we're in Tauri runtime (not SSR or Next.js dev server)
function isTauriRuntime(): boolean {
  if (typeof window === 'undefined') {
    return false; // SSR context
  }
  return typeof (window as any).__TAURI_INTERNALS__ !== 'undefined';
}

// Lazy load plugins only when needed and in Tauri context
async function loadPlugins() {
  // Only run in Tauri runtime
  if (!isTauriRuntime()) {
    return false;
  }
  
  try {
    if (!updaterModule) {
      // Dynamic import - modules only available in Tauri runtime
      // Type declarations in types/tauri-plugins.d.ts provide build-time types
      updaterModule = await import('@tauri-apps/plugin-updater');
    }
    if (!processModule) {
      // Dynamic import - modules only available in Tauri runtime
      // Type declarations in types/tauri-plugins.d.ts provide build-time types
      processModule = await import('@tauri-apps/plugin-process');
    }
    return true;
  } catch (error) {
    // Plugins not available (e.g., during Next.js build or dev server)
    return false;
  }
}

/**
 * Update status information returned by checkForUpdate
 */
export type UpdateStatus = {
  /** Whether an update is available */
  available: boolean;
  /** New version string (e.g., "1.2.0") */
  version?: string;
  /** Current installed version */
  currentVersion?: string;
  /** Release notes / changelog */
  body?: string;
  /** Release date */
  date?: string;
};

/**
 * Progress callback for download tracking
 * @param downloaded - Bytes downloaded so far
 * @param total - Total bytes to download
 */
export type ProgressCallback = (downloaded: number, total: number) => void;

/**
 * Check if an application update is available
 * 
 * Contacts the update endpoint configured in tauri.conf.json and
 * compares the remote version against the current installed version.
 * 
 * @returns UpdateStatus object indicating availability and version info
 * @throws Error if the update check fails (network error, invalid response, etc.)
 */
export async function checkForUpdate(): Promise<UpdateStatus> {
  // Load plugins if not already loaded
  const pluginsAvailable = await loadPlugins();
  if (!pluginsAvailable || !updaterModule) {
    return { available: false };
  }

  try {
    const update = await updaterModule.check();
    
    if (update) {
      return {
        available: true,
        version: update.version,
        currentVersion: update.currentVersion,
        body: update.body ?? undefined,
        date: update.date ?? undefined,
      };
    }
    
    return { available: false };
  } catch (error) {
    console.error('[UPDATER] Failed to check for updates:', error);
    // Return no update available instead of throwing in dev mode
    return { available: false };
  }
}

/**
 * Download and install the available update, then relaunch the app
 * 
 * Downloads the update package with progress tracking, installs it,
 * and relaunches the application to apply the update.
 * 
 * @param onProgress - Optional callback for download progress updates
 * @throws Error if no update is available or download/install fails
 */
export async function downloadAndInstall(
  onProgress?: ProgressCallback
): Promise<void> {
  // Load plugins if not already loaded
  const pluginsAvailable = await loadPlugins();
  if (!pluginsAvailable || !updaterModule || !processModule) {
    throw new Error('Update functionality is only available in Tauri runtime');
  }

  const update = await updaterModule.check();
  
  if (!update) {
    throw new Error('No update available');
  }
  
  console.log(`[UPDATER] Starting download of version ${update.version}`);
  
  let totalDownloaded = 0;
  let totalSize = 0;
  
  // Download with progress callback
  await update.downloadAndInstall((event: any) => {
    if (event.event === 'Started') {
      totalSize = event.data.contentLength ?? 0;
      console.log(`[UPDATER] Download started: ${totalSize || 'unknown'} bytes`);
    } else if (event.event === 'Progress') {
      totalDownloaded += event.data.chunkLength;
      
      if (totalSize && onProgress) {
        onProgress(totalDownloaded, totalSize);
      }
    } else if (event.event === 'Finished') {
      console.log('[UPDATER] Download finished, installing...');
    }
  });
  
  console.log('[UPDATER] Update installed, relaunching application...');
  
  // Relaunch the app to apply the update
  await processModule.relaunch();
}

