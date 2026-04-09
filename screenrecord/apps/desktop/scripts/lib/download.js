/**
 * ============================================================================
 * Download Utilities
 * ============================================================================
 * 
 * Functions for downloading files and extracting archives.
 * Supports HTTP/HTTPS with redirects, progress reporting, ZIP and tar.xz.
 */

const https = require('https');
const http = require('http');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const { ensureDir, isWindows } = require('./utils');

// =============================================================================
// Download Functions
// =============================================================================

/**
 * Download a file with redirect support and progress bar
 * @param {string} url - URL to download
 * @param {string} dest - Destination file path
 * @param {Object} options - Options
 * @param {boolean} options.showProgress - Show progress bar (default: true)
 * @param {string} options.userAgent - Custom user agent
 * @returns {Promise<void>}
 */
function downloadFile(url, dest, options = {}) {
  const { showProgress = true, userAgent = 'Mozilla/5.0 (compatible; ResourceManager/1.0)' } = options;
  
  return new Promise((resolve, reject) => {
    console.log(`  📥 Downloading: ${url}`);
    
    const makeRequest = (requestUrl, redirectCount = 0) => {
      if (redirectCount > 5) {
        reject(new Error('Too many redirects'));
        return;
      }
      
      const protocol = requestUrl.startsWith('https') ? https : http;
      
      protocol.get(requestUrl, {
        headers: { 'User-Agent': userAgent },
      }, (response) => {
        // Handle redirects (including 303 See Other)
        if (response.statusCode === 301 || response.statusCode === 302 || response.statusCode === 303 || response.statusCode === 307 || response.statusCode === 308) {
          const redirectUrl = response.headers.location;
          if (showProgress) {
            console.log(`  ↪️  Redirecting...`);
          }
          makeRequest(redirectUrl, redirectCount + 1);
          return;
        }
        
        if (response.statusCode !== 200) {
          reject(new Error(`HTTP ${response.statusCode}: ${response.statusMessage}`));
          return;
        }
        
        const totalSize = parseInt(response.headers['content-length'], 10);
        let downloadedSize = 0;
        
        ensureDir(path.dirname(dest));
        const file = fs.createWriteStream(dest);
        
        response.on('data', (chunk) => {
          downloadedSize += chunk.length;
          if (showProgress && totalSize) {
            const percent = ((downloadedSize / totalSize) * 100).toFixed(1);
            const mb = (downloadedSize / 1024 / 1024).toFixed(1);
            process.stdout.write(`\r  📥 Progress: ${percent}% (${mb} MB)    `);
          }
        });
        
        response.pipe(file);
        
        file.on('finish', () => {
          file.close();
          if (showProgress) {
            console.log(`\n  ✅ Downloaded: ${path.basename(dest)}`);
          }
          resolve();
        });
        
        file.on('error', (err) => {
          fs.unlink(dest, () => {});
          reject(err);
        });
      }).on('error', reject);
    };
    
    makeRequest(url);
  });
}

// =============================================================================
// Archive Extraction
// =============================================================================

/**
 * Extract a ZIP archive
 * @param {string} zipPath - Path to ZIP file
 * @param {string} extractTo - Destination directory
 */
function extractZip(zipPath, extractTo) {
  console.log(`  📦 Extracting ZIP archive...`);
  ensureDir(extractTo);
  
  try {
    // Try using unzip command (macOS/Linux)
    execSync(`unzip -q -o "${zipPath}" -d "${extractTo}"`, { stdio: 'pipe' });
  } catch {
    // Fallback for Windows
    try {
      execSync(`powershell -command "Expand-Archive -Path '${zipPath}' -DestinationPath '${extractTo}' -Force"`, { stdio: 'pipe' });
    } catch (winError) {
      throw new Error(`Failed to extract ZIP: ${winError.message}`);
    }
  }
  
  console.log(`  ✅ Extracted successfully`);
}

/**
 * Extract a tar.xz archive
 * @param {string} tarPath - Path to tar.xz file
 * @param {string} extractTo - Destination directory
 */
function extractTarXz(tarPath, extractTo) {
  console.log(`  📦 Extracting tar.xz archive...`);
  ensureDir(extractTo);
  
  try {
    execSync(`tar -xf "${tarPath}" -C "${extractTo}"`, { stdio: 'pipe' });
    console.log(`  ✅ Extracted successfully`);
  } catch (error) {
    throw new Error(`Failed to extract tar.xz: ${error.message}`);
  }
}

/**
 * Extract an archive (auto-detect type or use explicit type)
 * @param {string} archivePath - Path to archive
 * @param {string} extractTo - Destination directory
 * @param {string} type - Archive type: 'zip', 'tar.xz', or 'auto'
 */
function extractArchive(archivePath, extractTo, type = 'auto') {
  let archiveType = type;
  
  if (archiveType === 'auto') {
    if (archivePath.endsWith('.zip')) {
      archiveType = 'zip';
    } else if (archivePath.endsWith('.tar.xz') || archivePath.endsWith('.txz')) {
      archiveType = 'tar.xz';
    } else if (archivePath.endsWith('.tar.gz') || archivePath.endsWith('.tgz')) {
      archiveType = 'tar.gz';
    } else {
      throw new Error(`Unknown archive type for: ${archivePath}`);
    }
  }
  
  switch (archiveType) {
    case 'zip':
      extractZip(archivePath, extractTo);
      break;
    case 'tar.xz':
      extractTarXz(archivePath, extractTo);
      break;
    case 'tar.gz':
      console.log(`  📦 Extracting tar.gz archive...`);
      ensureDir(extractTo);
      execSync(`tar -xzf "${archivePath}" -C "${extractTo}"`, { stdio: 'pipe' });
      console.log(`  ✅ Extracted successfully`);
      break;
    default:
      throw new Error(`Unsupported archive type: ${archiveType}`);
  }
}

// =============================================================================
// Exports
// =============================================================================

module.exports = {
  downloadFile,
  extractZip,
  extractTarXz,
  extractArchive,
};
