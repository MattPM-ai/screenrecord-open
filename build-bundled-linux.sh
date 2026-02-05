#!/bin/bash

# Build script for creating a Linux .deb package for ScreenJournal Tracker
# This script MUST be run on Linux to build Linux binaries and create a .deb package
# Cross-compilation from macOS/Windows is not supported due to OpenSSL and .deb packaging requirements

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

echo -e "${GREEN}🔨 Building Linux Bundled ScreenJournal Application${NC}"
echo ""

# Check if running on Linux
if [[ "$OSTYPE" != "linux-gnu"* ]]; then
    echo -e "${RED}❌ This script must be run on Linux${NC}"
    echo -e "${YELLOW}   Cross-compilation from macOS/Windows is not supported${NC}"
    echo -e "${YELLOW}   Reasons:${NC}"
    echo -e "${YELLOW}   1. OpenSSL cross-compilation requires complex toolchain setup${NC}"
    echo -e "${YELLOW}   2. .deb package creation requires Linux-native tools${NC}"
    echo -e "${YELLOW}   3. For best results, use a Linux VM, CI, or native Linux machine${NC}"
    exit 1
fi
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

# Check for Linux build dependencies
if ! command_exists dpkg-deb; then
    echo -e "${YELLOW}⚠️  dpkg-deb not found. .deb package creation may fail.${NC}"
    echo -e "${YELLOW}   Install with: sudo apt-get install dpkg-dev${NC}"
fi

echo -e "${GREEN}✅ All prerequisites met${NC}"
echo ""

# Create temporary build directory
BUILD_DIR="$SCRIPT_DIR/dist-bundled-linux"
echo -e "${YELLOW}📁 Creating build directory: $BUILD_DIR${NC}"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/binaries"
mkdir -p "$BUILD_DIR/python"
echo -e "${GREEN}✅ Build directory created${NC}"
echo ""

# Build sj-collector for Linux
echo -e "${BLUE}🔧 Building sj-collector backend for Linux...${NC}"
cd sj-collector
GOOS=linux GOARCH=amd64 go build -o "$BUILD_DIR/binaries/sj-collector" ./cmd/server
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ sj-collector built successfully${NC}"
else
    echo -e "${RED}❌ Failed to build sj-collector${NC}"
    exit 1
fi
cd ..
echo ""

# Build sj-tracker-report for Linux
echo -e "${BLUE}🔧 Building sj-tracker-report backend for Linux...${NC}"
cd sj-tracker-report
GOOS=linux GOARCH=amd64 go build -o "$BUILD_DIR/binaries/sj-tracker-report" ./cmd/server
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ sj-tracker-report built successfully${NC}"
else
    echo -e "${RED}❌ Failed to build sj-tracker-report${NC}"
    exit 1
fi
cd ..
echo ""

# Package Python chat agent using PyInstaller (creates standalone executable)
echo -e "${BLUE}🤖 Packaging Python chat agent with PyInstaller for Linux...${NC}"
cd sj-tracker-chat-agent

# Create a dedicated build virtual environment
if [ -d "venv-build" ]; then
    echo -e "${YELLOW}📦 Removing existing build venv...${NC}"
    rm -rf venv-build
fi

echo -e "${YELLOW}📦 Creating build virtual environment...${NC}"
python3 -m venv venv-build

echo -e "${YELLOW}📦 Installing dependencies including PyInstaller...${NC}"
source venv-build/bin/activate
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
source venv-build/bin/activate
pyinstaller --clean --noconfirm chat-agent.spec
deactivate

CHAT_AGENT_EXE="dist/sj-chat-agent"

if [ -n "$CHAT_AGENT_EXE" ] && [ -f "$CHAT_AGENT_EXE" ]; then
    # Copy the standalone executable to bundled resources
    echo -e "${YELLOW}📦 Copying standalone executable to bundled resources...${NC}"
    mkdir -p "$BUILD_DIR/python/sj-tracker-chat-agent"
    cp "$CHAT_AGENT_EXE" "$BUILD_DIR/python/sj-tracker-chat-agent/sj-chat-agent"
    chmod +x "$BUILD_DIR/python/sj-tracker-chat-agent/sj-chat-agent"
    echo -e "${GREEN}✅ Python chat agent packaged${NC}"
