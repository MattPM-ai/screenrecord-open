#!/bin/bash

# Build script for creating a Windows installer (.exe) for ScreenJournal Tracker
# This script can be run from macOS, Linux, or Windows to build Windows binaries
# Note: Final NSIS installer generation works best on Windows, but binaries can be cross-compiled

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo -e "${GREEN}🔨 Building Windows Bundled ScreenJournal Application${NC}"
echo ""

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check prerequisites
echo -e "${YELLOW}📋 Checking prerequisites...${NC}"

if ! command_exists go; then
    echo -e "${RED}❌ Go is not installed. Please install Go first.${NC}"
    exit 1
fi

if ! command_exists node; then
    echo -e "${RED}❌ Node.js is not installed. Please install Node.js first.${NC}"
    exit 1
fi

if ! command_exists python3; then
    echo -e "${RED}❌ Python 3 is not installed. Please install Python 3 first.${NC}"
    exit 1
fi

echo -e "${GREEN}✅ All prerequisites met${NC}"
echo ""

# Create temporary build directory
BUILD_DIR="$SCRIPT_DIR/dist-bundled-windows"
echo -e "${YELLOW}📁 Creating build directory: $BUILD_DIR${NC}"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/binaries"
mkdir -p "$BUILD_DIR/python"
echo -e "${GREEN}✅ Build directory created${NC}"
echo ""

# Build sj-collector for Windows (cross-compilation)
echo -e "${BLUE}🔧 Building sj-collector backend for Windows...${NC}"
cd sj-collector
GOOS=windows GOARCH=amd64 go build -o "$BUILD_DIR/binaries/sj-collector.exe" ./cmd/server
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ sj-collector built successfully${NC}"
else
    echo -e "${RED}❌ Failed to build sj-collector${NC}"
    exit 1
fi
cd ..
echo ""

# Build sj-tracker-report for Windows (cross-compilation)
echo -e "${BLUE}🔧 Building sj-tracker-report backend for Windows...${NC}"
cd sj-tracker-report
GOOS=windows GOARCH=amd64 go build -o "$BUILD_DIR/binaries/sj-tracker-report.exe" ./cmd/server
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ sj-tracker-report built successfully${NC}"
else
    echo -e "${RED}❌ Failed to build sj-tracker-report${NC}"
    exit 1
fi
cd ..
echo ""

# Package Python chat agent using PyInstaller (creates standalone executable)
# Note: PyInstaller typically needs to run on the target platform, but we'll try
echo -e "${BLUE}🤖 Packaging Python chat agent with PyInstaller for Windows...${NC}"
cd sj-tracker-chat-agent

# Check if we're on Windows - if not, warn that PyInstaller might not work
if [[ "$OSTYPE" != "msys" ]] && [[ "$OSTYPE" != "win32" ]]; then
    echo -e "${YELLOW}⚠️  Warning: Building Windows executable from non-Windows platform${NC}"
    echo -e "${YELLOW}   PyInstaller may not create a proper Windows executable${NC}"
    echo -e "${YELLOW}   Consider building on Windows or using a Windows VM/CI${NC}"
    echo ""
fi

# Create a dedicated build virtual environment
if [ -d "venv-build" ]; then
    echo -e "${YELLOW}📦 Removing existing build venv...${NC}"
    rm -rf venv-build
fi

echo -e "${YELLOW}📦 Creating build virtual environment...${NC}"
python3 -m venv venv-build

echo -e "${YELLOW}📦 Installing dependencies including PyInstaller...${NC}"
# Detect activation script path (Windows uses Scripts, Unix uses bin)
if [ -f "venv-build/Scripts/activate" ]; then
    source venv-build/Scripts/activate
elif [ -f "venv-build/bin/activate" ]; then
    # Detect activation script path (Windows uses Scripts, Unix uses bin)
if [ -f "venv-build/Scripts/activate" ]; then
    source venv-build/Scripts/activate
elif [ -f "venv-build/bin/activate" ]; then
    source venv-build/bin/activate
