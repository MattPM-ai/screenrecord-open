#!/bin/bash

# Script to download and prepare MongoDB and InfluxDB binaries for bundling
# This script downloads platform-specific binaries and extracts them to the resources directory

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"
RESOURCES_DIR="$PROJECT_ROOT/screenrecord/apps/desktop/src-tauri/resources/databases"

echo -e "${GREEN}📦 Preparing Database Binaries${NC}"
echo ""

# Detect platform
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
    ARCH="x86_64"
elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]] || [[ "$OSTYPE" == "win32" ]] || [[ -n "$WINDIR" ]]; then
    # Windows (detected via Git Bash, Cygwin, or WINDIR environment variable)
    PLATFORM="windows"
    ARCH="x86_64"
else
    echo -e "${RED}❌ Unsupported platform: $OSTYPE${NC}"
    exit 1
fi

echo -e "${YELLOW}Detected platform: $PLATFORM/$ARCH${NC}"
echo ""

# Create directories
mkdir -p "$RESOURCES_DIR/mongodb/$PLATFORM/$ARCH"
mkdir -p "$RESOURCES_DIR/influxdb/$PLATFORM/$ARCH"

# MongoDB version
MONGO_VERSION="7.0.0"
MONGO_BASE_URL="https://fastdl.mongodb.org"

# InfluxDB version (using latest stable 2.x)
# Check available versions: https://github.com/influxdata/influxdb/releases
# Note: Some versions may not have separate arm64 builds - will fall back to amd64
INFLUX_VERSION="2.7.0"

# Download MongoDB
echo -e "${BLUE}📥 Downloading MongoDB...${NC}"
MONGO_TEMP_DIR=$(mktemp -d)
trap "rm -rf $MONGO_TEMP_DIR" EXIT

if [[ "$PLATFORM" == "darwin" ]]; then
    if [[ "$ARCH" == "aarch64" ]]; then
        MONGO_URL="$MONGO_BASE_URL/osx/mongodb-macos-arm64-$MONGO_VERSION.tgz"
        MONGO_ARCHIVE="mongodb-macos-arm64-$MONGO_VERSION.tgz"
    else
        MONGO_URL="$MONGO_BASE_URL/osx/mongodb-macos-x86_64-$MONGO_VERSION.tgz"
        MONGO_ARCHIVE="mongodb-macos-x86_64-$MONGO_VERSION.tgz"
    fi
elif [[ "$PLATFORM" == "linux" ]]; then
    MONGO_URL="$MONGO_BASE_URL/linux/mongodb-linux-x86_64-ubuntu2204-$MONGO_VERSION.tgz"
    MONGO_ARCHIVE="mongodb-linux-x86_64-ubuntu2204-$MONGO_VERSION.tgz"
elif [[ "$PLATFORM" == "windows" ]]; then
    MONGO_URL="$MONGO_BASE_URL/windows/mongodb-windows-x86_64-$MONGO_VERSION.zip"
    MONGO_ARCHIVE="mongodb-windows-x86_64-$MONGO_VERSION.zip"
fi

cd "$MONGO_TEMP_DIR"
if [ ! -f "$MONGO_ARCHIVE" ]; then
    echo -e "${YELLOW}Downloading from: $MONGO_URL${NC}"
    curl -L -f -o "$MONGO_ARCHIVE" "$MONGO_URL"
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Failed to download MongoDB${NC}"
        exit 1
    fi
fi

# Verify the file was downloaded correctly (should be > 10MB)
FILE_SIZE=$(stat -f%z "$MONGO_ARCHIVE" 2>/dev/null || stat -c%s "$MONGO_ARCHIVE" 2>/dev/null || echo "0")
if [ "$FILE_SIZE" -lt 10485760 ]; then
    echo -e "${RED}❌ Downloaded file is too small ($FILE_SIZE bytes), likely an error page${NC}"
    echo -e "${YELLOW}Checking file contents...${NC}"
    head -20 "$MONGO_ARCHIVE"
    rm -f "$MONGO_ARCHIVE"
    exit 1
fi

echo -e "${YELLOW}Extracting MongoDB (file size: $FILE_SIZE bytes)...${NC}"
if [[ "$PLATFORM" == "windows" ]]; then
    # Windows uses zip files
    unzip -q "$MONGO_ARCHIVE"
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Failed to extract MongoDB archive${NC}"
        exit 1
    fi
    MONGO_EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "mongodb-*" | head -1)
    if [ -z "$MONGO_EXTRACTED_DIR" ]; then
        echo -e "${RED}❌ Failed to find extracted MongoDB directory${NC}"
        exit 1
    fi
    cp "$MONGO_EXTRACTED_DIR/bin/mongod.exe" "$RESOURCES_DIR/mongodb/$PLATFORM/$ARCH/mongod.exe"
