/**
 * ============================================================================
 * Code Signing Utilities
 * ============================================================================
 * 
 * macOS code signing utilities for bundled binaries.
 * Uses Developer ID certificate when available, falls back to ad-hoc signing.
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const { isMacOS, fileExists } = require('./utils');

// =============================================================================
// Certificate Detection
// =============================================================================

/**
 * Get the signing identity to use (Developer ID or ad-hoc)
 * @returns {string} Signing identity name or "-" for ad-hoc
 */
function getSigningIdentity() {
  // Check for Developer ID certificate
  const developerIdCert = "Developer ID Application: Chomtana CHANJARASWICHAI (2N4Z8N5N6A)";
  
  try {
    // Try to find the certificate in any accessible keychain
    // This works for both local (login keychain) and CI (temporary keychain)
    const result = execSync(
      `security find-identity -v -p codesigning 2>/dev/null | grep "${developerIdCert}" || true`,
      { encoding: 'utf8', stdio: 'pipe' }
    );
    
    if (result.trim().includes(developerIdCert)) {
      return developerIdCert;
    }
    
    // Fall back to ad-hoc signing if certificate not found
    return "-";
  } catch (error) {
    // Fall back to ad-hoc signing if certificate not found
    return "-";
  }
}

// =============================================================================
// Signing Functions
// =============================================================================

/**
 * Sign a single binary (macOS only)
 * @param {string} binaryPath - Path to binary
 * @param {string} entitlementsPath - Path to entitlements.plist
 * @param {string} signingIdentity - Optional signing identity (defaults to auto-detect)
 * @returns {{ success: boolean, error?: string }}
 */
function signBinary(binaryPath, entitlementsPath, signingIdentity = null) {
  // Only sign on macOS
  if (!isMacOS()) {
    return { success: true, skipped: true };
  }
  
  // Verify binary exists
  if (!fileExists(binaryPath)) {
    return { success: false, error: `Binary not found: ${binaryPath}` };
  }
  
  // Skip symlinks - they'll be handled when signing the framework or actual binary
  if (isSymlink(binaryPath)) {
    return { success: true, skipped: true };
  }
  
  // Verify entitlements exists
  if (!fileExists(entitlementsPath)) {
    return { success: false, error: `Entitlements not found: ${entitlementsPath}` };
  }
  
  // Get signing identity if not provided
  const identity = signingIdentity || getSigningIdentity();
  const identityFlag = identity === "-" ? "-" : `"${identity}"`;
  
  // For notarization, we need:
  // - Hardened runtime (--options runtime)
  // - Secure timestamp (--timestamp)
  // - No --deep (deprecated, sign nested binaries separately)
  const isDeveloperId = identity !== "-";
  const runtimeOptions = isDeveloperId ? "--options runtime" : "";
  const timestamp = isDeveloperId ? "--timestamp" : "";
  
  // For frameworks, we need to sign them as bundles
  const isFrameworkBundle = isFramework(binaryPath);
  const deepFlag = isFrameworkBundle ? "--deep" : "";
  
  try {
    // Build codesign command with required flags for notarization
    const codesignCmd = [
      "codesign",
      "--force",
      deepFlag,
      "--sign", identityFlag,
      "--entitlements", `"${entitlementsPath}"`,
      runtimeOptions,
      timestamp,
      `"${binaryPath}"`
    ].filter(Boolean).join(" ");
    
    execSync(codesignCmd, { stdio: 'pipe' });
    return { success: true };
  } catch (error) {
    return { success: false, error: error.message };
  }
}

/**
 * Recursively find all files in a directory
 */
function findAllFiles(dir, fileList = []) {
  if (!fs.existsSync(dir)) {
    return fileList;
  }
  
  const files = fs.readdirSync(dir);
  for (const file of files) {
    const filePath = path.join(dir, file);
    try {
      const stat = fs.statSync(filePath);
      if (stat.isDirectory()) {
        // Skip .git and other hidden/system directories
        if (!file.startsWith('.')) {
          findAllFiles(filePath, fileList);
        }
      } else if (stat.isFile()) {
        fileList.push(filePath);
      }
    } catch (e) {
      // Skip files we can't access
    }
  }
  return fileList;
}

