## Getting Started

**First time setup:**
```bash
npm run setup-aw       # Download ActivityWatch binaries (only needed once)
npm run setup-ffmpeg   # Download ffmpeg binaries (only needed once)
npm run setup-whisper  # Download Whisper model for transcription (only needed once)
npm run tauri:dev      # Start development
```

Or with a custom port:
```bash
AW_PORT=5660 npm run tauri:dev
```

**Building for production:**
```bash
npm run tauri:build    # Builds app, then bundles ActivityWatch binaries
```

This automatically:
1. Checks that binaries exist (`check-aw`)
2. Builds the Tauri app (`tauri build`)
3. Copies binaries into the bundle (`bundle-resources`)
4. Signs the app bundle with entitlements (`sign-app`)

## ActivityWatch Binaries

### Automated Setup (Recommended)

Run the automated setup script to download and install ActivityWatch binaries for all platforms:

```bash
npm run setup-aw
npm run setup-ffmpeg
npm run setup-whisper
```

This will automatically:
- Download ActivityWatch v0.13.2 for macOS, Windows, and Linux
- Extract binaries to the correct directory structure
- Set executable permissions on Unix platforms

To use a different version:
```bash
npm run setup-aw -- --version v0.13.1
```

### Manual Setup (Alternative)

If the automated setup fails, you can manually download from https://activitywatch.net/downloads/ and place the **complete contents of each .zip** for each platform into:

- `src-tauri/resources/activitywatch/darwin/aarch64/` (macOS)
- `src-tauri/resources/activitywatch/windows/x86_64/` (Windows)
- `src-tauri/resources/activitywatch/linux/x86_64/` (Linux)

The app will automatically:
1. Start the `aw-server` from `platform/aw-server/aw-server`
2. Start the watchers from `platform/aw-watcher-*/aw-watcher-*`

Make binaries executable on Unix:
```bash
chmod +x src-tauri/resources/activitywatch/**/*/aw-*
```

## macOS Code Signing

### Ad-Hoc Signing (Development)

The app uses ad-hoc code signing for development and local use. This enables persistent accessibility permissions on macOS without requiring a paid Apple Developer account.

**What gets signed automatically:**
- Main Tauri application (signed during build)
- ActivityWatch server binary (`aw-server`)
- ActivityWatch watcher binaries (`aw-watcher-window`, `aw-watcher-afk`, `aw-watcher-input`)

**When signing occurs:**
1. **During setup** - `npm run setup-aw` signs downloaded binaries
2. **During build** - Tauri applies entitlements to the main app
3. **During bundling** - `npm run bundle-resources` signs binaries in the bundle
4. **Post-build** - `npm run sign-app` signs the entire .app bundle

**Limitations:**
- ✅ Works for local development and personal use
- ✅ Enables persistent accessibility permissions
- ❌ Cannot be distribute/notarized without paid Apple Developer ID

### Required Permissions

The app requires **Accessibility** permissions to monitor application usage and window activity.

**To grant permissions:**
1. Open **System Settings** > **Privacy & Security** > **Accessibility**
2. Click the **+** button or toggle switch for "ScreenJournal Tracker"
3. Grant permission when prompted

With proper code signing, this permission should **persist across app launches** and you will only need to grant it once.

### Troubleshooting Code Signing

**Verify app signature:**
```bash
codesign --verify --verbose /Applications/ScreenJournal\ Tracker.app
```

**Display signature information:**
```bash
codesign --display --verbose=4 /Applications/ScreenJournal\ Tracker.app
```

**Check entitlements:**
```bash
codesign --display --entitlements - /Applications/ScreenJournal\ Tracker.app
```

**Manually sign if needed:**
```bash
npm run sign-app
```

**Expected behavior after proper signing:**
- Permission dialog appears **once** on first launch
- No permission prompts on subsequent launches
- No `_CFBundleCreateUnique failed` errors in Console.app

## Bundled app layout (macOS and Windows)

The installer creates **one folder** that contains the desktop app and all backend services. The desktop app runs a **start script** in that folder to launch the servers (same idea on both platforms).

**Target layout after install:**

```
<InstallFolder>/
├── ScreenJournal Tracker.exe     (Windows)  or  ScreenJournal Tracker.app (macOS)
├── start-bundled.bat             (Windows)  or  start-bundled.sh (inside .app on macOS)
└── resources/                     (or Contents/Resources inside .app on macOS)
    ├── start-bundled.bat         (Windows)
    ├── start-bundled.sh          (macOS)
    ├── binaries/
    │   ├── sj-collector.exe      (Windows)  /  sj-collector (macOS)
    │   └── sj-tracker-report.exe (Windows)  /  sj-tracker-report (macOS)
    ├── python/
    │   └── sj-tracker-chat-agent/
    │       └── sj-chat-agent[.exe]
    ├── databases/
    │   ├── mongodb/<platform>/<arch>/
    │   └── influxdb/<platform>/<arch>/
    ├── frontend/
    │   └── sj-tracker-frontend/   (Next.js standalone or build)
    ├── activitywatch/
    │   └── <platform>/<arch>/     (aw-server, aw-watcher-*)
    ├── ffmpeg/
    │   └── <platform>/<arch>/
    └── whisper-tiny.en.bin        (Whisper model)
```

- The **desktop exe/app** resolves `resource_dir` to that `resources/` folder and runs `start-bundled.bat` (Windows) or `start-bundled.sh` (macOS) with `RESOURCE_DIR` and `APP_DATA_DIR` set.
- The **start script** starts MongoDB, InfluxDB, sj-collector, sj-tracker-report, chat agent, and (if available) the report frontend from `resources/`.

The root **build-bundled.sh** (repo root) and `tauri.conf.json` `bundle.resources` are set up so this layout is produced on both platforms.

## Production Distribution

When you build your app with `npm run tauri:build`, the ActivityWatch binaries are **automatically bundled** into the final application. End users who download your app will have all binaries included - they don't need to download anything separately.

**Workflow:**
1. **Developer (you):** Run `npm run setup-aw` once to download binaries
2. **Build:** Run `npm run tauri:build` - binaries get packaged into the app
3. **End user:** Downloads your app → binaries already included ✅

The `check-aw` script runs automatically before building to ensure binaries are present.

**Note on Distribution:** The current ad-hoc signing configuration works for local development and personal use only. To distribute the app to other users, you will need to:
1. Obtain a paid Apple Developer account ($99/year)
2. Update the `signingIdentity` in `tauri.conf.json` to use your Developer ID certificate
3. Configure notarization for macOS distribution

See `DISTRIBUTION.md` for detailed information on production distribution and code signing with Developer ID.