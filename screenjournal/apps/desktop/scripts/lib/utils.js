/**
 * ============================================================================
 * Shared Utilities
 * ============================================================================
 * 
 * Common utility functions for file operations, platform detection, and paths.
 * Used by resource manager and other build scripts.
 */

const fs = require('fs');
const path = require('path');
const os = require('os');

// =============================================================================
// Common Paths
// =============================================================================

const SCRIPTS_DIR = path.join(__dirname, '..');
const TAURI_DIR = path.join(SCRIPTS_DIR, '..', 'src-tauri');
const RESOURCES_DIR = path.join(TAURI_DIR, 'resources');
const TEMP_DIR = path.join(SCRIPTS_DIR, '..', '.temp-resource-download');

const PATHS = {
  scriptsDir: SCRIPTS_DIR,
  tauriDir: TAURI_DIR,
  resourcesDir: RESOURCES_DIR,
  tempDir: TEMP_DIR,
  entitlements: path.join(TAURI_DIR, 'entitlements.plist'),
};

// =============================================================================
// Platform Detection
// =============================================================================

/**
 * Get the current platform identifier
 * @returns {string|null} Platform key like 'darwin-aarch64', 'windows-x86_64', etc.
 */
function getCurrentPlatform() {
  const platform = os.platform();
  const arch = os.arch();
  
  if (platform === 'darwin') {
    return arch === 'arm64' ? 'darwin-aarch64' : 'darwin-x86_64';
  } else if (platform === 'win32') {
    return 'windows-x86_64';
  } else if (platform === 'linux') {
    return 'linux-x86_64';
  }
  
  return null;
}

/**
 * Get all supported platform keys
 * @returns {string[]} Array of platform keys
 */
function getAllPlatforms() {
  return ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64', 'linux-x86_64'];
}

/**
 * Get detailed platform information
 * @returns {{ os: string, arch: string, platformKey: string|null }}
 */
function getPlatformInfo() {
  return {
    os: os.platform(),
    arch: os.arch(),
    platformKey: getCurrentPlatform(),
  };
}

/**
 * Check if running on macOS
 * @returns {boolean}
 */
function isMacOS() {
  return os.platform() === 'darwin';
}

/**
 * Check if running on Windows
 * @returns {boolean}
 */
function isWindows() {
  return os.platform() === 'win32';
}

/**
 * Check if running on Linux
 * @returns {boolean}
 */
function isLinux() {
  return os.platform() === 'linux';
}

// =============================================================================
// File Operations
// =============================================================================

/**
 * Ensure a directory exists, creating it recursively if needed
 * @param {string} dir - Directory path
 */
function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}

/**
 * Remove a directory recursively
 * @param {string} dir - Directory path
 */
function removeDir(dir) {
  if (fs.existsSync(dir)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

/**
 * Check if a file exists
 * @param {string} filePath - File path
 * @returns {boolean}
 */
function fileExists(filePath) {
  return fs.existsSync(filePath);
}

/**
 * Copy a file
 * @param {string} src - Source path
 * @param {string} dest - Destination path
 */
function copyFile(src, dest) {
  ensureDir(path.dirname(dest));
  fs.copyFileSync(src, dest);
}

/**
 * Copy a directory recursively (cross-platform)
 * @param {string} src - Source directory path
 * @param {string} dest - Destination directory path
 */
function copyDirRecursive(src, dest) {
  ensureDir(dest);
  const entries = fs.readdirSync(src, { withFileTypes: true });
  
  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);
    
    if (entry.isDirectory()) {
      copyDirRecursive(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

/**
 * Make a file executable (Unix only)
 * @param {string} filePath - File path
 */
function makeExecutable(filePath) {
  if (!isWindows() && fs.existsSync(filePath)) {
    fs.chmodSync(filePath, 0o755);
  }
}

/**
 * Recursively find files matching a pattern
 * @param {string} dir - Directory to search
 * @param {RegExp} pattern - Pattern to match against full path
 * @returns {string[]} Array of matching file paths
 */
function findFiles(dir, pattern) {
  const results = [];
  
  function walk(currentDir) {
    if (!fs.existsSync(currentDir)) return;
    
    const files = fs.readdirSync(currentDir);
    for (const file of files) {
      const fullPath = path.join(currentDir, file);
      const stat = fs.statSync(fullPath);
      
      if (stat.isDirectory()) {
        walk(fullPath);
      } else if (pattern.test(fullPath)) {
        results.push(fullPath);
      }
    }
  }
  
  walk(dir);
  return results;
}

// =============================================================================
// Logging Utilities
// =============================================================================

/**
 * Print a header box
 * @param {string} title - Title text
 */
function printHeader(title) {
  console.log(`
╔${'═'.repeat(60)}╗
║  ${title.padEnd(56)}║
╚${'═'.repeat(60)}╝
`);
}

/**
 * Print a section divider
 * @param {string} title - Section title
 */
function printSection(title) {
  console.log(`\n${'═'.repeat(60)}`);
  console.log(`📊 ${title}`);
  console.log('═'.repeat(60));
}

// =============================================================================
// Exports
// =============================================================================

module.exports = {
  PATHS,
  // Platform
  getCurrentPlatform,
  getAllPlatforms,
  getPlatformInfo,
  isMacOS,
  isWindows,
  isLinux,
  // Files
  ensureDir,
  removeDir,
  fileExists,
  copyFile,
  copyDirRecursive,
  makeExecutable,
  findFiles,
  // Logging
  printHeader,
  printSection,
};