else
    echo -e "${RED}❌ Could not find virtual environment activation script${NC}"
    exit 1
fi
else
    echo -e "${RED}❌ Could not find virtual environment activation script${NC}"
    exit 1
fi
python3 -m pip install --upgrade pip
python3 -m pip install -r requirements.txt
python3 -m pip install pyinstaller
deactivate

# Create PyInstaller spec file for the chat agent server
cat > chat-agent.spec << 'EOF'
# -*- mode: python ; coding: utf-8 -*-

from PyInstaller.utils.hooks import collect_submodules

block_cipher = None

# Collect all langchain.agents submodules to ensure all imports work
langchain_agents_submodules = collect_submodules('langchain.agents')

# Collect all langchain_core submodules - agent.py depends on 129+ langchain_core modules
# Without these, agent.py fails to load and AgentExecutor is never available
langchain_core_submodules = collect_submodules('langchain_core')

a = Analysis(
    ['server.py'],
    pathex=[],
    binaries=[],
    datas=[
        ('main.py', '.'),
        ('backend_client.py', '.'),
    ],
    hiddenimports=[
        'flask',
        'flask_cors',
        'langchain',
        'langchain.agents',
        'langchain.agents.agent',  # AgentExecutor is here
        'langchain.agents.tool_calling_agent',  # create_tool_calling_agent package
        'langchain.agents.tool_calling_agent.base',  # create_tool_calling_agent function
        'langchain_google_genai',
        'langchain_google_genai.chat_models',
        'langchain_core',
        'langchain_core.messages',
        'langchain_core.tools',
        'langchain_core.prompts',
        'langchain_community',
        'requests',
        'dotenv',
        'pydantic',
        'pydantic.fields',
        'main',
        'backend_client',
    ] + langchain_agents_submodules + langchain_core_submodules,  # Add all collected submodules
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='sj-chat-agent',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,  # No console window
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
EOF

# Build standalone executable with PyInstaller
echo -e "${YELLOW}📦 Building standalone executable with PyInstaller...${NC}"
# Detect activation script path (Windows uses Scripts, Unix uses bin)
if [ -f "venv-build/Scripts/activate" ]; then
    source venv-build/Scripts/activate
elif [ -f "venv-build/bin/activate" ]; then
    source venv-build/bin/activate
else
    echo -e "${RED}❌ Could not find virtual environment activation script${NC}"
    exit 1
fi

# On Windows, PyInstaller will create .exe, on other platforms it might not work correctly
if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
    pyinstaller --clean --noconfirm chat-agent.spec
    CHAT_AGENT_EXE="dist/sj-chat-agent.exe"
else
    # Try to build anyway - might work with Wine or might fail
    echo -e "${YELLOW}   Attempting cross-platform build (may not work)...${NC}"
    pyinstaller --clean --noconfirm chat-agent.spec || {
        echo -e "${YELLOW}⚠️  PyInstaller cross-compilation failed${NC}"
        echo -e "${YELLOW}   You may need to build this on Windows or use CI${NC}"
        echo -e "${YELLOW}   Continuing without Python executable...${NC}"
        CHAT_AGENT_EXE=""
    }
    # On non-Windows, PyInstaller won't create .exe, but we'll check for the binary
    if [ -f "dist/sj-chat-agent" ]; then
        CHAT_AGENT_EXE="dist/sj-chat-agent"
        echo -e "${YELLOW}⚠️  Created Unix binary instead of Windows .exe${NC}"
    fi
fi
deactivate

if [ -n "$CHAT_AGENT_EXE" ] && [ -f "$CHAT_AGENT_EXE" ]; then
    # Copy the standalone executable to bundled resources
    echo -e "${YELLOW}📦 Copying standalone executable to bundled resources...${NC}"
    mkdir -p "$BUILD_DIR/python/sj-tracker-chat-agent"
    cp "$CHAT_AGENT_EXE" "$BUILD_DIR/python/sj-tracker-chat-agent/sj-chat-agent.exe"
    # Make executable if on Unix
    if [[ "$OSTYPE" != "msys" ]] && [[ "$OSTYPE" != "win32" ]]; then
        chmod +x "$BUILD_DIR/python/sj-tracker-chat-agent/sj-chat-agent.exe" 2>/dev/null || true
    fi
    echo -e "${GREEN}✅ Python chat agent packaged${NC}"
else
    echo -e "${YELLOW}⚠️  Python chat agent executable not created${NC}"
    echo -e "${YELLOW}   You may need to build this component on Windows${NC}"
fi

# Clean up build artifacts
rm -rf build dist chat-agent.spec

cd ..
echo ""

# Build frontend (needed for desktop app and report frontend)
echo -e "${BLUE}🌐 Building frontend for desktop app and report frontend...${NC}"
cd sj-tracker-frontend
if [ ! -d node_modules ]; then
    echo -e "${YELLOW}📦 Installing frontend dependencies...${NC}"
    npm install
fi

# Check if standalone mode is enabled in next.config.js
if grep -q "output.*standalone" next.config.js 2>/dev/null || grep -q "'standalone'" next.config.js 2>/dev/null; then
    echo -e "${YELLOW}📦 Building with standalone mode (for bundled app)...${NC}"
else
    echo -e "${YELLOW}⚠️  Standalone mode not enabled - frontend may not work in bundled app${NC}"
fi

npm run build
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Frontend built successfully${NC}"
    # Verify standalone build exists
    if [ -d ".next/standalone" ]; then
        echo -e "${GREEN}✅ Standalone build created${NC}"
    else
        echo -e "${YELLOW}⚠️  Standalone build not found - frontend may require full node_modules${NC}"
    fi
else
    echo -e "${RED}❌ Failed to build frontend${NC}"
    exit 1
fi
cd ..
echo ""

# Prepare database binaries if not already done (Windows versions)
echo -e "${YELLOW}📦 Preparing Windows database binaries...${NC}"
TAURI_RESOURCES_DIR="screenjournal/apps/desktop/src-tauri/resources"

# Ensure Windows database binaries exist
if [ ! -d "$TAURI_RESOURCES_DIR/databases/mongodb/windows" ] || [ ! -d "$TAURI_RESOURCES_DIR/databases/influxdb/windows" ]; then
    echo -e "${YELLOW}   Running prepare-databases.sh to download Windows binaries...${NC}"
    ./scripts/prepare-databases.sh
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Failed to prepare database binaries${NC}"
        exit 1
    fi
else
    echo -e "${GREEN}✅ Windows database binaries already prepared${NC}"
fi
echo ""

# Copy binaries and Python environment to Tauri resources
echo -e "${YELLOW}📦 Copying resources to Tauri app...${NC}"

# Create resources directories
mkdir -p "$TAURI_RESOURCES_DIR/binaries"
mkdir -p "$TAURI_RESOURCES_DIR/python"

# Copy Go binaries (Windows .exe)
cp "$BUILD_DIR/binaries/sj-collector.exe" "$TAURI_RESOURCES_DIR/binaries/sj-collector.exe"
cp "$BUILD_DIR/binaries/sj-tracker-report.exe" "$TAURI_RESOURCES_DIR/binaries/sj-tracker-report.exe"

# Copy Python standalone executable (if created)
if [ -d "$BUILD_DIR/python/sj-tracker-chat-agent" ]; then
    cp -r "$BUILD_DIR/python/sj-tracker-chat-agent" "$TAURI_RESOURCES_DIR/python/"
fi

echo -e "${GREEN}✅ Resources copied to Tauri app${NC}"
echo ""

# Build desktop app with Tauri for Windows
echo -e "${BLUE}🖥️  Building bundled desktop app for Windows...${NC}"

# Build UI package first (needed by desktop app)
echo -e "${YELLOW}📦 Building UI package...${NC}"
cd screenjournal
npm run build --workspace=@repo/ui
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Failed to build UI package${NC}"
    exit 1
fi
cd ..

cd screenjournal/apps/desktop

# Initialize variables for build outputs
NSIS_PATH=""
ZIP_PATH=""

# Remove frontend from resources if it exists (to avoid Next.js trying to compile it)
if [ -d "$TAURI_RESOURCES_DIR/frontend" ]; then
    echo -e "${YELLOW}🧹 Removing existing frontend from resources (will be added after build)...${NC}"
    rm -rf "$TAURI_RESOURCES_DIR/frontend"
fi

if [ ! -d node_modules ]; then
    echo -e "${YELLOW}📦 Installing desktop app dependencies...${NC}"
    npm install
fi

# Build Next.js first
npm run build
if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Failed to build Next.js${NC}"
    exit 1
fi

# Build Tauri app for Windows
# Use Windows target for cross-compilation
echo -e "${YELLOW}🔧 Building Tauri app for Windows target...${NC}"
if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
    # On Windows, build normally
    npm run tauri:build
else
    # On macOS/Linux, cross-compile for Windows
    echo -e "${YELLOW}   Cross-compiling for Windows (using GNU toolchain)${NC}"
    echo -e "${YELLOW}   Installing Windows GNU target if needed...${NC}"
    rustup target add x86_64-pc-windows-gnu 2>/dev/null || true
    
    # For GNU target, we need mingw-w64
    if [[ "$OSTYPE" == "darwin"* ]]; then
        if ! command_exists x86_64-w64-mingw32-gcc; then
            echo -e "${YELLOW}   Installing mingw-w64 (required for Windows GNU target)...${NC}"
            if command_exists brew; then
                brew install mingw-w64 2>/dev/null || {
                    echo -e "${RED}   ❌ Could not install mingw-w64 via Homebrew${NC}"
                    echo -e "${YELLOW}   Please install manually: brew install mingw-w64${NC}"
                    exit 1
                }
            else
                echo -e "${RED}   ❌ Homebrew not found. Please install mingw-w64 manually${NC}"
                exit 1
            fi
        fi
        echo -e "${GREEN}   ✓ mingw-w64 found${NC}"
    fi
    
    # Check for llvm-rc (required for Windows resource compilation on macOS/Linux)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        WINDRES_PATH=""
        
        # Try to find llvm-rc in multiple locations
        if command_exists llvm-rc; then
            WINDRES_PATH=$(which llvm-rc)
        elif [ -f "/opt/homebrew/opt/llvm/bin/llvm-rc" ]; then
            WINDRES_PATH="/opt/homebrew/opt/llvm/bin/llvm-rc"
        elif [ -f "/usr/local/opt/llvm/bin/llvm-rc" ]; then
            WINDRES_PATH="/usr/local/opt/llvm/bin/llvm-rc"
        else
            # Try to find it in Cellar (Homebrew's versioned directory)
            CELLAR_PATH=$(find /opt/homebrew/Cellar/llvm /usr/local/Cellar/llvm -name "llvm-rc" 2>/dev/null | head -1)
            if [ -n "$CELLAR_PATH" ] && [ -f "$CELLAR_PATH" ]; then
                WINDRES_PATH="$CELLAR_PATH"
            fi
        fi
        
        if [ -n "$WINDRES_PATH" ] && [ -f "$WINDRES_PATH" ]; then
            export WINDRES="$WINDRES_PATH"
            # Also add to PATH so tauri-winres can find it
            LLVM_BIN_DIR=$(dirname "$WINDRES_PATH")
            export PATH="$LLVM_BIN_DIR:$PATH"
            echo -e "${GREEN}   ✓ Found llvm-rc at: $WINDRES_PATH${NC}"
            echo -e "${GREEN}   ✓ Added to PATH: $LLVM_BIN_DIR${NC}"
        else
            echo -e "${RED}   ❌ llvm-rc not found!${NC}"
            echo -e "${YELLOW}   Please install with: brew install llvm${NC}"
            echo -e "${YELLOW}   Or manually set WINDRES environment variable${NC}"
            echo -e "${YELLOW}   Example: export WINDRES=/opt/homebrew/opt/llvm/bin/llvm-rc${NC}"
            exit 1
        fi
    fi
    
    # Build with Windows GNU target (for cross-compilation from macOS/Linux)
    cd src-tauri
    cargo build --release --target x86_64-pc-windows-gnu || {
        echo -e "${RED}❌ Cross-compilation failed${NC}"
        echo -e "${YELLOW}   Common issues:${NC}"
        echo -e "${YELLOW}   1. Missing mingw-w64: brew install mingw-w64${NC}"
        echo -e "${YELLOW}   2. Missing Windows target: rustup target add x86_64-pc-windows-gnu${NC}"
        echo -e "${YELLOW}   3. For best results, build on Windows directly${NC}"
        exit 1
    }
    cd ..
    
    # Use tauri CLI directly with Windows GNU target
    npx tauri build --target x86_64-pc-windows-gnu || {
        echo -e "${RED}❌ Tauri build failed${NC}"
        echo -e "${YELLOW}   This is often due to missing llvm-rc for resource compilation${NC}"
        echo -e "${YELLOW}   Install with: brew install llvm${NC}"
        echo -e "${YELLOW}   NSIS installer generation also requires Windows${NC}"
        echo -e "${YELLOW}   Binary is built, but installer may need to be generated on Windows${NC}"
    }
fi

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Bundled desktop app built successfully${NC}"
    
    # Copy frontend AFTER desktop app build (to avoid Next.js trying to compile it)
    echo -e "${YELLOW}📦 Copying frontend to Tauri resources...${NC}"
    cd "$SCRIPT_DIR"
    mkdir -p "$TAURI_RESOURCES_DIR/frontend"
    # Use rsync or cp with -L to follow symlinks and ensure complete copy
    if command -v rsync >/dev/null 2>&1; then
        rsync -a --copy-links "$SCRIPT_DIR/sj-tracker-frontend/" "$TAURI_RESOURCES_DIR/frontend/sj-tracker-frontend/"
    else
        # Use cp with -L to follow symlinks
        cp -RL "$SCRIPT_DIR/sj-tracker-frontend" "$TAURI_RESOURCES_DIR/frontend/"
    fi
    echo -e "${GREEN}✅ Frontend copied to Tauri app${NC}"
    
    # Re-run bundle-resources to include frontend in the final bundle
    echo -e "${YELLOW}📦 Re-bundling resources with frontend...${NC}"
    cd screenjournal/apps/desktop
    npm run bundle-resources
    
    # Find the Windows installer file
    NSIS_PATH=$(find src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis -name "*.exe" 2>/dev/null | head -1)
    if [ -z "$NSIS_PATH" ]; then
        # Try MSVC path (if built on Windows)
        NSIS_PATH=$(find src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis -name "*.exe" 2>/dev/null | head -1)
    fi
    if [ -z "$NSIS_PATH" ]; then
        # Try standard release path (if built on Windows)
        NSIS_PATH=$(find src-tauri/target/release/bundle/nsis -name "*.exe" 2>/dev/null | head -1)
    fi
    
    if [ -n "$NSIS_PATH" ]; then
        echo -e "${GREEN}   Windows installer created: $NSIS_PATH${NC}"
        echo -e "${GREEN}   You can now distribute this installer${NC}"
    else
        echo -e "${YELLOW}⚠️  Windows installer (.exe) not generated${NC}"
        echo -e "${YELLOW}   NSIS installer generation typically requires Windows${NC}"
        if [[ "$OSTYPE" == "darwin"* ]] || [[ "$OSTYPE" == "linux-gnu"* ]]; then
            echo -e "${YELLOW}   The Windows app binary was built successfully at:${NC}"
            echo -e "${YELLOW}   src-tauri/target/x86_64-pc-windows-gnu/release/${NC}"
            
            # Create a .zip file of the release directory for distribution
            echo -e "${BLUE}📦 Creating distribution .zip file...${NC}"
            RELEASE_DIR="$SCRIPT_DIR/screenjournal/apps/desktop/src-tauri/target/x86_64-pc-windows-gnu/release"
            TARGET_DIR="$SCRIPT_DIR/screenjournal/apps/desktop/src-tauri/target/x86_64-pc-windows-gnu"
            ZIP_NAME="ScreenJournal-Tracker-Windows-$(date +%Y%m%d).zip"
            ZIP_PATH="$TARGET_DIR/$ZIP_NAME"
            
            if [ ! -d "$RELEASE_DIR" ]; then
                echo -e "${RED}   ❌ Release directory not found: $RELEASE_DIR${NC}"
            else
                cd "$RELEASE_DIR"
                
                # Create zip with only necessary runtime files (exclude build artifacts)
                zip -r "$ZIP_PATH" . \
                    -x "*.d" \
                    -x "*.rlib" \
                    -x "*.rmeta" \
                    -x "build/*" \
                    -x "deps/*" \
                    -x "incremental/*" \
                    -x "examples/*" \
                    -x "*.pdb" \
                    -x "*.ilk" \
                    -x "*.exp" \
                    -x "*.lib" \
                    -x "*.a" \
                    -x "nsis/*" \
                    > /dev/null 2>&1
                
                if [ -f "$ZIP_PATH" ]; then
                    ZIP_SIZE=$(du -h "$ZIP_PATH" | cut -f1)
                    echo -e "${GREEN}   ✅ Distribution zip created: $ZIP_NAME (${ZIP_SIZE})${NC}"
                    echo -e "${GREEN}   📍 Location: $ZIP_PATH${NC}"
                    echo -e "${GREEN}   💡 Users can extract this zip and run Screenjournal.exe${NC}"
                else
                    echo -e "${YELLOW}   ⚠️  Failed to create zip file${NC}"
                fi
                
                cd "$SCRIPT_DIR"
            fi
            
            echo -e "${YELLOW}   To create the installer, either:${NC}"
            echo -e "${YELLOW}   1. Build on Windows directly, or${NC}"
            echo -e "${YELLOW}   2. Use GitHub Actions / CI to build Windows installers${NC}"
        fi
    fi
else
    echo -e "${RED}❌ Failed to build desktop app${NC}"
    exit 1
fi
cd ../../..
echo ""

echo -e "${GREEN}✨ Windows bundled application build completed!${NC}"
echo ""
echo -e "${GREEN}📍 Build Output:${NC}"
echo -e "  - Go binaries: $BUILD_DIR/binaries/${NC}"
echo -e "  - Python executable: $BUILD_DIR/python/${NC}"
if [ -n "$NSIS_PATH" ]; then
    echo -e "  - Windows installer: $NSIS_PATH${NC}"
elif [ -n "$ZIP_PATH" ] && [ -f "$ZIP_PATH" ]; then
    echo -e "  - Distribution zip: $ZIP_PATH${NC}"
    echo -e "  - Tauri app bundle: screenjournal/apps/desktop/src-tauri/target/x86_64-pc-windows-gnu/release/${NC}"
else
    echo -e "  - Tauri app bundle: screenjournal/apps/desktop/src-tauri/target/x86_64-pc-windows-gnu/release/${NC}"
    if [[ "$OSTYPE" != "msys" ]] && [[ "$OSTYPE" != "win32" ]]; then
        echo -e "  - Distribution zip: screenjournal/apps/desktop/src-tauri/target/x86_64-pc-windows-gnu/ScreenJournal-Tracker-Windows-*.zip${NC}"
    fi
fi
echo ""
if [[ "$OSTYPE" != "msys" ]] && [[ "$OSTYPE" != "win32" ]]; then
    echo -e "${YELLOW}💡 Note: Built from non-Windows platform${NC}"
    echo -e "${YELLOW}   For best results, especially NSIS installer generation, build on Windows${NC}"
    echo -e "${YELLOW}   Or use GitHub Actions / CI to build Windows installers${NC}"
fi

