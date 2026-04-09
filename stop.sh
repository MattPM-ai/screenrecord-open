#!/bin/bash

# ============================================================================
# STOP SCRIPT - Stops all services for the ScreenRecord application
# ============================================================================

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}Stopping ScreenRecord services...${NC}\n"

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Stop frontend
if [ -f "$SCRIPT_DIR/logs/frontend.pid" ]; then
    FRONTEND_PID=$(cat "$SCRIPT_DIR/logs/frontend.pid")
    if ps -p $FRONTEND_PID > /dev/null 2>&1; then
        echo -e "${YELLOW}Stopping frontend (PID: $FRONTEND_PID)...${NC}"
        kill $FRONTEND_PID
        rm "$SCRIPT_DIR/logs/frontend.pid"
        echo -e "${GREEN}✓ Frontend stopped${NC}"
    fi
fi

# Stop chat agent
if [ -f "$SCRIPT_DIR/logs/chat-agent.pid" ]; then
    AGENT_PID=$(cat "$SCRIPT_DIR/logs/chat-agent.pid")
    if ps -p $AGENT_PID > /dev/null 2>&1; then
        echo -e "${YELLOW}Stopping chat agent (PID: $AGENT_PID)...${NC}"
        kill $AGENT_PID
        rm "$SCRIPT_DIR/logs/chat-agent.pid"
        echo -e "${GREEN}✓ Chat agent stopped${NC}"
    fi
fi

# Stop backend
if [ -f "$SCRIPT_DIR/logs/backend.pid" ]; then
    BACKEND_PID=$(cat "$SCRIPT_DIR/logs/backend.pid")
    if ps -p $BACKEND_PID > /dev/null 2>&1; then
        echo -e "${YELLOW}Stopping backend (PID: $BACKEND_PID)...${NC}"
        kill $BACKEND_PID
        rm "$SCRIPT_DIR/logs/backend.pid"
        echo -e "${GREEN}✓ Backend stopped${NC}"
    fi
fi

# Stop Docker services
echo -e "${YELLOW}Stopping Docker services...${NC}"
cd "$SCRIPT_DIR"
docker-compose down
echo -e "${GREEN}✓ Docker services stopped${NC}\n"

echo -e "${GREEN}All services stopped.${NC}\n"




