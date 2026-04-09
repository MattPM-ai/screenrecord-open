#!/usr/bin/env node

/**
 * Tauri Build Wrapper Script
 * 
 * Sets up the correct environment variables for FFmpeg on different platforms
 * before running tauri dev or tauri build.
 * 
 * This is needed because FFmpeg headers are in non-standard locations
 * on macOS (Homebrew) and need to be discoverable by the build system.
 */

const { spawn, execSync } = require('child_process');
const os = require('os');
const path = require('path');

const platform = os.platform();
const args = process.argv.slice(2);

/**
 * Get Homebrew prefix (works for both Intel and Apple Silicon)
 */
function getBrewPrefix() {
  try {
    return execSync('brew --prefix', { encoding: 'utf8' }).trim();
  } catch {
    return null;
  }
}

/**
 * Get FFmpeg installation prefix from Homebrew
 */
function getFFmpegPrefix() {
  try {
    return execSync('brew --prefix ffmpeg', { encoding: 'utf8' }).trim();
  } catch {
    return null;
  }
}

/**
 * Setup environment variables for macOS
 */
function setupMacOSEnv(env) {
  const brewPrefix = getBrewPrefix();
  const ffmpegPrefix = getFFmpegPrefix();

  if (!brewPrefix || !ffmpegPrefix) {
    console.warn('⚠️  Could not detect Homebrew/FFmpeg paths. Build may fail.');
    return env;
  }

  console.log(`📍 Homebrew prefix: ${brewPrefix}`);
  console.log(`📍 FFmpeg prefix: ${ffmpegPrefix}`);

  // Set FFMPEG_DIR for ffmpeg-sys-next
  env.FFMPEG_DIR = ffmpegPrefix;

  // Update PKG_CONFIG_PATH to include FFmpeg
  const pkgConfigPaths = [
    `${ffmpegPrefix}/lib/pkgconfig`,
    `${brewPrefix}/lib/pkgconfig`,
    env.PKG_CONFIG_PATH,
  ].filter(Boolean);
  env.PKG_CONFIG_PATH = pkgConfigPaths.join(':');

  // Set include paths - CPATH is the most reliable for clang
  const includePaths = [
    `${ffmpegPrefix}/include`,
    `${brewPrefix}/include`,
  ];
  const includePathStr = includePaths.join(':');
  
  // CPATH is respected by both gcc and clang for all languages
  env.CPATH = includePathStr;
  env.C_INCLUDE_PATH = includePathStr;
  env.CPLUS_INCLUDE_PATH = includePathStr;
  
  // Set library path
  const libPaths = [
    `${ffmpegPrefix}/lib`,
    `${brewPrefix}/lib`,
  ];
  const libPathStr = libPaths.join(':');
  env.LIBRARY_PATH = libPathStr;
  env.DYLD_LIBRARY_PATH = libPathStr;
  env.LD_LIBRARY_PATH = libPathStr;

  // For bindgen/clang to find headers - use -isystem for system headers
  env.BINDGEN_EXTRA_CLANG_ARGS = [
    `-isystem${ffmpegPrefix}/include`,
    `-isystem${brewPrefix}/include`,
    `-I${ffmpegPrefix}/include`,
    `-I${brewPrefix}/include`,
  ].join(' ');

  // Also set CFLAGS and LDFLAGS for good measure
  env.CFLAGS = `-I${ffmpegPrefix}/include -I${brewPrefix}/include`;
  env.LDFLAGS = `-L${ffmpegPrefix}/lib -L${brewPrefix}/lib`;

  return env;
}

/**
 * Setup environment variables for Linux
 */
function setupLinuxEnv(env) {
  // Linux typically has FFmpeg in standard paths, but we can check
  const pkgConfigPaths = [
    '/usr/lib/pkgconfig',
    '/usr/lib/x86_64-linux-gnu/pkgconfig',
    '/usr/local/lib/pkgconfig',
    env.PKG_CONFIG_PATH,
  ].filter(Boolean);
  env.PKG_CONFIG_PATH = pkgConfigPaths.join(':');
  return env;
}

/**
 * Setup environment variables for Windows
 */
function setupWindowsEnv(env) {
  // Check common FFmpeg installation paths on Windows
  const possiblePaths = [
    'C:\\ffmpeg',
    'C:\\Program Files\\ffmpeg',
    process.env.FFMPEG_DIR,
  ].filter(Boolean);

  for (const p of possiblePaths) {
    try {
      const fs = require('fs');
      if (fs.existsSync(path.join(p, 'include', 'libavcodec'))) {
        env.FFMPEG_DIR = p;
        console.log(`📍 FFmpeg found at: ${p}`);
        break;
      }
    } catch {
      continue;
    }
  }

  return env;
}

/**
 * Main function
 */
function main() {
  console.log('🎬 Setting up FFmpeg environment for Tauri build...\n');

  // Clone current environment
  let env = { ...process.env };

  // For build command, temporarily disable Tauri's automatic notarization
  // We'll handle notarization manually after signing all binaries
  if (args[0] === 'build' && process.platform === 'darwin') {
    // Unset notarization env vars so Tauri doesn't try to notarize automatically
    // We'll notarize manually after signing
    delete env.APPLE_API_ISSUER;
    delete env.APPLE_API_KEY;
    delete env.APPLE_API_KEY_PATH;
    delete env.APPLE_ID;
    delete env.APPLE_PASSWORD;
    console.log('ℹ️  Disabled Tauri automatic notarization (will notarize manually after signing)\n');
  }

  // Setup platform-specific environment
  switch (platform) {
    case 'darwin':
      env = setupMacOSEnv(env);
      break;
    case 'linux':
      env = setupLinuxEnv(env);
      break;
    case 'win32':
      env = setupWindowsEnv(env);
      break;
  }

  // Log key environment variables
  console.log('\n📋 Environment variables set:');
  console.log(`   FFMPEG_DIR=${env.FFMPEG_DIR || '(not set)'}`);
  console.log(`   PKG_CONFIG_PATH=${env.PKG_CONFIG_PATH || '(not set)'}`);
  console.log(`   C_INCLUDE_PATH=${env.C_INCLUDE_PATH || '(not set)'}`);
  console.log('');

  // Determine tauri command
  const tauriCmd = args[0] === 'build' ? 'tauri build' : 'tauri dev';
  const tauriArgs = args.slice(1);

  console.log(`🚀 Running: ${tauriCmd} ${tauriArgs.join(' ')}\n`);

  // Spawn tauri process
  const child = spawn('npx', ['tauri', args[0] || 'dev', ...tauriArgs], {
    stdio: 'inherit',
    env,
    shell: true,
  });

  child.on('close', (code) => {
    process.exit(code || 0);
  });

  child.on('error', (err) => {
    console.error('Failed to start tauri:', err);
    process.exit(1);
  });
}

main();