else
    echo -e "${YELLOW}⚠️  Python chat agent executable not created${NC}"
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

# Prepare database binaries if not already done (Linux versions)
echo -e "${YELLOW}📦 Preparing Linux database binaries...${NC}"
TAURI_RESOURCES_DIR="screenjournal/apps/desktop/src-tauri/resources"

# Ensure Linux database binaries exist
if [ ! -d "$TAURI_RESOURCES_DIR/databases/mongodb/linux" ] || [ ! -d "$TAURI_RESOURCES_DIR/databases/influxdb/linux" ]; then
    echo -e "${YELLOW}   Running prepare-databases.sh to download Linux binaries...${NC}"
    ./scripts/prepare-databases.sh
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Failed to prepare database binaries${NC}"
        exit 1
    fi
else
    echo -e "${GREEN}✅ Linux database binaries already prepared${NC}"
fi
echo ""

# Copy binaries and Python environment to Tauri resources
echo -e "${YELLOW}📦 Copying resources to Tauri app...${NC}"

# Create resources directories
mkdir -p "$TAURI_RESOURCES_DIR/binaries"
mkdir -p "$TAURI_RESOURCES_DIR/python"

# Copy Go binaries (Linux - no .exe extension)
cp "$BUILD_DIR/binaries/sj-collector" "$TAURI_RESOURCES_DIR/binaries/sj-collector"
cp "$BUILD_DIR/binaries/sj-tracker-report" "$TAURI_RESOURCES_DIR/binaries/sj-tracker-report"

# Copy Python standalone executable (if created)
if [ -d "$BUILD_DIR/python/sj-tracker-chat-agent" ]; then
    cp -r "$BUILD_DIR/python/sj-tracker-chat-agent" "$TAURI_RESOURCES_DIR/python/"
fi

echo -e "${GREEN}✅ Resources copied to Tauri app${NC}"
echo ""

# Build desktop app with Tauri for Linux
echo -e "${BLUE}🖥️  Building bundled desktop app for Linux...${NC}"

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

# Build Tauri app for Linux (creates .deb package)
echo -e "${YELLOW}🔧 Building Tauri app for Linux target...${NC}"
npm run tauri:build

# Initialize variables for build outputs
DEB_PATH=""

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
    
    # Find the .deb package file
    DEB_PATH=$(find src-tauri/target/release/bundle/deb -name "*.deb" 2>/dev/null | head -1)
    
    if [ -n "$DEB_PATH" ]; then
        DEB_SIZE=$(du -h "$DEB_PATH" | cut -f1)
        echo -e "${GREEN}   ✅ Linux .deb package created: $(basename "$DEB_PATH") (${DEB_SIZE})${NC}"
        echo -e "${GREEN}   📍 Location: $DEB_PATH${NC}"
        echo -e "${GREEN}   💡 Users can install with: sudo dpkg -i $(basename "$DEB_PATH")${NC}"
    else
        echo -e "${YELLOW}⚠️  .deb package not found${NC}"
        echo -e "${YELLOW}   The app binary should be in: src-tauri/target/release/${NC}"
        echo -e "${YELLOW}   Check build output for errors${NC}"
    fi
else
    echo -e "${RED}❌ Failed to build desktop app${NC}"
    exit 1
fi
cd ../../..
echo ""

echo -e "${GREEN}✨ Linux bundled application build completed!${NC}"
echo ""
echo -e "${GREEN}📍 Build Output:${NC}"
echo -e "  - Go binaries: $BUILD_DIR/binaries/${NC}"
echo -e "  - Python executable: $BUILD_DIR/python/${NC}"
if [ -n "$DEB_PATH" ]; then
    echo -e "  - .deb package: $DEB_PATH${NC}"
else
    echo -e "  - Tauri app bundle: screenjournal/apps/desktop/src-tauri/target/release/bundle/${NC}"
fi
echo ""

