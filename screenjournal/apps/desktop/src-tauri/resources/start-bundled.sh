#!/bin/bash

# Bundled service startup script for ScreenJournal Tracker
# This script starts all services using bundled binaries from the Tauri app
# Outputs structured progress messages for the Rust service manager to parse

# Don't exit on error - let Rust handle error detection
set +e

# Progress markers for parsing
PROGRESS_PREFIX="[PROGRESS]"
ERROR_PREFIX="[ERROR]"
SUCCESS_PREFIX="[SUCCESS]"
STEP_PREFIX="[STEP]"

# Get paths from environment variables (set by Rust)
RESOURCE_DIR="${RESOURCE_DIR:-}"
APP_DATA_DIR="${APP_DATA_DIR:-}"

if [ -z "$RESOURCE_DIR" ] || [ -z "$APP_DATA_DIR" ]; then
    echo "${ERROR_PREFIX} RESOURCE_DIR or APP_DATA_DIR not set"
    exit 1
fi

# Determine platform and architecture
PLATFORM=""
ARCH=""
if [[ "$OSTYPE" == "darwin"* ]]; then
    PLATFORM="darwin"
    if [[ $(uname -m) == "arm64" ]]; then
        ARCH="aarch64"
    else
        ARCH="x86_64"
    fi
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    PLATFORM="linux"
    ARCH=$(uname -m)
    if [ "$ARCH" = "x86_64" ]; then
        ARCH="x86_64"
    elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
        ARCH="aarch64"
    fi
fi

# Function to check if a port is in use
port_in_use() {
    lsof -Pi :$1 -sTCP:LISTEN -t >/dev/null 2>&1
}

# Function to wait for a service to be ready
wait_for_service() {
    local service_name=$1
    local port=$2
    local max_attempts=${3:-30}
    local check_url=${4:-""}
    
    echo "${STEP_PREFIX} Waiting for ${service_name}..."
    for i in $(seq 1 $max_attempts); do
        if [ -n "$check_url" ]; then
            # HTTP health check
            if curl -s "$check_url" >/dev/null 2>&1; then
                echo "${SUCCESS_PREFIX} ${service_name} is ready"
                return 0
            fi
        else
            # TCP port check
            if port_in_use $port; then
                echo "${SUCCESS_PREFIX} ${service_name} is ready"
                return 0
            fi
        fi
        sleep 1
    done
    echo "${ERROR_PREFIX} ${service_name} failed to start within ${max_attempts} seconds"
    return 1
}

# Start MongoDB
echo "${STEP_PREFIX} Starting MongoDB..."
MONGOD_PATH="$RESOURCE_DIR/databases/mongodb/$PLATFORM/$ARCH/mongod"
if [ ! -f "$MONGOD_PATH" ]; then
    echo "${ERROR_PREFIX} MongoDB binary not found at: $MONGOD_PATH"
    exit 1
fi

# Create MongoDB data directory
MONGODB_DATA_DIR="$APP_DATA_DIR/mongodb/data"
mkdir -p "$MONGODB_DATA_DIR"

# Start MongoDB
"$MONGOD_PATH" \
    --dbpath "$MONGODB_DATA_DIR" \
    --port 27017 \
    --bind_ip 127.0.0.1 \
    --wiredTigerCacheSizeGB 0.5 \
    > "$APP_DATA_DIR/mongodb.log" 2>&1 &
MONGODB_PID=$!

# Wait for MongoDB
if wait_for_service "MongoDB" 27017 30; then
    echo "${PROGRESS_PREFIX} mongodb:ready"
else
    echo "${PROGRESS_PREFIX} mongodb:failed"
    kill $MONGODB_PID 2>/dev/null
    exit 1
fi

# Start InfluxDB
echo "${STEP_PREFIX} Starting InfluxDB..."
INFLUXD_PATH="$RESOURCE_DIR/databases/influxdb/$PLATFORM/$ARCH/influxd"
if [ ! -f "$INFLUXD_PATH" ]; then
    echo "${ERROR_PREFIX} InfluxDB binary not found at: $INFLUXD_PATH"
    exit 1