else
    # Unix uses tar.gz
    tar -xzf "$MONGO_ARCHIVE"
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Failed to extract MongoDB archive${NC}"
        echo -e "${YELLOW}File type: $(file "$MONGO_ARCHIVE")${NC}"
        exit 1
    fi
    MONGO_EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "mongodb-*" | head -1)
    if [ -z "$MONGO_EXTRACTED_DIR" ]; then
        echo -e "${RED}❌ Failed to find extracted MongoDB directory${NC}"
        exit 1
    fi
    cp "$MONGO_EXTRACTED_DIR/bin/mongod" "$RESOURCES_DIR/mongodb/$PLATFORM/$ARCH/"
    chmod +x "$RESOURCES_DIR/mongodb/$PLATFORM/$ARCH/mongod"
fi

echo -e "${GREEN}✅ MongoDB binary prepared${NC}"
echo ""

# Download InfluxDB
echo -e "${BLUE}📥 Downloading InfluxDB...${NC}"
INFLUX_TEMP_DIR=$(mktemp -d)
trap "rm -rf $INFLUX_TEMP_DIR" EXIT

if [[ "$PLATFORM" == "darwin" ]]; then
    # Note: InfluxDB 2.7.x may not have separate arm64 builds for macOS
    # The amd64 build works on Apple Silicon via Rosetta 2
    # Use official download site as primary (more reliable)
    if [[ "$ARCH" == "aarch64" ]]; then
        # Try arm64 first from official site, but fall back to amd64 if not available
        INFLUX_URL="https://dl.influxdata.com/influxdb/releases/influxdb2-${INFLUX_VERSION}-darwin-arm64.tar.gz"
        INFLUX_ARCHIVE="influxdb2-${INFLUX_VERSION}-darwin-arm64.tar.gz"
        # Fallback to amd64 (works via Rosetta 2 on Apple Silicon)
        INFLUX_ALT_URL="https://dl.influxdata.com/influxdb/releases/influxdb2-${INFLUX_VERSION}-darwin-amd64.tar.gz"
        INFLUX_ALT_ARCHIVE="influxdb2-${INFLUX_VERSION}-darwin-amd64.tar.gz"
        INFLUX_ALT_URL2="https://github.com/influxdata/influxdb/releases/download/v${INFLUX_VERSION}/influxdb2-${INFLUX_VERSION}-darwin-amd64.tar.gz"
    else
        INFLUX_URL="https://dl.influxdata.com/influxdb/releases/influxdb2-${INFLUX_VERSION}-darwin-amd64.tar.gz"
        INFLUX_ARCHIVE="influxdb2-${INFLUX_VERSION}-darwin-amd64.tar.gz"
        INFLUX_ALT_URL="https://github.com/influxdata/influxdb/releases/download/v${INFLUX_VERSION}/influxdb2-${INFLUX_VERSION}-darwin-amd64.tar.gz"
        INFLUX_ALT_ARCHIVE=""
        INFLUX_ALT_URL2=""
    fi
elif [[ "$PLATFORM" == "linux" ]]; then
    INFLUX_URL="https://dl.influxdata.com/influxdb/releases/influxdb2-${INFLUX_VERSION}-linux-amd64.tar.gz"
    INFLUX_ARCHIVE="influxdb2-${INFLUX_VERSION}-linux-amd64.tar.gz"
    INFLUX_ALT_URL="https://github.com/influxdata/influxdb/releases/download/v${INFLUX_VERSION}/influxdb2-${INFLUX_VERSION}-linux-amd64.tar.gz"
    INFLUX_ALT_URL2=""
elif [[ "$PLATFORM" == "windows" ]]; then
    INFLUX_URL="https://dl.influxdata.com/influxdb/releases/influxdb2-${INFLUX_VERSION}-windows-amd64.zip"
    INFLUX_ARCHIVE="influxdb2-${INFLUX_VERSION}-windows-amd64.zip"
    INFLUX_ALT_URL=""
    INFLUX_ALT_URL2=""
fi

