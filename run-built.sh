#!/bin/bash

# Run script for built ScreenRecord Productivity Tracker system
# This script starts all services using the built executables from build.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Get the directory where this script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo -e "${GREEN}🚀 Starting ScreenRecord Productivity Tracker System (Built Version)${NC}"
echo ""

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to check if a port is in use
port_in_use() {
    lsof -Pi :$1 -sTCP:LISTEN -t >/dev/null 2>&1
}

# Check prerequisites
echo -e "${YELLOW}📋 Checking prerequisites...${NC}"

# Check for Docker - prefer Docker for databases (InfluxDB and MongoDB)
USE_DOCKER=false
if command_exists docker && command_exists docker-compose; then
    if docker info >/dev/null 2>&1; then
        USE_DOCKER=true
        echo -e "${GREEN}✅ Docker detected and running${NC}"
    else
        echo -e "${YELLOW}⚠️  Docker is installed but not running${NC}"
        echo -e "${YELLOW}   Attempting to start Docker...${NC}"
        # Try to start Docker (works on macOS with Docker Desktop)
        if [[ "$OSTYPE" == "darwin"* ]]; then
            open -a Docker 2>/dev/null || true
            echo -e "${YELLOW}   Waiting for Docker to start...${NC}"
            for i in {1..30}; do
                if docker info >/dev/null 2>&1; then
                    USE_DOCKER=true
                    echo -e "${GREEN}✅ Docker started successfully${NC}"
                    break
                fi
                sleep 2
            done
        fi
        if [ "$USE_DOCKER" = "false" ]; then
            echo -e "${RED}❌ Failed to start Docker${NC}"
            echo -e "${YELLOW}   Please start Docker manually and run this script again${NC}"
            echo -e "${YELLOW}   Or set USE_EMBEDDED_DB=true to use embedded databases${NC}"
            exit 1
        fi
    fi
else
    echo -e "${RED}❌ Docker not found${NC}"
    echo -e "${YELLOW}   Docker is required for InfluxDB and MongoDB${NC}"
    echo -e "${YELLOW}   Please install Docker and Docker Compose${NC}"
    echo -e "${YELLOW}   Or set USE_EMBEDDED_DB=true to use embedded databases (limited functionality)${NC}"
    if [ "$USE_EMBEDDED_DB" != "true" ]; then
        exit 1
    fi
fi

# Check for USE_EMBEDDED_DB override (only if explicitly set)
if [ "$USE_EMBEDDED_DB" = "true" ]; then
    USE_DOCKER=false
    echo -e "${YELLOW}ℹ️  Embedded mode forced via USE_EMBEDDED_DB${NC}"
    echo -e "${YELLOW}   Note: InfluxDB and MongoDB will not be available${NC}"
fi

# Check if build directory exists
BUILD_DIR="$SCRIPT_DIR/dist"
if [ ! -d "$BUILD_DIR" ]; then
    echo -e "${RED}❌ Build directory not found: $BUILD_DIR${NC}"
    echo -e "${YELLOW}   Please run ./build.sh first to build all components${NC}"
    exit 1
fi

# Check if built binaries exist
if [ ! -f "$BUILD_DIR/sj-collector" ]; then
    echo -e "${RED}❌ sj-collector binary not found${NC}"
    echo -e "${YELLOW}   Please run ./build.sh first${NC}"
    exit 1
fi

if [ ! -f "$BUILD_DIR/sj-tracker-report" ]; then
    echo -e "${RED}❌ sj-tracker-report binary not found${NC}"
    echo -e "${YELLOW}   Please run ./build.sh first${NC}"
    exit 1
fi

if [ ! -d "sj-tracker-frontend/.next" ]; then
    echo -e "${RED}❌ Frontend build not found${NC}"
    echo -e "${YELLOW}   Please run ./build.sh first${NC}"
    exit 1
fi

if [ ! -d "$BUILD_DIR/sj-tracker-chat-agent" ]; then
    echo -e "${RED}❌ Chat agent build not found${NC}"
    echo -e "${YELLOW}   Please run ./build.sh first${NC}"
    exit 1
fi

echo -e "${GREEN}✅ All prerequisites met${NC}"
echo ""

# Create storage and data directories
echo -e "${YELLOW}📁 Creating storage directories...${NC}"
mkdir -p storage
mkdir -p sj-collector/storage
mkdir -p data
mkdir -p "$BUILD_DIR/storage"
mkdir -p "$BUILD_DIR/data"
echo -e "${GREEN}✅ Storage directories created${NC}"
echo ""

