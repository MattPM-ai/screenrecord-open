#!/bin/bash

# Build script for ScreenRecord Productivity Tracker system
# This script builds all components into production-ready executables

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

# Memory optimization: Set Node.js memory limit to prevent OOM kills
# Default to 2GB (very conservative - Next.js 16 with webpack is memory-intensive)
# Can be overridden with NODE_MEMORY_LIMIT environment variable
# If still getting killed, try: NODE_MEMORY_LIMIT=1536 ./build.sh
NODE_MEMORY_LIMIT="${NODE_MEMORY_LIMIT:-2048}"
export NODE_OPTIONS="--max-old-space-size=${NODE_MEMORY_LIMIT}"

# Rust build memory optimization: Limit parallel jobs to reduce memory pressure
# Default to half of CPU cores to leave memory for other processes
RUST_JOBS="${RUST_JOBS:-$(($(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 8) / 2))}"
export CARGO_BUILD_JOBS="${RUST_JOBS}"

echo -e "${GREEN}🔨 Building ScreenRecord Productivity Tracker System${NC}"
NODE_GB=$((NODE_MEMORY_LIMIT / 1024))

# Check available memory (macOS)
if [[ "$OSTYPE" == "darwin"* ]]; then
    TOTAL_MEM=$(sysctl -n hw.memsize 2>/dev/null || echo 0)
    TOTAL_GB=$((TOTAL_MEM / 1024 / 1024 / 1024))
    if [ $TOTAL_GB -gt 0 ]; then
        # Try to get available memory (macOS)
        FREE_MEM=$(vm_stat | grep "Pages free" | awk '{print $3}' | sed 's/\.//')
        if [ -n "$FREE_MEM" ]; then
            # Pages are typically 4KB on macOS
            FREE_MB=$((FREE_MEM * 4 / 1024))
            FREE_GB=$((FREE_MB / 1024))
            echo -e "${YELLOW}💾 System RAM: ${TOTAL_GB}GB total, ~${FREE_GB}GB free${NC}"
        else
            echo -e "${YELLOW}💾 System RAM: ${TOTAL_GB}GB total${NC}"
        fi
    fi
fi

echo -e "${YELLOW}📊 Memory settings: Node.js=${NODE_MEMORY_LIMIT}MB (${NODE_GB}GB), Rust jobs=${RUST_JOBS}${NC}"
echo -e "${YELLOW}💡 If build is killed, try: NODE_MEMORY_LIMIT=2048 ./build.sh${NC}"
echo -e "${YELLOW}⚠️  Make sure Cursor/IDE isn't using excessive memory before building${NC}"
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

# Create build directory
BUILD_DIR="$SCRIPT_DIR/dist"
echo -e "${YELLOW}📁 Creating build directory: $BUILD_DIR${NC}"
mkdir -p "$BUILD_DIR"
echo -e "${GREEN}✅ Build directory created${NC}"
echo ""

# Build sj-collector
echo -e "${BLUE}🔧 Building sj-collector backend...${NC}"
cd sj-collector
go build -o "$BUILD_DIR/sj-collector" ./cmd/server
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ sj-collector built successfully${NC}"
    echo -e "   Output: $BUILD_DIR/sj-collector${NC}"
else
    echo -e "${RED}❌ Failed to build sj-collector${NC}"
    exit 1
fi
cd ..
echo ""

# Build sj-tracker-report
echo -e "${BLUE}🔧 Building sj-tracker-report backend...${NC}"
cd sj-tracker-report
go build -o "$BUILD_DIR/sj-tracker-report" ./cmd/server
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ sj-tracker-report built successfully${NC}"
    echo -e "   Output: $BUILD_DIR/sj-tracker-report${NC}"
else
    echo -e "${RED}❌ Failed to build sj-tracker-report${NC}"
    exit 1
fi
cd ..
echo ""

# Build sj-tracker-frontend
echo -e "${BLUE}🌐 Building sj-tracker-frontend...${NC}"
cd sj-tracker-frontend
if [ ! -d node_modules ]; then
    echo -e "${YELLOW}📦 Installing frontend dependencies...${NC}"
    npm install
