#!/usr/bin/env node

/**
 * ============================================================================
 * Bundle Resources Script
 * ============================================================================
 * 
 * Copies external binary resources into the Tauri app bundle after building.
 * This is necessary because Tauri's resources config doesn't handle large
 * binary distributions well.
 * 
 * USAGE:
 *   npm run bundle-resources
 * 
 * BUNDLED RESOURCES:
 *   - ActivityWatch: aw-server, aw-watcher-window, etc.
 *   - FFmpeg: Video encoding for screen recording
 *   - Binaries: Go executables (sj-collector, sj-tracker-report)
 *   - Python: Python virtual environment and chat agent source
 *   - Databases: MongoDB and InfluxDB binaries
 * 
 */

const fs = require('fs');
const path = require('path');
const { bundleResource, getAllResourceNames, getResource } = require('./lib/resource-manager');
const { printHeader, printSection, isMacOS, PATHS, copyDirRecursive, removeDir, fileExists } = require('./lib/utils');
const { signDirectory } = require('./lib/signing');

// =============================================================================
// Bundle Path Detection
// =============================================================================

const TARGET_PLATFORM = process.platform;

/**
 * Get the bundle resources path for the current platform
 */
function getBundlePath() {
  const bundleDir = path.join(__dirname, '..', 'src-tauri', 'target', 'release', 'bundle');
  
  // Platform-specific bundle paths
  if (TARGET_PLATFORM === 'darwin') {
    // macOS: Find the .app bundle
    const macosDir = path.join(bundleDir, 'macos');
    if (fs.existsSync(macosDir)) {
      const apps = fs.readdirSync(macosDir).filter(f => f.endsWith('.app'));
      if (apps.length > 0) {
        return path.join(macosDir, apps[0], 'Contents', 'Resources');
      }
    }
  } else if (TARGET_PLATFORM === 'win32') {
    // Windows: resources go next to the .exe
    const nsis = path.join(bundleDir, 'nsis');
    if (fs.existsSync(nsis)) {
      return nsis;
    }
  } else if (TARGET_PLATFORM === 'linux') {
    // Linux: AppImage or deb
    const appimage = path.join(bundleDir, 'appimage');
    if (fs.existsSync(appimage)) {
      return appimage;
    }
  }
  
  return null;
}

// =============================================================================
// Main
// =============================================================================