/**
 * Check if a file is a symlink
 */
function isSymlink(filePath) {
  try {
    const stat = fs.lstatSync(filePath);
    return stat.isSymbolicLink();
  } catch (e) {
    return false;
  }
}

/**
 * Check if a path is a framework bundle
 */
function isFramework(filePath) {
  return filePath.endsWith('.framework') && fs.statSync(filePath).isDirectory();
}

/**
 * Check if a file is a Mach-O binary that needs signing
 */
function isBinaryFile(filePath) {
  try {
    // Skip symlinks - they'll be handled when we sign the framework or actual binary
    if (isSymlink(filePath)) {
      return false;
    }
    
    // Check by extension first (fast)
    const ext = path.extname(filePath);
    const basename = path.basename(filePath);
    if (ext === '.dylib' || ext === '.so') {
      return true;
    }
    
    // Skip Python.framework/Python symlinks - we'll sign the actual binary and framework
    if (basename === 'Python' && filePath.includes('.framework')) {
      // Check if it's the actual binary in Versions/X.X/Python, not a symlink
      if (!filePath.includes('/Versions/')) {
        return false; // This is likely a symlink
      }
    }
    
    // Check if it has execute permissions
    const stat = fs.statSync(filePath);
    if (stat.mode & parseInt('111', 8)) {
      // Has execute bit, check if it's actually a binary
      try {
        const fileOutput = execSync(`file -b "${filePath}"`, { encoding: 'utf8', stdio: 'pipe' });
        return fileOutput.includes('Mach-O');
      } catch (e) {
        // If file command fails, assume it's a binary if it has execute bit
        return true;
      }
    }
    
    // Check using file command for other potential binaries
    try {
      const fileOutput = execSync(`file -b "${filePath}"`, { encoding: 'utf8', stdio: 'pipe' });
      return fileOutput.includes('Mach-O');
    } catch (e) {
      return false;
    }
  } catch (e) {
    return false;
  }
}

/**
 * Sign all executables in a directory (macOS only)
 * @param {string} dir - Directory containing binaries
 * @param {string} entitlementsPath - Path to entitlements.plist
 * @param {Object} options - Options
 * @param {boolean} options.verbose - Log each file being signed
 * @returns {{ success: number, failed: number, skipped: number }}
 */
