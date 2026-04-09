#!/usr/bin/env node

/**
 * ============================================================================
 * Release Script
 * ============================================================================
 * 
 * Automates version bumping and release tagging for the desktop app.
 * Updates version in tauri.conf.json and package.json, creates a git commit
 * and tag, and optionally pushes to trigger the CI release workflow.
 * 
 * USAGE:
 *   node scripts/release.js <version>           # Update versions locally
 *   node scripts/release.js <version> --push    # Update + push to trigger CI
 *   node scripts/release.js --help              # Show help
 * 
 * EXAMPLES:
 *   npm run release 1.0.0
 *   npm run release 1.2.0 --push
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// =============================================================================
// Configuration
// =============================================================================

const DESKTOP_DIR = path.join(__dirname, '..');
const TAURI_CONF_PATH = path.join(DESKTOP_DIR, 'src-tauri', 'tauri.conf.json');
const PACKAGE_JSON_PATH = path.join(DESKTOP_DIR, 'package.json');

// =============================================================================
// Helpers
// =============================================================================

/**
 * Print usage help
 */
function printHelp() {
  console.log(`
Usage: node scripts/release.js <version> [options]

Arguments:
  version     Semantic version (e.g., 1.0.0, 1.2.3)

Options:
  --push      Push commit and tag to remote (triggers CI release)
  --help      Show this help message

Examples:
  node scripts/release.js 1.0.0           # Update versions locally
  node scripts/release.js 1.2.0 --push    # Update and push to trigger CI
  npm run release 1.0.0                   # Via npm script
  npm run release 1.0.0 -- --push         # Via npm script with push
`);
}

/**
 * Validate semantic version format
 * @param {string} version - Version string
 * @returns {boolean}
 */
function isValidVersion(version) {
  return /^\d+\.\d+\.\d+$/.test(version);
}

/**
 * Read and parse JSON file
 * @param {string} filePath - Path to JSON file
 * @returns {Object}
 */
function readJson(filePath) {
  const content = fs.readFileSync(filePath, 'utf8');
  return JSON.parse(content);
}

/**
 * Write JSON file with pretty formatting
 * @param {string} filePath - Path to JSON file
 * @param {Object} data - Data to write
 */
function writeJson(filePath, data) {
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n');
}

/**
 * Execute a shell command and return output
 * @param {string} cmd - Command to execute
 * @param {Object} options - execSync options
 * @returns {string}
 */
function exec(cmd, options = {}) {
  return execSync(cmd, { encoding: 'utf8', stdio: 'pipe', ...options }).trim();
}

/**
 * Check if git working directory is clean
 * @returns {boolean}
 */
function isGitClean() {
  try {
    const status = exec('git status --porcelain');
    return status === '';
  } catch {
    return false;
  }
}

/**
 * Check if a git tag already exists
 * @param {string} tag - Tag name
 * @returns {boolean}
 */
function tagExists(tag) {
  try {
    exec(`git rev-parse ${tag}`);
    return true;
  } catch {
    return false;
  }
}

// =============================================================================
// Main
// =============================================================================

function main() {
  const args = process.argv.slice(2);
  
  // Parse arguments
  const shouldPush = args.includes('--push');
  const showHelp = args.includes('--help') || args.includes('-h');
  const version = args.find(arg => !arg.startsWith('--'));
  
  // Show help
  if (showHelp) {
    printHelp();
    process.exit(0);
  }
  
  // Validate version
  if (!version) {
    console.error('Error: Version argument is required');
    printHelp();
    process.exit(1);
  }
  
  if (!isValidVersion(version)) {
    console.error(`Error: Invalid version format "${version}"`);
    console.error('       Expected format: MAJOR.MINOR.PATCH (e.g., 1.0.0)');
    process.exit(1);
  }
  
  const tag = `v${version}`;
  
  console.log(`
╔════════════════════════════════════════════════════════════╗
║  Release Script                                            ║
╚════════════════════════════════════════════════════════════╝
`);
  
  console.log(`📦 Version: ${version}`);
  console.log(`🏷️  Tag: ${tag}`);
  console.log(`🚀 Push: ${shouldPush ? 'yes' : 'no'}\n`);
  
  // Check if tag already exists
  if (tagExists(tag)) {
    console.error(`Error: Tag ${tag} already exists`);
    console.error('       Use a different version number');
    process.exit(1);
  }
  
  // Update tauri.conf.json
  console.log('📝 Updating tauri.conf.json...');
  const tauriConf = readJson(TAURI_CONF_PATH);
  const oldTauriVersion = tauriConf.version;
  tauriConf.version = version;
  writeJson(TAURI_CONF_PATH, tauriConf);
  console.log(`   ${oldTauriVersion} → ${version}`);
  
  // Update package.json
  console.log('📝 Updating package.json...');
  const packageJson = readJson(PACKAGE_JSON_PATH);
  const oldPackageVersion = packageJson.version;
  packageJson.version = version;
  writeJson(PACKAGE_JSON_PATH, packageJson);
  console.log(`   ${oldPackageVersion} → ${version}`);
  
  // Git operations
  console.log('\n📋 Git operations:');
  
  // Stage changes
  console.log('   Staging changes...');
  exec(`git add "${TAURI_CONF_PATH}" "${PACKAGE_JSON_PATH}"`);
  
  // Create commit
  console.log('   Creating commit...');
  exec(`git commit -m "chore: release ${tag}"`);
  console.log(`   ✓ Committed: chore: release ${tag}`);
  
  // Create tag
  console.log('   Creating tag...');
  exec(`git tag ${tag}`);
  console.log(`   ✓ Tagged: ${tag}`);
  
  // Push if requested
  if (shouldPush) {
    console.log('\n🚀 Pushing to remote...');
    
    // Get current branch
    const branch = exec('git rev-parse --abbrev-ref HEAD');
    
    // Push commit
    console.log(`   Pushing ${branch}...`);
    exec(`git push origin ${branch}`);
    
    // Push tag
    console.log(`   Pushing tag ${tag}...`);
    exec(`git push origin ${tag}`);
    
    console.log(`
╔════════════════════════════════════════════════════════════╗
║  ✅ Release ${tag} pushed!                                  
╚════════════════════════════════════════════════════════════╝

🎉 CI workflow has been triggered!

📊 Monitor the build:
   (GitHub integration disabled in open source version)

📦 Release will be available at:
   (GitHub integration disabled in open source version)
`);
  } else {
    console.log(`
╔════════════════════════════════════════════════════════════╗
║  ✅ Release ${tag} prepared locally                         
╚════════════════════════════════════════════════════════════╝

📋 Changes committed and tagged locally.

🚀 To trigger the CI release, push with:
   git push origin $(git rev-parse --abbrev-ref HEAD) --tags

   Or run again with --push:
   npm run release ${version} -- --push
`);
  }
}

// Run
try {
  main();
} catch (error) {
  console.error(`\n❌ Release failed: ${error.message}`);
  process.exit(1);
}