function main() {
  printHeader('Bundling Resources');
  
  // Find bundle location
  const bundlePath = getBundlePath();
  if (!bundlePath) {
    console.error('❌ Could not find Tauri bundle directory');
    console.error('   Make sure you ran `tauri build` first');
    process.exit(1);
  }
  
  console.log(`🎯 Bundle target: ${bundlePath}\n`);
  
  // Bundle each resource from resource-manager (activitywatch, ffmpeg)
  const resourceNames = getAllResourceNames();
  const results = {};
  
  for (const resourceName of resourceNames) {
    const resource = getResource(resourceName);
    console.log(`📦 Bundling ${resource.name}...`);
    
    const result = bundleResource(resourceName, bundlePath);
    results[resourceName] = result;
    
    if (result.success) {
      console.log(`  ✅ ${resource.name} bundled successfully!\n`);
    } else {
      console.error(`  ❌ Failed: ${result.error}`);
      if (resourceName === 'activitywatch') {
        console.error(`     Run: npm run setup-aw\n`);
      } else if (resourceName === 'ffmpeg') {
        console.error(`     Run: npm run setup-ffmpeg\n`);
      } else if (resourceName === 'whisper') {
        console.error(`     Run: npm run setup-whisper\n`);
      }
    }
  }
  
  // Bundle additional resources (binaries, python, databases, frontend, scripts)
  const additionalResources = ['binaries', 'python', 'databases', 'frontend'];
  
  // Bundle individual script files (platform-specific)
  const scriptFiles = process.platform === 'win32' 
    ? ['start-bundled.bat']
    : ['start-bundled.sh'];
  
  for (const resourceDir of additionalResources) {
    console.log(`📦 Bundling ${resourceDir}...`);
    
    const sourcePath = path.join(PATHS.resourcesDir, resourceDir);
    const targetPath = path.join(bundlePath, resourceDir);
    
    // Check if source exists
    if (!fileExists(sourcePath)) {
      console.warn(`  ⚠️  ${resourceDir} not found at ${sourcePath}, skipping...\n`);
      results[resourceDir] = { success: false, error: 'Source not found', skipped: true };
      continue;
    }
    
    // Remove existing
    if (fileExists(targetPath)) {
      removeDir(targetPath);
    }
    
    // Copy
    try {
      copyDirRecursive(sourcePath, targetPath);
      
      // Make binaries and scripts executable (Unix only)
      if (process.platform !== 'win32' && (resourceDir === 'binaries' || resourceDir === 'databases')) {
        console.log(`  🔧 Making binaries executable...`);
        const { makeExecutable, findFiles } = require('./lib/utils');
        const binaries = findFiles(targetPath, /^(mongod|influxd|sj-collector|sj-tracker-report)$/);
        binaries.forEach(b => {
          try {
            makeExecutable(b);
          } catch (e) {
            console.warn(`  ⚠️  Failed to make ${b} executable: ${e.message}`);
          }
        });
      }
      
      // Make Python executables executable (Unix only)
      if (process.platform !== 'win32' && resourceDir === 'python') {
        console.log(`  🔧 Making Python executables executable...`);
        const { makeExecutable, findFiles } = require('./lib/utils');
        const pythonExes = findFiles(targetPath, /^sj-chat-agent$/);
        pythonExes.forEach(exe => {
          try {
            makeExecutable(exe);
            console.log(`  🔧 Made executable: ${path.basename(exe)}`);
          } catch (e) {
            console.warn(`  ⚠️  Failed to make ${exe} executable: ${e.message}`);
          }
        });
      }
      
      // Make shell scripts executable (Unix only)
      if (process.platform !== 'win32') {
        const { makeExecutable, findFiles } = require('./lib/utils');
        const scripts = findFiles(targetPath, /\.sh$/);
        scripts.forEach(script => {
          try {
            makeExecutable(script);
            console.log(`  🔧 Made script executable: ${path.basename(script)}`);
          } catch (e) {
            console.warn(`  ⚠️  Failed to make ${script} executable: ${e.message}`);
          }
        });
      }
      
      // Sign on macOS
      if (isMacOS()) {
        console.log(`  🔏 Signing ${resourceDir} binaries...`);
        const signResults = signDirectory(targetPath, PATHS.entitlements, { verbose: false });
        if (signResults.success > 0) {
          console.log(`  ✓ Signed ${signResults.success} binaries`);
        }
        if (signResults.failed > 0) {
          console.warn(`  ⚠️  ${signResults.failed} binaries failed to sign`);
        }
      }
      
      results[resourceDir] = { success: true };
      console.log(`  ✅ ${resourceDir} bundled successfully!\n`);
    } catch (error) {
      const errorMsg = `Copy failed: ${error.message}`;
      console.error(`  ❌ Failed: ${errorMsg}\n`);
      results[resourceDir] = { success: false, error: errorMsg };
    }
  }
  
  // Bundle individual script files
  for (const scriptFile of scriptFiles) {
    console.log(`📦 Bundling ${scriptFile}...`);
    
    const sourcePath = path.join(PATHS.resourcesDir, scriptFile);
    const targetPath = path.join(bundlePath, scriptFile);
    
    if (!fileExists(sourcePath)) {
      console.warn(`  ⚠️  ${scriptFile} not found at ${sourcePath}, skipping...\n`);
      results[scriptFile] = { success: false, error: 'Source not found', skipped: true };
      continue;
    }
    
    try {
      const { copyFile, makeExecutable } = require('./lib/utils');
      copyFile(sourcePath, targetPath);
      
      // Make script executable (Unix only)
      if (process.platform !== 'win32') {
        makeExecutable(targetPath);
        console.log(`  🔧 Made script executable`);
      }
      
      // Sign on macOS
      if (isMacOS()) {
        console.log(`  🔏 Signing ${scriptFile}...`);
        const signResults = signDirectory(path.dirname(targetPath), PATHS.entitlements, { verbose: false });
        if (signResults.success > 0) {
          console.log(`  ✓ Signed ${signResults.success} files`);
        }
      }
      
      results[scriptFile] = { success: true };
      console.log(`  ✅ ${scriptFile} bundled successfully!\n`);
    } catch (error) {
      const errorMsg = `Copy failed: ${error.message}`;
      console.error(`  ❌ Failed: ${errorMsg}\n`);
      results[scriptFile] = { success: false, error: errorMsg };
    }
  }
  
  // Summary
  printSection('Bundle Summary');
  
  let hasFailures = false;
  for (const [name, result] of Object.entries(results)) {
    const status = result.success ? '✅' : (result.skipped ? '⚠️' : '❌');
    console.log(`  ${status} ${name}${result.skipped ? ' (skipped)' : ''}`);
    if (!result.success && !result.skipped) hasFailures = true;
  }
  
  console.log('═'.repeat(60));
  
  if (hasFailures) {
    console.error('\n❌ Some resources failed to bundle. Check errors above.');
    process.exit(1);
  }
  
  console.log('\n✅ All resources bundled successfully!\n');
  console.log(`📍 Resources are at: ${bundlePath}`);
}

// =============================================================================
// Entry Point
// =============================================================================

// Run if called directly
if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error('❌ Failed to bundle resources:', error.message);
    process.exit(1);
  }
}

module.exports = { getBundlePath };
