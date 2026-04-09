# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ScreenRecord Collector is a Go-based WebSocket service that streams time-series data to InfluxDB 3.x. It features JWT authentication and automatically tags metrics with user and organization metadata.

## Common Commands

### Development

```bash
# Install dependencies
go mod download

# Build the server
go build -o bin/server ./cmd/server

# Run the server (requires .env configuration)
go run ./cmd/server

# Run tests
go test ./...

# Run tests with coverage
go test -cover ./...

# Update dependencies
go mod tidy
```

### Docker

```bash
# Build and start with Docker Compose (includes Caddy)
docker-compose up -d

# View logs
docker-compose logs -f sj-collector

# Rebuild after code changes
docker-compose up -d --build

# Stop services
docker-compose down

# Build Docker image manually
docker build -t sj-collector .
```

## Architecture

### Dependency Injection Flow

The application follows a constructor-based dependency injection pattern:

1. **main.go** loads configuration and creates service instances
2. Services (JWT, InfluxDB) are initialized and injected into handlers
3. Handlers (Auth, TimeSeries) receive services via constructors
4. Router wires handlers to HTTP/WebSocket endpoints

Example flow:
```
main() → NewJWTService() → NewAuthHandler(jwtService) → router.POST("/mock-auth", handler.MockAuth)
main() → NewInfluxService() → NewTimeSeriesHandler(jwtService, influxService) → router.GET("/time-series", handler.HandleWebSocket)
```

### WebSocket Authentication Protocol

The `/time-series` endpoint implements a two-phase protocol:

1. **Authentication Phase** (handlers/timeseries.go:authenticate):
   - Client must send `$AUTH <jwt-token>` as first message
   - Server validates token and extracts `user` and `org` claims
   - Server responds with `AUTH_SUCCESS`
   - Connection is rejected if auth fails

2. **Data Streaming Phase** (handlers/timeseries.go:handleMessages):
   - Client sends InfluxDB line protocol messages
   - Server automatically injects `user=<user>,org=<org>` tags
   - Server writes to InfluxDB and responds with `OK` or `ERROR: <msg>`

### Tag Injection Mechanism

The `InfluxService.addTagsToLineProtocol()` function (services/influx.go:49-70) modifies line protocol by:
- Finding the first space (separates tags from fields)
- Inserting `,user=<user>,org=<org>` before that space

Example transformation:
```
Input:  "temperature,location=room1 value=23.5"
Output: "temperature,location=room1,user=alice,org=wonderland value=23.5"
```

## Configuration

Environment variables are loaded via `internal/config/config.go`. Required variables:
- `INFLUXDB_URL`: InfluxDB instance URL
- `INFLUXDB_TOKEN`: InfluxDB auth token
- `INFLUXDB_DATABASE`: Target database name

Optional variables (with defaults):
- `SERVER_HOST`: Bind address (default: `0.0.0.0`)
- `SERVER_PORT`: Port (default: `8080`)
- `JWT_SECRET`: JWT signing key (default: `your-secret-key-change-in-production`)

Docker-specific variables (for docker-compose.yml):
- `DOMAIN`: Domain for Caddy reverse proxy (default: `sj-collector.localhost`)
- `TLS_EMAIL`: Email for Let's Encrypt or `internal` for self-signed certs (default: `internal`)

The application will fail to start if required InfluxDB variables are missing.

## Docker Deployment

The project includes a multi-stage Dockerfile and Docker Compose setup with Caddy reverse proxy:

- **Dockerfile**: Multi-stage build (Go builder + Alpine runtime) for minimal image size
- **docker-compose.yml**: Orchestrates sj-collector + Caddy with automatic HTTPS
- **Caddy labels**: Configured for automatic reverse proxy, WebSocket support, and TLS

### Caddy Integration

The service uses Caddy Docker Proxy (`lucaslorentz/caddy-docker-proxy`) which automatically configures reverse proxy via Docker labels:

- `caddy`: Defines the domain
- `caddy.reverse_proxy`: Configures upstream to port 8080
- `caddy.tls`: Handles TLS (Let's Encrypt or internal)
- `caddy.header`: Preserves WebSocket headers (Connection, Upgrade)

WebSocket connections to `/time-series` work automatically through Caddy.

## Adding New Features

### New REST Endpoint

1. Add handler method to existing or new handler in `internal/handlers/`
2. Wire to router in `cmd/server/main.go:setupRouter()`

### New Service

1. Create service in `internal/services/`
2. Initialize in `cmd/server/main.go:main()`
3. Inject into handlers via constructor

### New WebSocket Endpoint

1. Create handler in `internal/handlers/`
2. Use `gorilla/websocket.Upgrader` (see handlers/timeseries.go:16-22)
3. Register route in `cmd/server/main.go:setupRouter()`

### New Configuration

1. Add field to struct in `internal/config/config.go`
2. Load from environment in `config.Load()`
3. Access via config object passed through dependency injection

## Testing Strategy

The codebase currently has no tests. When adding tests:
- Place test files alongside source (e.g., `services/jwt_test.go`)
- Mock InfluxDB client for `InfluxService` tests
- Test WebSocket protocol phases separately (auth vs. streaming)
- Use `httptest` for handler tests

## Docker compose guideline
- Do not include version at the top because it's deprecated
- Do not need websocket upgrade caddy tags
- Do not hardcode container name because we will deploy multiple instance of them
