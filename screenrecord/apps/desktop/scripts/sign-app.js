#!/usr/bin/env node

/**
 * Sign App Bundle Script
 * 
 * This script signs the complete .app bundle after Tauri build completes.
 * It ensures the entire app bundle is properly signed with entitlements.
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const { signDirectory, getSigningIdentity } = require('./lib/signing');
const { isMacOS } = require('./lib/utils');

// Parse command line arguments
const args = process.argv.slice(2);
const bundlePathArg = args.includes('--bundle-path') ? args[args.indexOf('--bundle-path') + 1] : null;
const skipVerify = args.includes('--skip-verify');

function findAppBundle() {
  // If bundle path provided, use it
  if (bundlePathArg) {
    if (fs.existsSync(bundlePathArg)) {
      return bundlePathArg;
    }
    console.error(`❌ Provided bundle path not found: ${bundlePathArg}`);
    return null;
  }

  // Otherwise, search in standard location
  const bundleDir = path.join(__dirname, '..', 'src-tauri', 'target', 'release', 'bundle', 'macos');
  
  if (!fs.existsSync(bundleDir)) {
    console.error('❌ macOS bundle directory not found');
    console.error(`   Expected at: ${bundleDir}`);
    return null;
  }

  const apps = fs.readdirSync(bundleDir).filter(f => f.endsWith('.app'));
  
  if (apps.length === 0) {
    console.error('❌ No .app bundle found in bundle directory');
    return null;
  }

  if (apps.length > 1) {
    console.warn(`⚠️  Multiple .app bundles found, using first: ${apps[0]}`);
  }

  return path.join(bundleDir, apps[0]);
}

function signAppBundle() {
  console.log(`
╔════════════════════════════════════════════════════════════╗
║  App Bundle Signing Script                                 ║
╚════════════════════════════════════════════════════════════╝
`);

  // Only run on macOS
  if (process.platform !== 'darwin') {
    console.log('ℹ️  Not on macOS, skipping app signing');
    return 0;
  }

  // Find the app bundle
  const appBundlePath = findAppBundle();
  if (!appBundlePath) {
    console.error('❌ Failed to locate app bundle');
    return 1;
  }

  console.log(`📦 Found app bundle: ${path.basename(appBundlePath)}`);
  console.log(`📍 Location: ${appBundlePath}\n`);

  // Find entitlements file
  const entitlementsPath = path.join(__dirname, '..', 'src-tauri', 'entitlements.plist');
  if (!fs.existsSync(entitlementsPath)) {
    console.error(`❌ Entitlements file not found: ${entitlementsPath}`);
    return 1;
  }

  console.log(`🔑 Using entitlements: ${entitlementsPath}\n`);

  // Get signing identity (Developer ID or ad-hoc)
  const signingIdentity = getSigningIdentity();
  const identityFlag = signingIdentity === "-" ? "-" : `"${signingIdentity}"`;
  
  if (signingIdentity !== "-") {
    console.log(`🔐 Using Developer ID certificate: ${signingIdentity}\n`);
  } else {
    console.log(`⚠️  Using ad-hoc signing (no Developer ID certificate found)\n`);
  }

  // Sign all resource directories explicitly BEFORE signing the app bundle
  // This ensures all binaries are properly signed with hardened runtime and timestamps
  // Required for notarization - all nested binaries must be signed individually
  const resourcesPath = path.join(appBundlePath, 'Contents', 'Resources');
  const additionalResources = ['binaries', 'python', 'databases', 'activitywatch', 'ffmpeg'];
  
  console.log('🔏 Signing all resource directories...');
  for (const resourceDir of additionalResources) {
    const resourcePath = path.join(resourcesPath, resourceDir);
    if (fs.existsSync(resourcePath)) {
      console.log(`  📦 Signing ${resourceDir}...`);
      const signResults = signDirectory(resourcePath, entitlementsPath, { 
        verbose: true,
        signingIdentity: signingIdentity 
      });
      if (signResults.success > 0) {
        console.log(`  ✓ Signed ${signResults.success} binaries in ${resourceDir}`);
      }
      if (signResults.failed > 0) {
        console.warn(`  ⚠️  ${signResults.failed} binaries failed to sign in ${resourceDir}`);
      }
    } else {
      console.log(`  ⊘ ${resourceDir} not found, skipping...`);
    }
  }
  console.log('');

  // Sign the main executable
  const mainExecutable = path.join(appBundlePath, 'Contents', 'MacOS', path.basename(appBundlePath, '.app'));
  if (fs.existsSync(mainExecutable)) {
    console.log('🔏 Signing main executable...');
    try {
      const { signBinary } = require('./lib/signing');
      const result = signBinary(mainExecutable, entitlementsPath, signingIdentity);
      if (result.success) {
        console.log('✅ Main executable signed successfully!\n');
      } else {
        console.error(`❌ Failed to sign main executable: ${result.error}`);
        return 1;
      }
    } catch (error) {
      console.error('❌ Failed to sign main executable:', error.message);
      return 1;
    }
  }

  // Sign the app bundle (without --deep, since we've signed everything explicitly)
  // For notarization, we need hardened runtime and timestamps
  try {
    console.log('🔏 Signing app bundle...');
    const isDeveloperId = signingIdentity !== "-";
    const runtimeOptions = isDeveloperId ? "--options runtime" : "";
    const timestamp = isDeveloperId ? "--timestamp" : "";
    
    const codesignCmd = [
      "codesign",
      "--force",
      "--sign", identityFlag,
      "--entitlements", `"${entitlementsPath}"`,
      runtimeOptions,
      timestamp,
      `"${appBundlePath}"`
    ].filter(Boolean).join(" ");
    
    execSync(codesignCmd, { stdio: 'inherit' });
    console.log('✅ App bundle signed successfully!\n');
  } catch (error) {
    console.error('❌ Failed to sign app bundle:', error.message);
    return 1;
  }

  // Verify signature (unless skipped)
  if (!skipVerify) {
    try {
      console.log('🔍 Verifying signature...');
      execSync(`codesign --verify --verbose "${appBundlePath}"`, { stdio: 'inherit' });
      console.log('✅ Signature verified!\n');
    } catch (error) {
      console.error('❌ Signature verification failed:', error.message);
      return 1;
    }

    // Check for unsigned binaries (critical for notarization)
    try {
      console.log('🔍 Checking for unsigned binaries...');
      const checkCmd = `codesign --verify --deep --strict --verbose=2 "${appBundlePath}" 2>&1 || true`;
      const checkOutput = execSync(checkCmd, { encoding: 'utf8' });
      
      // Look for warnings about unsigned files
      if (checkOutput.includes('unsigned') || checkOutput.includes('not signed')) {
        console.warn('⚠️  Warning: Some binaries may not be properly signed');
        console.warn('   This may cause notarization to fail');
      } else {
        console.log('✅ All binaries appear to be signed\n');
      }
    } catch (error) {
      console.warn('⚠️  Could not verify all binaries');
    }

    // Display signature information
    try {
      console.log('📋 Signature details:');
      execSync(`codesign --display --verbose=4 "${appBundlePath}"`, { stdio: 'inherit' });
      console.log('');
    } catch (error) {
      console.warn('⚠️  Could not display signature details');
    }
  }

  console.log(`
╔════════════════════════════════════════════════════════════╗
║  ✅ Signing Complete!                                      ║
╚════════════════════════════════════════════════════════════╝

📦 Signed bundle: ${appBundlePath}

🔒 Your app is now properly signed with entitlements.
   Accessibility permissions should persist across launches.

💡 To verify signature manually:
   codesign --verify --verbose "${appBundlePath}"
   codesign --display --entitlements - "${appBundlePath}"
`);

  return 0;
}

// Run the script
if (require.main === module) {
  const exitCode = signAppBundle();
  process.exit(exitCode);
}

module.exports = { signAppBundle, findAppBundle };


