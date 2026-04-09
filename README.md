# ScreenRecord Open

A productivity tracking application that monitors your computer usage and generates insights about your work patterns.

## Prerequisites

Before running or building the application, ensure you have the following installed:

### Required Dependencies

- **Node.js** (v20 or higher) - [Download](https://nodejs.org/)
- **Go** (1.21 or higher) - [Download](https://golang.org/)
- **Python 3** (3.8 or higher) - [Download](https://www.python.org/)
- **Rust** - [Install via rustup](https://rustup.rs/)

### Platform-Specific Dependencies

#### macOS
- **Homebrew** (recommended for managing dependencies)
- **FFmpeg** - Install with: `brew install ffmpeg`
- **Xcode Command Line Tools** - Install with: `xcode-select --install`

#### Linux
- **Build tools**: `sudo apt-get install build-essential`
- **WebKit dependencies**: `sudo apt-get install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev`
- **OpenSSL development**: `sudo apt-get install libssl-dev`
- **FFmpeg**: `sudo apt-get install ffmpeg`
- **dpkg-dev** (for .deb packages): `sudo apt-get install dpkg-dev`

#### Windows
- **Visual Studio Build Tools** - Install "Desktop development with C++" workload
- **Git for Windows** - [Download](https://git-scm.com/download/win)

### Optional Dependencies

- **Docker & Docker Compose** - For running databases in development (optional, script can use local binaries)
- **OpenAI API Key** - For AI-powered chat agent features (optional)

## Development Mode

To run the application in development mode with all services:

```bash
./start-new.sh
```

This script will:
1. Check for Docker (optional - will use local database binaries if Docker is not available)
2. Start MongoDB and InfluxDB databases
3. Build and start Go backend services (sj-collector, sj-tracker-report)
4. Start Python chat agent
5. Build and start Next.js frontend
6. Launch the Tauri desktop app

All services will run in the foreground. Press `Ctrl+C` to stop all services.

### Development Mode Services

- **MongoDB**: `localhost:27017`
- **InfluxDB**: `localhost:8086`
- **Collector Backend**: `localhost:8080`
- **Report Backend**: `localhost:8085`
- **Chat Agent**: `localhost:8087`
- **Report Frontend**: `localhost:3030`
- **Desktop App**: Launches automatically

## Building Bundled Applications

The project includes platform-specific build scripts that create standalone, distributable applications containing all services.

### macOS Build

Creates a `.dmg` installer for macOS:

```bash
./build-bundled.sh
```

**Requirements:**
- macOS (Apple Silicon or Intel)
- All prerequisites listed above
- Database binaries will be downloaded automatically if needed

**Output:**
- DMG installer: `screenrecord/apps/desktop/src-tauri/target/release/bundle/dmg/ScreenRecord Tracker_*.dmg`

### Windows Build

Creates a Windows `.exe` installer or distribution zip:

```bash
./build-bundled-windows.sh
```

**Requirements:**
- **For native Windows builds**: Windows 10/11 with Visual Studio Build Tools
- **For cross-compilation from macOS**: 
  - `llvm` (for Windows resource compilation): `brew install llvm`
  - `mingw-w64` (for Windows GNU target): `brew install mingw-w64`
  - Note: NSIS installer generation requires Windows

**Output:**
- **On Windows**: NSIS installer at `screenrecord/apps/desktop/src-tauri/target/release/bundle/nsis/*.exe`
- **Cross-compiled from macOS**: Distribution zip at `screenrecord/apps/desktop/src-tauri/target/x86_64-pc-windows-gnu/ScreenRecord-Tracker-Windows-*.zip`

**Note**: Cross-compilation from macOS creates a zip file that users can extract and run. For a proper Windows installer, build on Windows directly.

### Linux Build

Creates a `.deb` package for Debian/Ubuntu:

```bash
./build-bundled-linux.sh
```

**Requirements:**
- **Must be run on Linux** (cross-compilation from macOS/Windows is not supported)
- All prerequisites listed above
- Linux-specific build dependencies (see Platform-Specific Dependencies above)

**Output:**
- DEB package: `screenrecord/apps/desktop/src-tauri/target/release/bundle/deb/*.deb`

**Installation:**
```bash
sudo dpkg -i screenrecord-tracker_*.deb
```

## Build Script Details

### What Each Build Script Does

All build scripts follow a similar process:

1. **Build Go Backends**: Compiles `sj-collector` and `sj-tracker-report` for the target platform
2. **Package Python Chat Agent**: Uses PyInstaller to create a standalone executable
3. **Build Frontend**: Creates Next.js standalone build
4. **Prepare Database Binaries**: Downloads MongoDB and InfluxDB binaries for the target platform
5. **Copy Resources**: Copies all binaries, Python executable, and frontend to Tauri resources
6. **Build Tauri App**: Compiles the desktop application
7. **Bundle Resources**: Copies all resources into the final bundle
8. **Create Installer**: Generates platform-specific installer (DMG, NSIS, or DEB)

### Build Output Locations

- **macOS**: `screenrecord/apps/desktop/src-tauri/target/release/bundle/dmg/`
- **Windows (native)**: `screenrecord/apps/desktop/src-tauri/target/release/bundle/nsis/`
- **Windows (cross-compiled)**: `screenrecord/apps/desktop/src-tauri/target/x86_64-pc-windows-gnu/`
- **Linux**: `screenrecord/apps/desktop/src-tauri/target/release/bundle/deb/`

## Additional Notes

- **Database Setup**: The bundled applications automatically set up InfluxDB on first run
- **Service Management**: All services start automatically when the bundled app launches
- **Logs**: Service logs are stored in the application data directory:
  - **macOS**: `~/Library/Application Support/com.screenrecord.tracker/`
  - **Windows**: `%APPDATA%\com.screenrecord.tracker\`
  - **Linux**: `~/.local/share/com.screenrecord.tracker/`

## Troubleshooting

### Build Fails with Missing Dependencies

Ensure all prerequisites are installed. For platform-specific issues:
- **macOS**: Check that Xcode Command Line Tools are installed
- **Linux**: Install all build dependencies listed above
- **Windows**: Ensure Visual Studio Build Tools are properly configured

### Services Don't Start in Bundled App

Check the logs in the application data directory (see Additional Notes above). Common issues:
- Port conflicts (another instance may be running)
- Missing database binaries
- Permission issues (especially on Linux)

### Cross-Compilation Issues

For best results:
- **macOS builds**: Build on macOS
- **Windows builds**: Build on Windows (cross-compilation from macOS is limited)
- **Linux builds**: Must build on Linux (cross-compilation not supported)