cd "$INFLUX_TEMP_DIR"
if [ ! -f "$INFLUX_ARCHIVE" ]; then
    echo -e "${YELLOW}Downloading from: $INFLUX_URL${NC}"
    # Temporarily disable set -e to allow fallback on curl failure
    set +e
    curl -L -f -o "$INFLUX_ARCHIVE" "$INFLUX_URL"
    CURL_EXIT=$?
    set -e
    
    if [ $CURL_EXIT -ne 0 ] || [ ! -s "$INFLUX_ARCHIVE" ]; then
        echo -e "${YELLOW}Primary URL failed (arm64 build may not exist), trying amd64 build: $INFLUX_ALT_URL${NC}"
        echo -e "${YELLOW}(amd64 build works on Apple Silicon via Rosetta 2)${NC}"
        rm -f "$INFLUX_ARCHIVE"
        if [ -n "$INFLUX_ALT_ARCHIVE" ]; then
            # Use different archive name for fallback
            INFLUX_ARCHIVE="$INFLUX_ALT_ARCHIVE"
        fi
        set +e
        curl -L -f -o "$INFLUX_ARCHIVE" "$INFLUX_ALT_URL"
        CURL_EXIT=$?
        set -e
        if [ $CURL_EXIT -ne 0 ] || [ ! -s "$INFLUX_ARCHIVE" ]; then
            if [ -n "$INFLUX_ALT_URL2" ]; then
                echo -e "${YELLOW}Second alternative failed, trying third: $INFLUX_ALT_URL2${NC}"
                rm -f "$INFLUX_ARCHIVE"
                set +e
                curl -L -f -o "$INFLUX_ARCHIVE" "$INFLUX_ALT_URL2"
                CURL_EXIT=$?
                set -e
                if [ $CURL_EXIT -ne 0 ]; then
                    echo -e "${RED}❌ Failed to download InfluxDB from all URLs${NC}"
                    echo -e "${RED}   Tried:${NC}"
                    echo -e "${RED}   1. $INFLUX_URL${NC}"
                    echo -e "${RED}   2. $INFLUX_ALT_URL${NC}"
                    echo -e "${RED}   3. $INFLUX_ALT_URL2${NC}"
                    exit 1
                fi
            else
                echo -e "${RED}❌ Failed to download InfluxDB from both URLs${NC}"
                echo -e "${RED}   Tried:${NC}"
                echo -e "${RED}   1. $INFLUX_URL${NC}"
                echo -e "${RED}   2. $INFLUX_ALT_URL${NC}"
                exit 1
            fi
        fi
    fi
fi

# Verify the file was downloaded correctly (should be > 1MB)
FILE_SIZE=$(stat -f%z "$INFLUX_ARCHIVE" 2>/dev/null || stat -c%s "$INFLUX_ARCHIVE" 2>/dev/null || echo "0")
if [ "$FILE_SIZE" -lt 1048576 ]; then
    echo -e "${RED}❌ Downloaded file is too small ($FILE_SIZE bytes), likely an error page${NC}"
    echo -e "${YELLOW}Checking file contents...${NC}"
    head -20 "$INFLUX_ARCHIVE"
    rm -f "$INFLUX_ARCHIVE"
    exit 1
fi

echo -e "${YELLOW}Extracting InfluxDB (file size: $FILE_SIZE bytes)...${NC}"
if [[ "$PLATFORM" == "windows" ]]; then
    # Windows uses zip files
    unzip -q "$INFLUX_ARCHIVE"
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Failed to extract InfluxDB archive${NC}"
        exit 1
    fi
    # Try both naming patterns: influxdb2-* (hyphens) and influxdb2_* (underscores)
    INFLUX_EXTRACTED_DIR=$(find . -maxdepth 1 -type d \( -name "influxdb2-*" -o -name "influxdb2_*" \) | head -1)
    if [ -z "$INFLUX_EXTRACTED_DIR" ]; then
        echo -e "${RED}❌ Failed to find extracted InfluxDB directory${NC}"
        echo -e "${YELLOW}Contents of extraction directory:${NC}"
        ls -la
        exit 1
    fi
    cp "$INFLUX_EXTRACTED_DIR/influxd.exe" "$RESOURCES_DIR/influxdb/$PLATFORM/$ARCH/influxd.exe"
else
    # Unix uses tar.gz
    tar -xzf "$INFLUX_ARCHIVE"
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Failed to extract InfluxDB archive${NC}"
        echo -e "${YELLOW}File type: $(file "$INFLUX_ARCHIVE")${NC}"
        exit 1
    fi
    # Try both naming patterns: influxdb2-* (hyphens) and influxdb2_* (underscores)
    INFLUX_EXTRACTED_DIR=$(find . -maxdepth 1 -type d \( -name "influxdb2-*" -o -name "influxdb2_*" \) | head -1)
    if [ -z "$INFLUX_EXTRACTED_DIR" ]; then
        echo -e "${RED}❌ Failed to find extracted InfluxDB directory${NC}"
        echo -e "${YELLOW}Contents of extraction directory:${NC}"
        ls -la
        exit 1
    fi
    cp "$INFLUX_EXTRACTED_DIR/influxd" "$RESOURCES_DIR/influxdb/$PLATFORM/$ARCH/"
    chmod +x "$RESOURCES_DIR/influxdb/$PLATFORM/$ARCH/influxd"
fi

echo -e "${GREEN}✅ InfluxDB binary prepared${NC}"
echo ""

echo -e "${GREEN}✨ Database binaries prepared successfully!${NC}"
echo -e "${GREEN}   Location: $RESOURCES_DIR${NC}"

