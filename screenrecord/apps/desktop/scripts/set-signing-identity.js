#!/usr/bin/env node
/**
 * Set Signing Identity in tauri.conf.json
 * 
 * Usage:
 *   node set-signing-identity.js "-"                    # Ad-hoc signing
 *   node set-signing-identity.js "Developer ID..."        # Developer ID
 */

const fs = require('fs');
const path = require('path');

const tauriConfigPath = path.join(__dirname, '..', 'src-tauri', 'tauri.conf.json');
const signingIdentity = process.argv[2];

if (!signingIdentity) {
  console.error('❌ Error: Signing identity not provided');
  console.error('Usage: node set-signing-identity.js "<identity>"');
  console.error('  Example: node set-signing-identity.js "-"');
  console.error('  Example: node set-signing-identity.js "Developer ID Application: ..."');
  process.exit(1);
}

try {
  const config = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf8'));
  
  if (!config.bundle || !config.bundle.macOS) {
    console.error('❌ Error: Invalid tauri.conf.json structure');
    process.exit(1);
  }
  
  const oldIdentity = config.bundle.macOS.signingIdentity;
  config.bundle.macOS.signingIdentity = signingIdentity;
  
  fs.writeFileSync(tauriConfigPath, JSON.stringify(config, null, 2) + '\n');
  
  console.log(`✅ Updated signing identity:`);
  console.log(`   Old: ${oldIdentity}`);
  console.log(`   New: ${signingIdentity}`);
} catch (error) {
  console.error(`❌ Error updating tauri.conf.json: ${error.message}`);
  process.exit(1);
}

