/**
 * ============================================================================
 * Resource Configuration
 * ============================================================================
 * 
 * Defines all external binary resources that are bundled with the application.
 * Each resource specifies download URLs, extraction logic, and verification.
 * 
 * To add a new resource:
 * 1. Add a new entry to the `resources` object below
 * 2. Define platforms with URLs, archive types, and binary paths
 * 3. Optionally specify a verification command
 */

const path = require('path');
const fs = require('fs');
const { copyDirRecursive } = require('./lib/utils');

// =============================================================================
// Resource Definitions
// =============================================================================

const resources = {
  /**
   * ActivityWatch - Time tracking and activity monitoring
   * https://github.com/ActivityWatch/activitywatch
   */
  activitywatch: {
    name: 'ActivityWatch',
    version: 'v0.13.2',
    resourceDir: 'activitywatch',
    skipEnvVar: 'SKIP_AW_CHECK',
    
    platforms: {
      'darwin-aarch64': {
        url: (version) => `https://github.com/ActivityWatch/activitywatch/releases/download/${version}/activitywatch-${version}-macos-x86_64.zip`,
        archiveType: 'zip',
        // Note: macOS builds are x86_64 but work on Apple Silicon via Rosetta
      },
      'darwin-x86_64': {
        url: (version) => `https://github.com/ActivityWatch/activitywatch/releases/download/${version}/activitywatch-${version}-macos-x86_64.zip`,
        archiveType: 'zip',
      },
      'windows-x86_64': {
        url: (version) => `https://github.com/ActivityWatch/activitywatch/releases/download/${version}/activitywatch-${version}-windows-x86_64.zip`,
        archiveType: 'zip',
      },
      'linux-x86_64': {
        url: (version) => `https://github.com/ActivityWatch/activitywatch/releases/download/${version}/activitywatch-${version}-linux-x86_64.zip`,
        archiveType: 'zip',
      },
    },
    
    // Binaries to extract (relative paths in target directory)
    binaries: [
      { name: 'aw-server', path: 'aw-server/aw-server', windowsPath: 'aw-server/aw-server.exe' },
      { name: 'aw-watcher-window', path: 'aw-watcher-window/aw-watcher-window', windowsPath: 'aw-watcher-window/aw-watcher-window.exe' },
      { name: 'aw-watcher-afk', path: 'aw-watcher-afk/aw-watcher-afk', windowsPath: 'aw-watcher-afk/aw-watcher-afk.exe' },
      { name: 'aw-watcher-input', path: 'aw-watcher-input/aw-watcher-input', windowsPath: 'aw-watcher-input/aw-watcher-input.exe' },
    ],
    
    // Primary binary for verification (just check existence)
    primaryBinary: (platform) => {
      const isWindows = platform.startsWith('windows');
      return isWindows ? 'aw-server/aw-server.exe' : 'aw-server/aw-server';
    },
    
    // Custom extraction logic for ActivityWatch
    extractBinaries: (extractDir, targetDir, platform) => {
      // Find the activitywatch directory in extracted content
      const contents = fs.readdirSync(extractDir);
      let awDir = extractDir;
      
      for (const item of contents) {
        const fullPath = path.join(extractDir, item);
        if (fs.statSync(fullPath).isDirectory() && item.startsWith('activitywatch')) {
          awDir = fullPath;
          break;
        }
      }
      
      // Copy each binary directory
      const binariesToCopy = ['aw-server', 'aw-watcher-window', 'aw-watcher-afk', 'aw-watcher-input'];
      const copied = [];
      
      for (const binary of binariesToCopy) {
        const sourcePath = path.join(awDir, binary);
        const targetPath = path.join(targetDir, binary);
        
        if (fs.existsSync(sourcePath)) {
          // Remove target if exists
          if (fs.existsSync(targetPath)) {
            fs.rmSync(targetPath, { recursive: true, force: true });
          }
          // Copy directory (cross-platform)
          copyDirRecursive(sourcePath, targetPath);
          copied.push(binary);
        }
      }
      
      return copied;
    },
    
    // No version verification command (just check existence)
    verifyCommand: null,
  },
  
  /**
   * FFmpeg - Video encoding for screen recording
   * Static builds from various sources per platform
   */
  ffmpeg: {
    name: 'FFmpeg',
    resourceDir: 'ffmpeg',
    skipEnvVar: 'SKIP_FFMPEG_CHECK',
    
    platforms: {
      'darwin-aarch64': {
        url: 'https://evermeet.cx/ffmpeg/ffmpeg-7.1.1.zip',
        archiveType: 'zip',
        binaryName: 'ffmpeg',
        extractedName: 'ffmpeg',
      },
      'darwin-x86_64': {
        url: 'https://evermeet.cx/ffmpeg/ffmpeg-7.1.1.zip',
        archiveType: 'zip',
        binaryName: 'ffmpeg',
        extractedName: 'ffmpeg',
      },
      'windows-x86_64': {
        url: 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip',
        archiveType: 'zip',
        binaryName: 'ffmpeg.exe',
        extractedPattern: /ffmpeg-.*-essentials.*[\/\\]bin[\/\\]ffmpeg\.exe$/,
      },
      'linux-x86_64': {
        url: 'https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz',
        archiveType: 'tar.xz',
        binaryName: 'ffmpeg',
        extractedPattern: /ffmpeg-.*-amd64-static[\/\\]ffmpeg$/,
      },
    },
    
    // Primary binary for verification
    primaryBinary: (platform) => {
      const isWindows = platform.startsWith('windows');
      return isWindows ? 'ffmpeg.exe' : 'ffmpeg';
    },
    
    // Simple extraction - find and copy single binary
    extractBinaries: null, // Use default extraction logic
    
    // Verification command (run ffmpeg -version)
    verifyCommand: (binaryPath) => `"${binaryPath}" -version`,
  },
  
  /**
   * Whisper Model - Speech-to-text transcription model
   * Single file that works on all platforms
   * https://huggingface.co/ggerganov/whisper.cpp
   */
  whisper: {
    name: 'Whisper Model',
    version: '1.5.4',
    resourceDir: '.', // Goes directly in resources/ directory
    skipEnvVar: 'SKIP_WHISPER_CHECK',
    
    // Single file, no platform-specific config needed
    isSingleFile: true,
    fileName: 'whisper-tiny.en.bin',
    
    // Direct download URL from Hugging Face
    // Note: Hugging Face serves it as ggml-tiny.en.bin, but we rename it to whisper-tiny.en.bin
    url: 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin',
    downloadedFileName: 'ggml-tiny.en.bin', // Actual filename from download
    
    // No extraction needed - it's a direct binary download
    archiveType: null,
    
    // Verification: just check file exists and has reasonable size (~75MB)
    verifyCommand: null,
    minFileSize: 70 * 1024 * 1024, // ~70MB minimum
  },
};