fi
npm run build
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ sj-tracker-frontend built successfully${NC}"
    echo -e "   Output: $BUILD_DIR/sj-tracker-frontend (symlinked)${NC}"
    # Create a symlink or copy the .next directory for easier access
    if [ -d ".next" ]; then
        echo -e "${GREEN}   Build artifacts in: sj-tracker-frontend/.next${NC}"
    fi
else
    echo -e "${RED}❌ Failed to build sj-tracker-frontend${NC}"
    exit 1
fi
cd ..
echo ""

# Build desktop app
echo -e "${BLUE}🖥️  Building desktop app...${NC}"
cd screenrecord/apps/desktop
if [ ! -d node_modules ]; then
    echo -e "${YELLOW}📦 Installing desktop app dependencies...${NC}"
    npm install
fi
# Build Next.js first
# Ensure NODE_OPTIONS is set (in case it wasn't inherited)
export NODE_OPTIONS="--max-old-space-size=${NODE_MEMORY_LIMIT}"
echo -e "${YELLOW}🔧 Using NODE_OPTIONS: ${NODE_OPTIONS}${NC}"
echo -e "${YELLOW}🔧 Memory limit: ${NODE_MEMORY_LIMIT}MB (${NODE_GB}GB)${NC}"

# Check memory before build (macOS)
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo -e "${YELLOW}📊 Memory check before build:${NC}"
    vm_stat | head -5
    echo ""
fi

# Try building without --webpack flag first (uses default bundler, more memory-efficient)
# Only fall back to --webpack if the default build fails
# Use npx with explicit node args to ensure memory limit is applied
echo -e "${YELLOW}🔧 Attempting build without --webpack (more memory-efficient)...${NC}"
if ! NODE_OPTIONS="--max-old-space-size=${NODE_MEMORY_LIMIT}" npx --node-options="--max-old-space-size=${NODE_MEMORY_LIMIT}" next build; then
    echo -e "${YELLOW}⚠️  Default build failed, trying with --webpack (fallback)...${NC}"
    if ! NODE_OPTIONS="--max-old-space-size=${NODE_MEMORY_LIMIT}" npx --node-options="--max-old-space-size=${NODE_MEMORY_LIMIT}" next build --webpack; then
        echo -e "${RED}❌ Build failed${NC}"
        if [[ "$OSTYPE" == "darwin"* ]]; then
            echo -e "${YELLOW}📊 Memory check after failure:${NC}"
            vm_stat | head -5
            echo -e "${YELLOW}💡 If process was killed (Killed: 9), the system ran out of memory${NC}"
            echo -e "${YELLOW}💡 Solutions:${NC}"
            echo -e "${YELLOW}   1. Close Cursor and other heavy apps, then retry${NC}"
            echo -e "${YELLOW}   2. Use lower memory: NODE_MEMORY_LIMIT=1536 ./build.sh${NC}"
            echo -e "${YELLOW}   3. Use minimal memory: NODE_MEMORY_LIMIT=1024 ./build.sh${NC}"
        fi
        exit 1
    fi
fi
# Then build Tauri app
npm run tauri:build
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Desktop app built successfully${NC}"
    # Tauri build output location varies by OS
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo -e "   Output: screenrecord/apps/desktop/src-tauri/target/release/bundle/${NC}"
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo -e "   Output: screenrecord/apps/desktop/src-tauri/target/release/bundle/${NC}"
    elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
        echo -e "   Output: screenrecord/apps/desktop/src-tauri/target/release/bundle/${NC}"
    fi
else
    echo -e "${RED}❌ Failed to build desktop app${NC}"
    exit 1
fi
cd ../../..
echo ""

# Package Python chat agent
echo -e "${BLUE}🤖 Packaging Python chat agent...${NC}"
cd sj-tracker-chat-agent

# Create a dedicated build virtual environment
if [ -d "venv-build" ]; then
    echo -e "${YELLOW}📦 Removing existing build venv...${NC}"
    rm -rf venv-build
fi

echo -e "${YELLOW}📦 Creating build virtual environment...${NC}"
python3 -m venv venv-build