fi

# Create InfluxDB data directory
INFLUXDB_DATA_DIR="$APP_DATA_DIR/influxdb/data"
mkdir -p "$INFLUXDB_DATA_DIR"

# InfluxDB paths and default token (used by collector/report; may be updated after setup)
BOLT_PATH="$INFLUXDB_DATA_DIR/influxdb.bolt"
# Run setup on first run (no bolt file) OR when API says setup is allowed
NEEDS_SETUP="false"
if [ ! -f "$BOLT_PATH" ]; then
    NEEDS_SETUP="true"
fi
RESOLVED_INFLUX_TOKEN="screenjournal-admin-token-change-in-production"

# Start InfluxDB
export INFLUXD_DATA_DIR="$INFLUXDB_DATA_DIR"
export INFLUXD_BOLT_PATH="$BOLT_PATH"
export INFLUXD_ENGINE_PATH="$INFLUXDB_DATA_DIR/engine"

"$INFLUXD_PATH" \
    --http-bind-address 127.0.0.1:8086 \
    --log-level error \
    > "$APP_DATA_DIR/influxdb.log" 2>&1 &
INFLUXDB_PID=$!

# Wait for InfluxDB to be ready
if wait_for_service "InfluxDB" 8086 30 "http://localhost:8086/health"; then
    echo "${PROGRESS_PREFIX} influxdb:ready"
    
    # Also check API: run setup when API says allowed (handles partial/corrupt state)
    sleep 2  # Give InfluxDB a moment to fully initialize
    SETUP_STATUS=$(curl -s http://localhost:8086/api/v2/setup 2>&1)
    if echo "$SETUP_STATUS" | grep -q "\"allowed\":true"; then
        echo "${STEP_PREFIX} InfluxDB needs setup (detected via API)"
        NEEDS_SETUP="true"
    fi
    if [ "$NEEDS_SETUP" = "true" ]; then
        echo "${STEP_PREFIX} InfluxDB needs setup (first run or API indicated setup required)"
    else
        echo "${STEP_PREFIX} InfluxDB appears to be already set up"
    fi
    
    # Default token for collector and report (used if we don't run setup or response has no token)
    RESOLVED_INFLUX_TOKEN="screenjournal-admin-token-change-in-production"
    
    # If setup is needed, use InfluxDB REST API to set it up
    if [ "$NEEDS_SETUP" = "true" ]; then
        echo "${STEP_PREFIX} Setting up InfluxDB (creating user, org, bucket)..."
        
        SETUP_USERNAME="admin"
        SETUP_PASSWORD="admin123"
        SETUP_ORG="screenjournal-org"
        SETUP_BUCKET="screenjournal-metrics"
        SETUP_TOKEN="screenjournal-admin-token-change-in-production"
        
        SETUP_RESPONSE=$(curl -s -X POST http://localhost:8086/api/v2/setup \
            -H "Content-Type: application/json" \
            -d "{
                \"username\": \"$SETUP_USERNAME\",
                \"password\": \"$SETUP_PASSWORD\",
                \"org\": \"$SETUP_ORG\",
                \"bucket\": \"$SETUP_BUCKET\",
                \"token\": \"$SETUP_TOKEN\"
            }" 2>&1)
        
        if echo "$SETUP_RESPONSE" | grep -q "\"user\"" || echo "$SETUP_RESPONSE" | grep -q "\"auth\""; then
            echo "${SUCCESS_PREFIX} InfluxDB setup completed successfully"
            # Use token from response if present (some InfluxDB versions return a new token)
            EXTRACTED_TOKEN=$(echo "$SETUP_RESPONSE" | grep -oE '"token"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed -n 's/.*:[[:space:]]*"\([^"]*\)".*/\1/p')
            if [ -n "$EXTRACTED_TOKEN" ]; then
                RESOLVED_INFLUX_TOKEN="$EXTRACTED_TOKEN"
                echo "${STEP_PREFIX} Using token from InfluxDB setup response"
            fi
            sleep 1
            VERIFY_STATUS=$(curl -s http://localhost:8086/api/v2/setup 2>&1)
            if echo "$VERIFY_STATUS" | grep -q "\"allowed\":false"; then
                echo "${SUCCESS_PREFIX} InfluxDB setup verified"
            else
                echo "${ERROR_PREFIX} InfluxDB setup may have failed - verification failed"
            fi
        else
            echo "${ERROR_PREFIX} InfluxDB setup failed. Response: $SETUP_RESPONSE"
            echo "${ERROR_PREFIX} You may need to visit http://localhost:8086 to complete setup manually."
            echo "${ERROR_PREFIX} Use these credentials:"
            echo "${ERROR_PREFIX}   Username: $SETUP_USERNAME"
            echo "${ERROR_PREFIX}   Password: $SETUP_PASSWORD"
            echo "${ERROR_PREFIX}   Org: $SETUP_ORG"
            echo "${ERROR_PREFIX}   Bucket: $SETUP_BUCKET"
        fi
    fi
    
    # Verify our token works (avoid 'unauthorized' writes later)
    BUCKETS_HTTP=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Token $RESOLVED_INFLUX_TOKEN" "http://localhost:8086/api/v2/buckets?org=screenjournal-org" 2>&1)
    if [ "$BUCKETS_HTTP" = "401" ] || [ "$BUCKETS_HTTP" = "000" ]; then
        echo "${ERROR_PREFIX} InfluxDB token verification failed (HTTP $BUCKETS_HTTP). Writes will fail with 'unauthorized'."
        echo "${ERROR_PREFIX} To fix: quit the app, delete the InfluxDB data directory, then restart:"
        echo "${ERROR_PREFIX}   rm -rf \"$INFLUXDB_DATA_DIR\""
    fi
else
    echo "${PROGRESS_PREFIX} influxdb:failed"
    kill $INFLUXDB_PID 2>/dev/null
    exit 1
fi

# Start Collector
echo "${STEP_PREFIX} Starting Collector..."
COLLECTOR_BINARY="$RESOURCE_DIR/binaries/sj-collector"
if [ ! -f "$COLLECTOR_BINARY" ]; then
    echo "${ERROR_PREFIX} Collector binary not found at: $COLLECTOR_BINARY"
    exit 1
fi

# Create storage directory
STORAGE_DIR="$APP_DATA_DIR/storage"
mkdir -p "$STORAGE_DIR"

# Start collector with environment variables (use resolved InfluxDB token)
cd "$APP_DATA_DIR"
SERVER_HOST=0.0.0.0 \
SERVER_PORT=8080 \
JWT_SECRET=screenjournal-bundled-secret-key \
INFLUXDB2_URL=http://localhost:8086 \
INFLUXDB2_TOKEN="$RESOLVED_INFLUX_TOKEN" \
INFLUXDB2_ORG=screenjournal-org \
INFLUXDB2_BUCKET=screenjournal-metrics \
STORAGE_BASE_PATH="$STORAGE_DIR" \
STORAGE_BASE_URL=http://localhost:8080/storage \
"$COLLECTOR_BINARY" > "$APP_DATA_DIR/collector.log" 2>&1 &
COLLECTOR_PID=$!

# Wait for Collector
sleep 2
if wait_for_service "Collector" 8080 30 "http://localhost:8080/health"; then
    echo "${PROGRESS_PREFIX} collector:ready"
else
    echo "${PROGRESS_PREFIX} collector:failed"
    kill $COLLECTOR_PID 2>/dev/null
    exit 1
fi

# Start Report Service
echo "${STEP_PREFIX} Starting Report Service..."
REPORT_BINARY="$RESOURCE_DIR/binaries/sj-tracker-report"
if [ ! -f "$REPORT_BINARY" ]; then
    echo "${ERROR_PREFIX} Report service binary not found at: $REPORT_BINARY"
    exit 1
fi

# Start report service with environment variables (use resolved InfluxDB token)
cd "$APP_DATA_DIR"
PORT=8085 \
HOST=0.0.0.0 \
INFLUXDB2_URL=http://localhost:8086 \
INFLUXDB2_TOKEN="$RESOLVED_INFLUX_TOKEN" \
INFLUXDB2_ORG=screenjournal-org \
INFLUXDB2_BUCKET=screenjournal-metrics \
MONGODB_HOST=localhost \
MONGODB_PORT=27017 \
MONGODB_DATABASE=reports \
MONGODB_USERNAME=admin \
MONGODB_PASSWORD=admin123 \
MONGODB_AUTH_SOURCE=admin \
OPENAI_API_KEY="" \
"$REPORT_BINARY" > "$APP_DATA_DIR/report.log" 2>&1 &
REPORT_PID=$!

# Wait for Report Service
sleep 2
if wait_for_service "Report Service" 8085 30 "http://localhost:8085/health"; then
    echo "${PROGRESS_PREFIX} report:ready"
else
    echo "${PROGRESS_PREFIX} report:failed"
    kill $REPORT_PID 2>/dev/null
    exit 1
fi

# Start Chat Agent (using PyInstaller standalone executable)
echo "${STEP_PREFIX} Starting Chat Agent..."
CHAT_AGENT_EXE="$RESOURCE_DIR/python/sj-tracker-chat-agent/sj-chat-agent"

if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
    CHAT_AGENT_EXE="$RESOURCE_DIR/python/sj-tracker-chat-agent/sj-chat-agent.exe"
fi

if [ ! -f "$CHAT_AGENT_EXE" ]; then
    echo "${ERROR_PREFIX} Chat agent executable not found at: $CHAT_AGENT_EXE"
    exit 1
fi

# Make sure it's executable (Unix)
if [[ "$OSTYPE" != "msys" ]] && [[ "$OSTYPE" != "win32" ]]; then
    chmod +x "$CHAT_AGENT_EXE"
fi

# Start chat agent using the standalone executable
cd "$APP_DATA_DIR"
BACKEND_URL=http://localhost:8085 \
CHAT_AGENT_PORT=8087 \
HOST=0.0.0.0 \
"$CHAT_AGENT_EXE" > "$APP_DATA_DIR/chat-agent.log" 2>&1 &
CHAT_AGENT_PID=$!

# Wait for Chat Agent (give it more time - Python startup can be slow)
sleep 3
if wait_for_service "Chat Agent" 8087 60 "http://localhost:8087/health"; then
    echo "${PROGRESS_PREFIX} chat_agent:ready"
else
    # Don't exit on chat agent failure - it's not critical for basic functionality
    echo "${PROGRESS_PREFIX} chat_agent:failed"
    # Check if process is still running (might be starting slowly)
    if kill -0 $CHAT_AGENT_PID 2>/dev/null; then
        echo "${PROGRESS_PREFIX} chat_agent:starting (process running, may start later)"
    else
        echo "${PROGRESS_PREFIX} chat_agent:failed (process exited)"
    fi
    # Continue anyway - chat agent is optional
fi

# Start Report Frontend (optional - requires Node.js)
echo "${STEP_PREFIX} Starting Report Frontend..."
FRONTEND_DIR="$RESOURCE_DIR/frontend/sj-tracker-frontend"

# Function to find Node.js in common locations
find_node() {
    # Try command first (uses PATH)
    if command -v node >/dev/null 2>&1; then
        command -v node
        return 0
    fi
    
    # Try common installation locations
    local common_paths=(
        "/usr/local/bin/node"
        "/opt/homebrew/bin/node"
        "/usr/bin/node"
        "$HOME/.nvm/versions/node/*/bin/node"
    )
    
    for path in "${common_paths[@]}"; do
        # Handle glob patterns
        for expanded in $path; do
            if [ -f "$expanded" ] && [ -x "$expanded" ]; then
                echo "$expanded"
                return 0
            fi
        done
    done
    
    return 1
}

NODE_EXE=$(find_node)

if [ ! -d "$FRONTEND_DIR" ]; then
    echo "${PROGRESS_PREFIX} frontend:skipped (Frontend directory not found at: $FRONTEND_DIR)"
elif [ -z "$NODE_EXE" ]; then
    echo "${PROGRESS_PREFIX} frontend:skipped (Node.js not found - please install Node.js to use the report frontend)"
else
    NEXT_BIN="$FRONTEND_DIR/node_modules/.bin/next"
    if [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "win32" ]]; then
        NEXT_BIN="${NEXT_BIN}.cmd"
    fi
    
    if [ ! -f "$NEXT_BIN" ]; then
        echo "${PROGRESS_PREFIX} frontend:skipped (Next.js binary not found at: $NEXT_BIN)"
        echo "${STEP_PREFIX} Checking if .next directory exists..."
        if [ ! -d "$FRONTEND_DIR/.next" ]; then
            echo "${ERROR_PREFIX} Frontend not built - .next directory missing"
        fi
    else
        # Check if standalone build exists (Next.js standalone mode)
        STANDALONE_DIR="$FRONTEND_DIR/.next/standalone"
        if [ -d "$STANDALONE_DIR" ] && [ -f "$STANDALONE_DIR/server.js" ]; then
            echo "${STEP_PREFIX} Using Next.js standalone build"
            cd "$STANDALONE_DIR"
            # In standalone mode, the server.js is in the root
            # Set PORT environment variable for Next.js
            NODE_ENV=production PORT=3030 "$NODE_EXE" server.js > "$APP_DATA_DIR/frontend.log" 2>&1 &
            FRONTEND_PID=$!
            echo "${STEP_PREFIX} Started standalone server from: $STANDALONE_DIR"
        else
            echo "${STEP_PREFIX} Standalone build not found, trying standard Next.js build"
            echo "${STEP_PREFIX} Starting Next.js server from: $FRONTEND_DIR"
            echo "${STEP_PREFIX} Using Node.js at: $NODE_EXE"
            cd "$FRONTEND_DIR"
            # Try using npx next start which might work better
            if command -v npx >/dev/null 2>&1; then
                NODE_ENV=production npx next start -p 3030 > "$APP_DATA_DIR/frontend.log" 2>&1 &
                FRONTEND_PID=$!
            else
                NODE_ENV=production "$NODE_EXE" "$NEXT_BIN" start -p 3030 > "$APP_DATA_DIR/frontend.log" 2>&1 &
                FRONTEND_PID=$!
            fi
        fi
        echo "${STEP_PREFIX} Frontend process started with PID: $FRONTEND_PID"
        
        sleep 3
        if wait_for_service "Frontend" 3030 30 "http://localhost:3030"; then
            echo "${PROGRESS_PREFIX} frontend:ready"
        else
            echo "${PROGRESS_PREFIX} frontend:failed"
            echo "${ERROR_PREFIX} Frontend failed to start. Check logs at: $APP_DATA_DIR/frontend.log"
            # Check if process is still running
            if kill -0 $FRONTEND_PID 2>/dev/null; then
                echo "${STEP_PREFIX} Frontend process is still running, may start later"
            else
                echo "${ERROR_PREFIX} Frontend process exited"
            fi
            # Don't kill it - let it continue trying
        fi
    fi
fi

# All services started
echo "${SUCCESS_PREFIX} All services started successfully"
echo "${PROGRESS_PREFIX} all:ready"

# Keep script running and track PIDs
echo "MONGODB_PID=$MONGODB_PID" > "$APP_DATA_DIR/service_pids.txt"
echo "INFLUXDB_PID=$INFLUXDB_PID" >> "$APP_DATA_DIR/service_pids.txt"
echo "COLLECTOR_PID=$COLLECTOR_PID" >> "$APP_DATA_DIR/service_pids.txt"
echo "REPORT_PID=$REPORT_PID" >> "$APP_DATA_DIR/service_pids.txt"
echo "CHAT_AGENT_PID=$CHAT_AGENT_PID" >> "$APP_DATA_DIR/service_pids.txt"
if [ -n "$FRONTEND_PID" ]; then
    echo "FRONTEND_PID=$FRONTEND_PID" >> "$APP_DATA_DIR/service_pids.txt"
fi

# Wait for all background processes
wait