function signDirectory(dir, entitlementsPath, options = {}) {
  const { verbose = false, signingIdentity = null } = options;
  const results = { success: 0, failed: 0, skipped: 0 };
  
  // Only sign on macOS
  if (!isMacOS()) {
    return results;
  }
  
  // Verify directory exists
  if (!fileExists(dir)) {
    console.warn(`  ⚠ Directory not found: ${dir}`);
    return results;
  }
  
  // Verify entitlements exists
  if (!fileExists(entitlementsPath)) {
    console.warn(`  ⚠ Entitlements not found: ${entitlementsPath}`);
    return results;
  }
  
  // Get signing identity once for all binaries in this directory
  const identity = signingIdentity || getSigningIdentity();
  if (verbose && identity !== "-") {
    console.log(`  🔑 Using signing identity: ${identity}`);
  }
  
  try {
    // Find all files recursively
    const allFiles = findAllFiles(dir);
    
    // Filter to only binaries
    const binaries = allFiles.filter(filePath => {
      // Skip non-binary files by extension
      const ext = path.extname(filePath);
      const basename = path.basename(filePath);
      if (['.json', '.txt', '.md', '.pyc', '.py', '.plist', '.html', '.css', '.js', '.ts'].includes(ext)) {
        return false;
      }
      // Check if it's a binary
      return isBinaryFile(filePath);
    });
    
    // Find and sign frameworks separately (they need special handling)
    const frameworks = [];
    const regularBinaries = [];
    const frameworkPaths = new Set();
    
    // First, find all .framework directories
    try {
      const frameworkFind = `find "${dir}" -type d -name "*.framework" 2>/dev/null`;
      const frameworkOutput = execSync(frameworkFind, { encoding: 'utf8' });
      const foundFrameworks = frameworkOutput.trim().split('\n').filter(Boolean);
      foundFrameworks.forEach(f => frameworkPaths.add(f));
    } catch (e) {
      // Ignore find errors
    }
    
    // Also check binaries to see if they're inside frameworks
    for (const filePath of binaries) {
      // Check if this is inside a framework
      const frameworkMatch = filePath.match(/(.+\.framework)/);
      if (frameworkMatch) {
        frameworkPaths.add(frameworkMatch[1]);
      } else if (!frameworkPaths.has(filePath)) {
        // Check if this file is inside any known framework
        let isInFramework = false;
        for (const fwPath of frameworkPaths) {
          if (filePath.startsWith(fwPath + path.sep)) {
            isInFramework = true;
            break;
          }
        }
        if (!isInFramework) {
          regularBinaries.push(filePath);
        }
      }
    }
    
    frameworks.push(...Array.from(frameworkPaths));
    
    // Sort to sign in consistent order
    regularBinaries.sort();
    frameworks.sort();
    
    if (verbose) {
      console.log(`  📋 Found ${regularBinaries.length} binaries and ${frameworks.length} frameworks to sign`);
    }
    
    // First, sign all regular binaries
    for (const binaryPath of regularBinaries) {
      try {
        const result = signBinary(binaryPath, entitlementsPath, identity);
      
      if (result.skipped) {
        results.skipped++;
      } else if (result.success) {
        if (verbose) {
            console.log(`  ✓ Signed: ${path.relative(dir, binaryPath)}`);
          }
          results.success++;
        } else {
          if (verbose) {
            console.warn(`  ⚠ Failed: ${path.relative(dir, binaryPath)}: ${result.error}`);
          }
          results.failed++;
        }
      } catch (err) {
        if (verbose) {
          console.warn(`  ⚠ Error signing ${path.relative(dir, binaryPath)}: ${err.message}`);
        }
        results.failed++;
      }
    }
    
    // Then sign frameworks (sign the actual binary first, then the framework bundle)
    for (const frameworkPath of frameworks) {
      try {
        const frameworkName = path.basename(frameworkPath, '.framework');
        
        // Remove ALL existing signatures from the entire framework structure first
        // This ensures we start with a clean slate
        try {
          execSync(`codesign --remove-signature "${frameworkPath}" 2>/dev/null || true`, { stdio: 'pipe' });
        } catch (e) {
          // Ignore errors
        }
        
        // Remove signatures from symlinks (they can't be signed directly)
        const symlinkPaths = [
          path.join(frameworkPath, frameworkName),
          path.join(frameworkPath, 'Versions', 'Current', frameworkName),
          path.join(frameworkPath, 'Versions', 'Current')
        ];
        
        for (const symlinkPath of symlinkPaths) {
          if (fs.existsSync(symlinkPath)) {
            try {
              // Remove signature from symlink if it exists (symlinks can't be signed)
              execSync(`codesign --remove-signature "${symlinkPath}" 2>/dev/null || true`, { stdio: 'pipe' });
            } catch (e) {
              // Ignore errors - symlink might not have a signature
            }
          }
        }
        
        // Find and sign the actual Python binary in Versions/X.X/Python
        const versionsDir = path.join(frameworkPath, 'Versions');
        let actualBinaryPath = null;
        if (fs.existsSync(versionsDir)) {
          const versions = fs.readdirSync(versionsDir);
          for (const version of versions) {
            const versionPath = path.join(versionsDir, version);
            if (fs.statSync(versionPath).isDirectory()) {
              const binaryPath = path.join(versionPath, frameworkName);
              if (fs.existsSync(binaryPath) && !isSymlink(binaryPath)) {
                actualBinaryPath = binaryPath;
                // Sign the actual binary first
                const result = signBinary(binaryPath, entitlementsPath, identity);
                if (result.success) {
                  if (verbose) {
                    console.log(`  ✓ Signed framework binary: ${path.relative(dir, binaryPath)}`);
                    
                    // Verify the binary is signed for all architectures
                    try {
                      const verifyOutput = execSync(`codesign --verify --verbose "${binaryPath}" 2>&1`, { encoding: 'utf8' });
                      if (verbose) {
                        console.log(`  📋 Binary signature info: ${verifyOutput.split('\n')[0]}`);
                      }
                      
                      // Check architectures
                      const lipoOutput = execSync(`lipo -info "${binaryPath}" 2>&1`, { encoding: 'utf8' });
                      if (verbose) {
                        console.log(`  📋 Binary architectures: ${lipoOutput.trim()}`);
                      }
                    } catch (e) {
                      // Ignore verification errors
                    }
        }
        results.success++;
      } else {
                  if (verbose) {
                    console.warn(`  ⚠ Failed to sign framework binary: ${path.relative(dir, binaryPath)}: ${result.error}`);
                  }
                  results.failed++;
                }
                break; // Only sign the first actual binary we find
              }
            }
          }
        }
        
        if (!actualBinaryPath) {
          if (verbose) {
            console.warn(`  ⚠ Could not find actual binary in framework: ${frameworkPath}`);
          }
          results.failed++;
          continue;
        }
        
        // Sign the framework bundle itself
        // For frameworks, we need to sign all nested binaries first, then the framework
        // But we must NOT sign symlinks - they inherit signatures from what they point to
        const identityFlag = identity === "-" ? "-" : `"${identity}"`;
        const isDeveloperId = identity !== "-";
        const runtimeOptions = isDeveloperId ? "--options runtime" : "";
        const timestamp = isDeveloperId ? "--timestamp" : "";
        
        try {
          // First, sign any other binaries in the framework (not symlinks)
          const frameworkBinaries = findAllFiles(frameworkPath).filter(f => {
            if (isSymlink(f)) return false;
            return isBinaryFile(f) && f !== actualBinaryPath;
          });
          
          for (const fwBinary of frameworkBinaries) {
            try {
              const result = signBinary(fwBinary, entitlementsPath, identity);
              if (result.success && verbose) {
                console.log(`  ✓ Signed framework component: ${path.relative(dir, fwBinary)}`);
              }
            } catch (e) {
              // Ignore errors for framework components
            }
          }
          
          // Now sign the framework bundle itself
          // Do NOT use --deep - we've already signed the actual binary
          // Using --deep would try to sign symlinks, which causes invalid signatures
          // The framework bundle signature will reference the signed binary
          const frameworkCmd = [
            "codesign",
            "--force",
            "--sign", identityFlag,
            "--entitlements", `"${entitlementsPath}"`,
            runtimeOptions,
            timestamp,
            `"${frameworkPath}"`
          ].filter(Boolean).join(" ");
          
          execSync(frameworkCmd, { stdio: verbose ? 'inherit' : 'pipe' });
          
          // CRITICAL: After signing, recreate symlinks to ensure they're completely clean
          // Symlinks should NEVER have signatures - they inherit from what they point to
          // Recreating them ensures no signatures are attached
          try {
            // Find which version directory contains the actual binary
            const versionsDir = path.join(frameworkPath, 'Versions');
            let versionName = null;
            if (fs.existsSync(versionsDir)) {
              const versions = fs.readdirSync(versionsDir);
              for (const version of versions) {
                const versionPath = path.join(versionsDir, version);
                if (fs.statSync(versionPath).isDirectory()) {
                  const binaryPath = path.join(versionPath, frameworkName);
                  if (fs.existsSync(binaryPath) && !isSymlink(binaryPath) && binaryPath === actualBinaryPath) {
                    versionName = version;
                    break;
                  }
                }
              }
            }
            
            if (!versionName) {
              if (verbose) {
                console.warn(`  ⚠ Could not determine version for framework symlinks`);
              }
            } else {
              // Recreate Versions/Current symlink
              const currentSymlink = path.join(frameworkPath, 'Versions', 'Current');
              if (fs.existsSync(currentSymlink)) {
                fs.unlinkSync(currentSymlink);
              }
              fs.symlinkSync(versionName, currentSymlink);
              
              // Recreate main framework symlink
              const mainSymlink = path.join(frameworkPath, frameworkName);
              if (fs.existsSync(mainSymlink)) {
                fs.unlinkSync(mainSymlink);
              }
              fs.symlinkSync(`Versions/Current/${frameworkName}`, mainSymlink);
              
              if (verbose) {
                console.log(`  ✓ Recreated framework symlinks pointing to Versions/${versionName}`);
              }
              
              // Verify symlinks have no signatures (they shouldn't)
              const symlinksToCheck = [mainSymlink, currentSymlink];
              for (const symlinkPath of symlinksToCheck) {
                try {
                  // Try to verify - if it succeeds, it has a signature (bad)
                  execSync(`codesign --verify "${symlinkPath}" 2>&1`, { stdio: 'pipe' });
                  // If we get here, it has a signature - remove it
                  execSync(`codesign --remove-signature "${symlinkPath}" 2>/dev/null || true`, { stdio: 'pipe' });
                  if (verbose) {
                    console.warn(`  ⚠ Removed signature from symlink: ${path.basename(symlinkPath)}`);
                  }
                } catch (e) {
                  // Good - verification failed means no signature (expected for symlinks)
                }
              }
            }
          } catch (symlinkErr) {
            if (verbose) {
              console.warn(`  ⚠ Could not recreate symlinks: ${symlinkErr.message}`);
            }
          }
          
          // Verify the framework signature
          try {
            execSync(`codesign --verify --verbose "${frameworkPath}" 2>&1`, { stdio: 'pipe' });
            if (verbose) {
              console.log(`  ✓ Signed framework bundle: ${path.relative(dir, frameworkPath)}`);
            }
            results.success++;
          } catch (verifyErr) {
            if (verbose) {
              console.warn(`  ⚠ Framework signed but verification failed: ${path.relative(dir, frameworkPath)}`);
              console.warn(`     Error: ${verifyErr.message}`);
            }
            // Still count as success if signing worked, verification might fail for other reasons
            results.success++;
          }
        } catch (err) {
          if (verbose) {
            console.warn(`  ⚠ Failed to sign framework bundle: ${path.relative(dir, frameworkPath)}: ${err.message}`);
          }
          results.failed++;
        }
      } catch (err) {
        if (verbose) {
          console.warn(`  ⚠ Error signing framework ${path.relative(dir, frameworkPath)}: ${err.message}`);
        }
        results.failed++;
      }
    }
  } catch (error) {
    console.warn(`  ⚠ Failed to sign directory: ${error.message}`);
  }
  
  return results;
}