echo -e "${YELLOW}📦 Installing dependencies...${NC}"
source venv-build/bin/activate
python3 -m pip install --upgrade pip
python3 -m pip install -r requirements.txt
deactivate

# Copy the build venv to dist
echo -e "${YELLOW}📦 Copying Python environment to dist...${NC}"
cp -r venv-build "$BUILD_DIR/sj-tracker-chat-agent-venv"

# Copy Python source files
mkdir -p "$BUILD_DIR/sj-tracker-chat-agent"
cp server.py "$BUILD_DIR/sj-tracker-chat-agent/"
cp main.py "$BUILD_DIR/sj-tracker-chat-agent/"
cp backend_client.py "$BUILD_DIR/sj-tracker-chat-agent/"

# Create a wrapper script to run the chat agent
cat > "$BUILD_DIR/sj-tracker-chat-agent/run.sh" << 'EOF'
#!/bin/bash
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
VENV_DIR="$SCRIPT_DIR/../sj-tracker-chat-agent-venv"
source "$VENV_DIR/bin/activate"
cd "$SCRIPT_DIR"
python3 server.py
EOF
chmod +x "$BUILD_DIR/sj-tracker-chat-agent/run.sh"

echo -e "${GREEN}✅ Python chat agent packaged successfully${NC}"
echo -e "   Output: $BUILD_DIR/sj-tracker-chat-agent/${NC}"
cd ..
echo ""

# Copy necessary configuration and data files
echo -e "${YELLOW}📁 Copying configuration files...${NC}"

# Copy .env.example files if they don't exist in dist
if [ -f "sj-collector/.env.example" ]; then
    cp sj-collector/.env.example "$BUILD_DIR/sj-collector.env.example"
fi
if [ -f "sj-tracker-report/.env.example" ]; then
    cp sj-tracker-report/.env.example "$BUILD_DIR/sj-tracker-report.env.example"
fi

# Create storage directories structure
mkdir -p "$BUILD_DIR/storage"
mkdir -p "$BUILD_DIR/data"

echo -e "${GREEN}✅ Configuration files copied${NC}"
echo ""

# Create a README for the build
cat > "$BUILD_DIR/README.md" << EOF
# ScreenRecord Productivity Tracker - Built Distribution

This directory contains the built executables and assets for the ScreenRecord system.

## Contents

- \`sj-collector\` - Go backend for data collection
- \`sj-tracker-report\` - Go backend for report generation
- \`sj-tracker-frontend/\` - Next.js frontend (run from sj-tracker-frontend directory)
- \`sj-tracker-chat-agent/\` - Python chat agent with virtual environment
- \`storage/\` - Storage directory for uploaded files
- \`data/\` - Data directory for local databases

## Running the Built System

Use the \`run-built.sh\` script from the project root to start all services.

## Manual Execution

1. Start Docker services: \`docker-compose up -d\`
2. Configure environment variables (copy .env.example files)
3. Run each service:
   - \`./sj-collector\`
   - \`./sj-tracker-report\`
   - \`cd sj-tracker-frontend && npm start\`
   - \`cd sj-tracker-chat-agent && ./run.sh\`

## Notes

- The frontend build artifacts remain in \`sj-tracker-frontend/.next\`
- The desktop app build artifacts are in \`screenrecord/apps/desktop/src-tauri/target/release/bundle/\`
- Python dependencies are bundled in \`sj-tracker-chat-agent-venv/\`
EOF

echo -e "${GREEN}✨ Build completed successfully!${NC}"
echo ""
echo -e "${GREEN}📍 Build Output:${NC}"
echo -e "  - Build directory: $BUILD_DIR${NC}"
echo -e "  - sj-collector: $BUILD_DIR/sj-collector${NC}"
echo -e "  - sj-tracker-report: $BUILD_DIR/sj-tracker-report${NC}"
echo -e "  - Frontend: sj-tracker-frontend/.next${NC}"
echo -e "  - Chat Agent: $BUILD_DIR/sj-tracker-chat-agent/${NC}"
echo ""
echo -e "${YELLOW}💡 Next step: Run \`./run-built.sh\` to start the built system${NC}"

