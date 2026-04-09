#!/usr/bin/env node

/**
 * ============================================================================
 * Resource Setup Script
 * ============================================================================
 * 
 * Downloads and installs external binary resources for the application.
 * 
 * USAGE:
 *   node scripts/setup-resource.js activitywatch     # Setup AW for current platform
 *   node scripts/setup-resource.js ffmpeg            # Setup FFmpeg for current platform
 *   node scripts/setup-resource.js activitywatch --all   # Setup AW for all platforms
 *   node scripts/setup-resource.js ffmpeg --all      # Setup FFmpeg for all platforms
 *   node scripts/setup-resource.js --all             # Setup ALL resources for all platforms
 * 
 * OPTIONS:
 *   --all       Download for all platforms (default: current platform only)
 *   --force     Re-download even if already installed
 * 
 */

const { setupResource, getAllResourceNames } = require('./lib/resource-manager');
const { printHeader, printSection, getCurrentPlatform } = require('./lib/utils');

// =============================================================================
// CLI Argument Parsing
// =============================================================================

function parseArgs() {
  const args = process.argv.slice(2);
  
  const options = {
    resources: [],
    allPlatforms: args.includes('--all'),
    force: args.includes('--force'),
  };
  
  // Get resource names (non-flag arguments)
  for (const arg of args) {
    if (!arg.startsWith('--')) {
      options.resources.push(arg);
    }
  }
  
  // If no resources specified but --all is set, setup all resources
  if (options.resources.length === 0 && options.allPlatforms) {
    options.resources = getAllResourceNames();
  }
  
  return options;
}

// =============================================================================
// Main
// =============================================================================

async function main() {
  const options = parseArgs();
  
  // Show help if no resources specified
  if (options.resources.length === 0) {
    console.log(`
Usage: node scripts/setup-resource.js <resource> [options]

Resources:
  activitywatch   ActivityWatch time tracking binaries
  ffmpeg          FFmpeg video encoding binary

Options:
  --all           Download for all platforms (default: current platform only)
  --force         Re-download even if already installed

Examples:
  npm run setup-aw                    # Setup ActivityWatch for current platform
  npm run setup-ffmpeg                # Setup FFmpeg for current platform
  npm run setup-ffmpeg -- --all       # Setup FFmpeg for all platforms
  npm run setup -- --all              # Setup all resources for all platforms
`);
    process.exit(0);
  }
  
  // Validate resource names
  const validResources = getAllResourceNames();
  for (const resource of options.resources) {
    if (!validResources.includes(resource)) {
      console.error(`❌ Unknown resource: ${resource}`);
      console.error(`   Valid resources: ${validResources.join(', ')}`);
      process.exit(1);
    }
  }
  
  // Print header
  printHeader('Resource Setup');
  
  const platforms = options.allPlatforms ? 'all' : 'current';
  console.log(`📦 Resources: ${options.resources.join(', ')}`);
  console.log(`🖥️  Platforms: ${platforms === 'all' ? 'all platforms' : getCurrentPlatform()}`);
  if (options.force) {
    console.log(`🔄 Force: re-downloading even if exists`);
  }
  
  // Setup each resource
  const results = {};
  let hasFailures = false;
  
  for (const resourceName of options.resources) {
    try {
      const result = await setupResource(resourceName, {
        platforms,
        force: options.force,
        verbose: true,
      });
      results[resourceName] = result;
      if (!result.success) {
        hasFailures = true;
      }
    } catch (error) {
      console.error(`\n❌ Failed to setup ${resourceName}: ${error.message}`);
      results[resourceName] = { success: false, error: error.message };
      hasFailures = true;
    }
  }
  
  // Print summary
  printSection('Setup Summary');
  
  for (const [name, result] of Object.entries(results)) {
    const status = result.success ? '✅' : '❌';
    console.log(`  ${status} ${name}`);
    
    if (result.platforms) {
      for (const [platform, platformResult] of Object.entries(result.platforms)) {
        const pStatus = platformResult.success ? '✓' : '✗';
        const extra = platformResult.skipped ? ' (already installed)' : '';
        console.log(`      ${pStatus} ${platform}${extra}`);
      }
    }
  }
  
  console.log('═'.repeat(60));
  
  if (hasFailures) {
    console.error('\n❌ Some resources failed to setup. Check errors above.');
    process.exit(1);
  }
  
  console.log(`
✅ Setup complete!

🚀 You can now build the app:
   npm run tauri:dev    # Development
   npm run tauri:build  # Production
`);
}

// Run
main().catch((error) => {
  console.error('❌ Setup failed:', error.message);
  process.exit(1);
});