# Start databases based on mode
if [ "$USE_DOCKER" = "true" ]; then
    # Docker mode: Start MongoDB and InfluxDB containers
    echo -e "${YELLOW}🐳 Starting Docker services (MongoDB, InfluxDB)...${NC}"
    
    # Check if containers already exist and are running
    if docker ps --format '{{.Names}}' | grep -q "^screenrecord-mongodb$" && \
       docker ps --format '{{.Names}}' | grep -q "^screenrecord-influxdb$"; then
        echo -e "${GREEN}✅ Database containers already running${NC}"
    else
        docker-compose up -d
        echo -e "${YELLOW}⏳ Waiting for services to be ready...${NC}"
        sleep 5
    fi

    # Check if MongoDB is ready
    echo -e "${YELLOW}🔍 Checking MongoDB...${NC}"
    for i in {1..30}; do
        if docker exec screenrecord-mongodb mongosh --eval "db.adminCommand('ping')" --quiet >/dev/null 2>&1; then
            echo -e "${GREEN}✅ MongoDB is ready${NC}"
            break
        fi
        if [ $i -eq 30 ]; then
            echo -e "${RED}❌ MongoDB failed to start${NC}"
            echo -e "${YELLOW}   Check logs with: docker logs screenrecord-mongodb${NC}"
            exit 1
        fi
        sleep 1
    done

    # Check if InfluxDB is ready
    echo -e "${YELLOW}🔍 Checking InfluxDB...${NC}"
    for i in {1..30}; do
        if curl -s http://localhost:8086/health >/dev/null 2>&1; then
            echo -e "${GREEN}✅ InfluxDB is ready${NC}"
            break
        fi
        if [ $i -eq 30 ]; then
            echo -e "${RED}❌ InfluxDB failed to start${NC}"
            echo -e "${YELLOW}   Check logs with: docker logs screenrecord-influxdb${NC}"
            exit 1
        fi
        sleep 1
    done
    
    # Verify InfluxDB setup (org, bucket, token)
    echo -e "${YELLOW}🔍 Verifying InfluxDB setup...${NC}"
    INFLUX_SETUP_OK=true
    if ! curl -s -o /dev/null -w "%{http_code}" http://localhost:8086/api/v2/setup 2>/dev/null | grep -q "200\|404"; then
        INFLUX_SETUP_OK=false
    fi
    if [ "$INFLUX_SETUP_OK" = "true" ]; then
        echo -e "${GREEN}✅ InfluxDB setup verified${NC}"
    else
        echo -e "${YELLOW}⚠️  InfluxDB may need initial setup${NC}"
        echo -e "${YELLOW}   Visit http://localhost:8086 to complete setup if needed${NC}"
    fi
else
    # Embedded mode: Use SQLite databases (limited functionality)
    echo -e "${YELLOW}📦 Using embedded database mode (SQLite)${NC}"
    echo -e "${RED}⚠️  WARNING: InfluxDB and MongoDB are not available in embedded mode${NC}"
    echo -e "${YELLOW}   Data persistence will be limited${NC}"
    echo -e "${YELLOW}   MongoDB → SQLite (data/reports.db)${NC}"
    echo -e "${YELLOW}   InfluxDB → SQLite (data/metrics.db) - NOT SUPPORTED${NC}"
    
    # Set environment variable to indicate embedded mode
    export USE_EMBEDDED_DB=true
fi

echo ""

# Start sj-collector backend (using built binary)
echo -e "${YELLOW}🔧 Starting sj-collector backend (built)...${NC}"
cd sj-collector
if [ ! -f .env ]; then
    echo -e "${YELLOW}⚠️  .env file not found, creating from template...${NC}"
    cp .env.example .env 2>/dev/null || cat > .env <<EOF
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
JWT_SECRET=your-secret-key-change-in-production
INFLUXDB2_URL=http://localhost:8086
INFLUXDB2_TOKEN=screenrecord-admin-token-change-in-production
INFLUXDB2_ORG=screenrecord-org
INFLUXDB2_BUCKET=screenrecord-metrics
STORAGE_BASE_PATH=./storage
STORAGE_BASE_URL=http://localhost:8080/storage
EOF
    # Update existing .env file to match docker-compose defaults if it exists
    if [ -f .env ]; then
        # Update InfluxDB settings to match docker-compose.yml defaults
        sed -i.bak 's/^INFLUXDB2_ORG=.*/INFLUXDB2_ORG=screenrecord-org/' .env
        sed -i.bak 's/^INFLUXDB2_BUCKET=.*/INFLUXDB2_BUCKET=screenrecord-metrics/' .env
        sed -i.bak 's/^INFLUXDB2_TOKEN=.*/INFLUXDB2_TOKEN=screenrecord-admin-token-change-in-production/' .env
        sed -i.bak 's/^INFLUXDB2_URL=.*/INFLUXDB2_URL=http:\/\/localhost:8086/' .env
        # Also update any old bucket names
        sed -i.bak 's/matt-metrics/screenrecord-metrics/g' .env
        rm -f .env.bak
    fi