/**
 * Sign specific binaries by path list (macOS only)
 * @param {string[]} binaryPaths - Array of binary paths to sign
 * @param {string} entitlementsPath - Path to entitlements.plist
 * @param {Object} options - Options
 * @param {boolean} options.verbose - Log each file being signed
 * @returns {{ success: number, failed: number, skipped: number }}
 */
function signBinaries(binaryPaths, entitlementsPath, options = {}) {
  const { verbose = false, signingIdentity = null } = options;
  const results = { success: 0, failed: 0, skipped: 0 };
  
  // Only sign on macOS
  if (!isMacOS()) {
    return results;
  }
  
  // Get signing identity once for all binaries
  const identity = signingIdentity || getSigningIdentity();
  if (verbose && identity !== "-") {
    console.log(`  🔑 Using signing identity: ${identity}`);
  }
  
  for (const binaryPath of binaryPaths) {
    if (!fileExists(binaryPath)) {
      if (verbose) {
        console.log(`  ⊘ Skipped: ${path.basename(binaryPath)} (not found)`);
      }
      results.skipped++;
      continue;
    }
    
    const result = signBinary(binaryPath, entitlementsPath, identity);
    
    if (result.success) {
      if (verbose) {
        console.log(`  ✓ Signed: ${path.basename(binaryPath)}`);
      }
      results.success++;
    } else {
      if (verbose) {
        console.warn(`  ⚠ Failed: ${path.basename(binaryPath)}: ${result.error}`);
      }
      results.failed++;
    }
  }
  
  return results;
}

// =============================================================================
// Exports
// =============================================================================

module.exports = {
  signBinary,
  signDirectory,
  signBinaries,
  getSigningIdentity,
};
