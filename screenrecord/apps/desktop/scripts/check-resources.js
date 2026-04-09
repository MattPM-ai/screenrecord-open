#!/usr/bin/env node

/**
 * ============================================================================
 * Resource Check Script
 * ============================================================================
 * 
 * Verifies that external binary resources are properly installed.
 * Runs before builds to catch missing dependencies early.
 * 
 * USAGE:
 *   node scripts/check-resources.js                  # Check all resources for current platform
 *   node scripts/check-resources.js activitywatch    # Check only ActivityWatch
 *   node scripts/check-resources.js ffmpeg           # Check only FFmpeg
 *   node scripts/check-resources.js --verbose        # Show all platforms status
 *   node scripts/check-resources.js --all            # Check all platforms (CI mode)
 * 
 * ENVIRONMENT VARIABLES:
 *   SKIP_AW_CHECK=true      Skip ActivityWatch check
 *   SKIP_FFMPEG_CHECK=true  Skip FFmpeg check
 * 
 */

const { checkResource, checkAllResources, getAllResourceNames, getResource } = require('./lib/resource-manager');
const { printHeader, getCurrentPlatform, getAllPlatforms } = require('./lib/utils');

// =============================================================================
// CLI Argument Parsing
// =============================================================================

function parseArgs() {
  const args = process.argv.slice(2);
  
  const options = {
    resources: [],
    allPlatforms: args.includes('--all'),
    verbose: args.includes('--verbose') || args.includes('-v'),
  };
  
  // Get resource names (non-flag arguments)
  for (const arg of args) {
    if (!arg.startsWith('--') && !arg.startsWith('-')) {
      options.resources.push(arg);
    }
  }
  
  // If no resources specified, check all
  if (options.resources.length === 0) {
    options.resources = getAllResourceNames();
  }
  
  return options;
}

// =============================================================================
// Output Formatting
// =============================================================================

function printMissingResourceError(resourceName, result) {
  const resource = getResource(resourceName);
  const currentPlatform = getCurrentPlatform();
  
  console.error(`
╔${'═'.repeat(60)}╗
║  ❌ ${resource.name} Not Installed${' '.repeat(60 - resource.name.length - 24)}║
╚${'═'.repeat(60)}╝

${resource.name} binaries are required but not found.

To install ${resource.name}:

  npm run setup-${resourceName === 'activitywatch' ? 'aw' : resourceName}

For all platforms (CI/CD):

  npm run setup-${resourceName === 'activitywatch' ? 'aw' : resourceName} -- --all

`);
}

// =============================================================================
// Main
// =============================================================================

function main() {
  const options = parseArgs();
  
  // Validate resource names
  const validResources = getAllResourceNames();
  for (const resource of options.resources) {
    if (!validResources.includes(resource)) {
      console.error(`❌ Unknown resource: ${resource}`);
      console.error(`   Valid resources: ${validResources.join(', ')}`);
      process.exit(1);
    }
  }
  
  const platforms = options.allPlatforms ? 'all' : 'current';
  const currentPlatform = getCurrentPlatform();
  
  console.log(`🔍 Checking resources for ${platforms === 'all' ? 'all platforms' : currentPlatform}...\n`);
  
  // Check each resource
  let allSuccess = true;
  const results = {};
  
  for (const resourceName of options.resources) {
    const resource = getResource(resourceName);
    
    // Check skip env var
    if (process.env[resource.skipEnvVar] === 'true') {
      console.log(`⚠️  ${resource.skipEnvVar} is set, skipping ${resource.name} check`);
      results[resourceName] = { success: true, skipped: true };
      continue;
    }
    
    const result = checkResource(resourceName, {
      platforms,
      verify: true,
      verbose: options.verbose,
    });
    
    results[resourceName] = result;
    
    if (!result.success) {
      allSuccess = false;
    }
    
    // Print result for this resource
    if (result.success) {
      const platformResults = result.platforms;
      const platformKeys = Object.keys(platformResults);
      
      // For single platform check, show version if available
      if (platformKeys.length === 1) {
        const pr = platformResults[platformKeys[0]];
        const versionStr = pr.version ? ` (${pr.version.substring(0, 40)})` : '';
        console.log(`✅ ${resource.name}${versionStr}`);
      } else {
        // Multi-platform
        console.log(`✅ ${resource.name}:`);
        for (const [platform, pr] of Object.entries(platformResults)) {
          const status = pr.exists ? '✓' : '✗';
          console.log(`   ${status} ${platform}`);
        }
      }
    } else {
      console.log(`❌ ${resource.name}: Missing`);
      
      if (options.verbose) {
        const platformResults = result.platforms;
        for (const [platform, pr] of Object.entries(platformResults)) {
          const status = pr.exists ? '✓' : '✗';
          console.log(`   ${status} ${platform}`);
        }
      }
    }
  }
  
  console.log('');
  
  // Exit with error if any resource is missing
  if (!allSuccess) {
    // Find first missing resource to show detailed error
    for (const [resourceName, result] of Object.entries(results)) {
      if (!result.success && !result.skipped) {
        printMissingResourceError(resourceName, result);
        break;
      }
    }
    process.exit(1);
  }
  
  console.log('✅ All resource checks passed!\n');
}

// Run
try {
  main();
} catch (error) {
  console.error('❌ Check failed:', error.message);
  process.exit(1);
}