fi

# Check if collector is already running
if port_in_use 8080; then
    echo -e "${YELLOW}⚠️  Port 8080 is already in use${NC}"
    echo -e "${YELLOW}   Checking if collector is already running...${NC}"
    if curl -s http://localhost:8080/health >/dev/null 2>&1 || curl -s http://localhost:8080/mock-auth >/dev/null 2>&1; then
        echo -e "${GREEN}✅ Collector appears to be running on port 8080${NC}"
        COLLECTOR_PID=""
    else
        echo -e "${RED}❌ Port 8080 is in use but collector is not responding${NC}"
        echo -e "${YELLOW}   Please stop the process using port 8080 and try again${NC}"
        exit 1
    fi
    cd ..
else
    # Run the built binary from the source directory (so it can find .env)
    "$BUILD_DIR/sj-collector" > /tmp/sj-collector.log 2>&1 &
    COLLECTOR_PID=$!
    cd ..
    echo -e "${GREEN}✅ sj-collector started (PID: $COLLECTOR_PID)${NC}"
    
    # Wait for collector to be ready
    echo -e "${YELLOW}⏳ Waiting for collector to be ready...${NC}"
    for i in {1..30}; do
        if port_in_use 8080; then
            # Give it a moment to fully initialize
            sleep 1
            if curl -s http://localhost:8080/mock-auth >/dev/null 2>&1; then
                echo -e "${GREEN}✅ Collector is ready${NC}"
                break
            fi
        fi
        if [ $i -eq 30 ]; then
            echo -e "${RED}❌ Collector failed to start${NC}"
            echo -e "${YELLOW}   Check logs: tail -f /tmp/sj-collector.log${NC}"
            if [ -n "$COLLECTOR_PID" ]; then
                kill $COLLECTOR_PID 2>/dev/null || true
            fi
            exit 1
        fi
        sleep 1
    done
fi

# Start sj-tracker-report backend (using built binary)
echo -e "${YELLOW}🔧 Starting sj-tracker-report backend (built)...${NC}"
cd sj-tracker-report
if [ ! -f .env ]; then
    echo -e "${YELLOW}⚠️  .env file not found, creating from template...${NC}"
    cp .env.example .env 2>/dev/null || cat > .env <<EOF
PORT=8085
HOST=0.0.0.0
INFLUXDB2_URL=http://localhost:8086
INFLUXDB2_TOKEN=screenrecord-admin-token-change-in-production
INFLUXDB2_ORG=screenrecord-org
INFLUXDB2_BUCKET=screenrecord-metrics
MONGODB_HOST=localhost
MONGODB_PORT=27017
MONGODB_DATABASE=reports
MONGODB_USERNAME=admin
MONGODB_PASSWORD=admin123
MONGODB_AUTH_SOURCE=admin
OPENAI_API_KEY=your-openai-api-key-here
EOF
fi
# Update existing .env file to match docker-compose defaults if it exists
if [ -f .env ]; then
    # Update InfluxDB settings to match docker-compose.yml defaults
    sed -i.bak 's/^INFLUXDB2_ORG=.*/INFLUXDB2_ORG=screenrecord-org/' .env
    sed -i.bak 's/^INFLUXDB2_BUCKET=.*/INFLUXDB2_BUCKET=screenrecord-metrics/' .env
    sed -i.bak 's/^INFLUXDB2_TOKEN=.*/INFLUXDB2_TOKEN=screenrecord-admin-token-change-in-production/' .env
    sed -i.bak 's/^INFLUXDB2_URL=.*/INFLUXDB2_URL=http:\/\/localhost:8086/' .env
    # Update MongoDB settings
    sed -i.bak 's/^MONGODB_DATABASE=.*/MONGODB_DATABASE=reports/' .env
    sed -i.bak 's/^MONGODB_USERNAME=.*/MONGODB_USERNAME=admin/' .env
    sed -i.bak 's/^MONGODB_PASSWORD=.*/MONGODB_PASSWORD=admin123/' .env
    sed -i.bak 's/^MONGODB_AUTH_SOURCE=.*/MONGODB_AUTH_SOURCE=admin/' .env
    rm -f .env.bak
fi
# Run the built binary from the source directory (so it can find .env)
"$BUILD_DIR/sj-tracker-report" &
REPORT_PID=$!
cd ..
echo -e "${GREEN}✅ sj-tracker-report started (PID: $REPORT_PID)${NC}"

# Wait for report service to be ready
sleep 3

# Start Python chat agent (using built virtualenv)
echo -e "${YELLOW}🤖 Starting Python chat agent (built)...${NC}"
cd "$BUILD_DIR/sj-tracker-chat-agent"

# Set default backend URL if not set
if [ -z "$BACKEND_URL" ]; then
    export BACKEND_URL="http://localhost:8085"