// =============================================================================
// Helper Functions
// =============================================================================

/**
 * Get configuration for a specific resource
 * @param {string} name - Resource name
 * @returns {Object|null} Resource configuration or null if not found
 */
function getResource(name) {
  return resources[name] || null;
}

/**
 * Get all resource names
 * @returns {string[]} Array of resource names
 */
function getAllResourceNames() {
  return Object.keys(resources);
}

/**
 * Get the download URL for a resource/platform combination
 * @param {string} resourceName - Resource name
 * @param {string} platform - Platform key
 * @returns {string|null} Download URL or null
 */
function getDownloadUrl(resourceName, platform) {
  const resource = resources[resourceName];
  if (!resource || !resource.platforms[platform]) {
    return null;
  }
  
  const platformConfig = resource.platforms[platform];
  const url = platformConfig.url;
  
  // URL can be a string or a function that takes version
  if (typeof url === 'function') {
    return url(resource.version);
  }
  return url;
}

/**
 * Get platform configuration for a resource
 * @param {string} resourceName - Resource name
 * @param {string} platform - Platform key
 * @returns {Object|null} Platform configuration or null
 */
function getPlatformConfig(resourceName, platform) {
  const resource = resources[resourceName];
  if (!resource || !resource.platforms[platform]) {
    return null;
  }
  return resource.platforms[platform];
}

// =============================================================================
// Exports
// =============================================================================

module.exports = {
  resources,
  getResource,
  getAllResourceNames,
  getDownloadUrl,
  getPlatformConfig,
};