fi

# Run Python agent using the built virtualenv
# On macOS, copied venvs have broken Python binary library paths (@executable_path/../Python3)
# Solution: Use system Python with venv's site-packages in PYTHONPATH
# Find the actual site-packages directory
SITE_PACKAGES=$(find "$BUILD_DIR/sj-tracker-chat-agent-venv/lib" -type d -name "site-packages" | head -1)
if [ -z "$SITE_PACKAGES" ]; then
    echo -e "${RED}❌ Could not find site-packages in venv${NC}"
    exit 1
fi

# Use system Python with venv's site-packages
# This works around broken library paths in copied venvs on macOS
export PYTHONPATH="$SITE_PACKAGES:$PYTHONPATH"
# Also set VIRTUAL_ENV for packages that check it
export VIRTUAL_ENV="$BUILD_DIR/sj-tracker-chat-agent-venv"
python3 server.py > /tmp/sj-chat-agent.log 2>&1 &
CHAT_AGENT_PID=$!
cd "$SCRIPT_DIR"
echo -e "${GREEN}✅ Python chat agent started (PID: $CHAT_AGENT_PID)${NC}"

# Wait for chat agent to be ready
sleep 2

# Start frontend (using built version)
echo -e "${YELLOW}🌐 Starting frontend (built)...${NC}"
cd sj-tracker-frontend
# Use npm start to run the production build
npm start &
FRONTEND_PID=$!
cd ..
echo -e "${GREEN}✅ Frontend started (PID: $FRONTEND_PID)${NC}"

# Wait for frontend to be ready
sleep 5

# Note about desktop app
echo -e "${YELLOW}🖥️  Desktop app${NC}"
echo -e "${YELLOW}   The desktop app is built as a standalone application${NC}"
if [[ "$OSTYPE" == "darwin"* ]]; then
    DESKTOP_APP_PATH="screenrecord/apps/desktop/src-tauri/target/release/bundle/macos"
    if [ -d "$DESKTOP_APP_PATH" ]; then
        echo -e "${GREEN}   Desktop app available at: $DESKTOP_APP_PATH${NC}"
        echo -e "${YELLOW}   Launch it manually from the .app bundle${NC}"
    else
        echo -e "${YELLOW}   Desktop app not found. Run ./build.sh to build it.${NC}"
    fi
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    DESKTOP_APP_PATH="screenrecord/apps/desktop/src-tauri/target/release/bundle"
    if [ -d "$DESKTOP_APP_PATH" ]; then
        echo -e "${GREEN}   Desktop app available at: $DESKTOP_APP_PATH${NC}"
        echo -e "${YELLOW}   Launch it manually from the bundle directory${NC}"
    else
        echo -e "${YELLOW}   Desktop app not found. Run ./build.sh to build it.${NC}"
    fi
else
    echo -e "${YELLOW}   Desktop app build location varies by platform${NC}"
fi

echo ""
echo -e "${GREEN}✨ All services started successfully!${NC}"
echo ""
echo -e "${GREEN}📍 Service URLs:${NC}"
echo -e "  - Frontend:        http://localhost:3030"
echo -e "  - Collector API:   http://localhost:8080"
echo -e "  - Report API:      http://localhost:8085"
echo -e "  - Chat Agent:      http://localhost:8087"
if [ "$USE_DOCKER" = "true" ]; then
    echo -e "  - MongoDB:         mongodb://localhost:27017"
    echo -e "  - InfluxDB:        http://localhost:8086"
else
    echo -e "  - Database Mode:   Embedded (SQLite)"
    echo -e "  - Reports DB:      data/reports.db"
    echo -e "  - Metrics DB:      data/metrics.db"
fi
echo ""
echo -e "${YELLOW}⚠️  Press Ctrl+C to stop all services${NC}"

# Function to cleanup on exit
cleanup() {
    echo ""
    echo -e "${YELLOW}🛑 Shutting down services...${NC}"
    if [ -n "$COLLECTOR_PID" ]; then
        kill $COLLECTOR_PID 2>/dev/null || true
    fi
    if [ -n "$REPORT_PID" ]; then
        kill $REPORT_PID 2>/dev/null || true
    fi
    if [ -n "$CHAT_AGENT_PID" ]; then
        kill $CHAT_AGENT_PID 2>/dev/null || true
    fi
    if [ -n "$FRONTEND_PID" ]; then
        kill $FRONTEND_PID 2>/dev/null || true
    fi
    if [ "$USE_DOCKER" = "true" ]; then
        docker-compose down
    fi
    echo -e "${GREEN}✅ All services stopped${NC}"
    exit 0
}

# Trap Ctrl+C
trap cleanup SIGINT SIGTERM

# Wait for all processes
wait

