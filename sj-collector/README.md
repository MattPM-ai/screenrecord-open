# ScreenRecord Collector

A high-performance Go service for streaming time-series data to InfluxDB with WebSocket support and JWT authentication.

## Features

- **WebSocket Time Series Streaming**: Stream InfluxDB line protocol data via WebSocket
- **JWT Authentication**: Secure WebSocket sessions with JWT tokens
- **Auto-tagging**: Automatically appends user and organization tags to all metrics
- **Mock Authentication**: Development-friendly endpoint for generating JWT tokens
- **InfluxDB 3 Support**: Native support for InfluxDB 3.x line protocol

## Architecture

The project follows a clean, modular architecture designed for easy extension:

```
sj-collector/
├── cmd/
│   └── server/
│       └── main.go              # Application entry point
├── internal/
│   ├── config/
│   │   └── config.go            # Configuration management
│   ├── handlers/
│   │   ├── auth.go              # Authentication handlers
│   │   └── timeseries.go        # WebSocket handlers
│   ├── services/
│   │   ├── jwt.go               # JWT service
│   │   └── influx.go            # InfluxDB service
│   └── models/
│       └── models.go            # Data models
├── .env.example                 # Environment configuration template
└── README.md
```

## Prerequisites

- Go 1.21 or higher
- InfluxDB 3.x instance
- Access to InfluxDB URL and token

## Installation

1. Clone the repository:
```bash
git clone <your-repo-url>
cd sj-collector
```

2. Install dependencies:
```bash
go mod download
```

3. Configure environment variables:
```bash
cp .env.example .env
# Edit .env with your InfluxDB credentials
```

4. Run the server:
```bash
go run ./cmd/server
```

## Docker Deployment

### Using Docker Compose (Recommended)

The project includes Docker Compose configuration with Caddy reverse proxy:

1. Configure environment variables:
```bash
cp .env.example .env
# Edit .env with your configuration
```

2. Start the services:
```bash
docker-compose up -d
```

3. Access the service:
- Local development: `http://sj-collector.localhost`
- Production: Configure `DOMAIN` in `.env` (e.g., `sj-collector.yourdomain.com`)

### Caddy Configuration

The Docker Compose setup includes automatic HTTPS via Caddy:

- **WebSocket Support**: Automatically configured for `/time-series` endpoint
- **Automatic HTTPS**: Set `TLS_EMAIL=your@email.com` for Let's Encrypt certificates
- **Local Development**: Use `TLS_EMAIL=internal` for self-signed certificates

### Docker Commands

```bash
# Build and start services
docker-compose up -d

# View logs
docker-compose logs -f sj-collector

# Rebuild after code changes
docker-compose up -d --build

# Stop services
docker-compose down

# Stop and remove volumes
docker-compose down -v
```

### Manual Docker Build

```bash
# Build image
docker build -t sj-collector .

# Run container
docker run -d \
  --name sj-collector \
  -p 8080:8080 \
  -e INFLUXDB_URL=your-url \
  -e INFLUXDB_TOKEN=your-token \
  -e INFLUXDB_DATABASE=your-db \
  sj-collector
```

## Configuration

Create a `.env` file based on `.env.example`:

| Variable | Description | Default |
|----------|-------------|---------|
| `SERVER_HOST` | Server bind address | `0.0.0.0` |
| `SERVER_PORT` | Server port | `8080` |
| `JWT_SECRET` | Secret key for signing JWT tokens (must match screenrecord-backend JWT_SECRET for backend token validation) | `your-secret-key-change-in-production` |
| `INFLUXDB_URL` | InfluxDB instance URL | Required |
| `INFLUXDB_TOKEN` | InfluxDB authentication token | Required |
| `INFLUXDB_DATABASE` | InfluxDB database name | Required |
| `DOMAIN` | Domain for Caddy reverse proxy (Docker only) | `sj-collector.localhost` |
| `TLS_EMAIL` | Email for Let's Encrypt or `internal` for self-signed (Docker only) | `internal` |

## API Endpoints

### Health Check

```http
GET /health
```

Returns server health status.

**Response:**
```json
{
  "status": "ok"
}
```

### Mock Authentication

```http
POST /mock-auth
Authorization: Bearer <backend-jwt-token>
Content-Type: application/json
```

Generates a JWT token for the authenticated user and organization. This endpoint is protected and requires a valid JWT token from screenrecord-backend (shared JWT_SECRET).

**Authentication:**
- Requires a valid JWT token from screenrecord-backend in the `Authorization` header
- The token must be signed with the same `JWT_SECRET` used by screenrecord-backend
- Token structure: `{ userId: string, email: string, iat: number, exp: number }`
- **Security**: The `user_id` in the request body must match the `userId` from the authenticated backend token

**Request Body:**
```json
{
  "user": "John Doe",
  "user_id": "123",        // Must match authenticated backend user's userId
  "org": "Acme Corp",
  "org_id": "456",
  "account_id": "789"
}
```

**Security Note:**
- The endpoint validates that `user_id` matches the authenticated backend user, preventing users from generating tokens for other users
- `org_id` and `account_id` are accepted as provided (backend token doesn't include these fields)
- For production, consider validating org/account membership via a user lookup service

**Response:**
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

### Time Series WebSocket

```
ws://localhost:8080/time-series
```

WebSocket endpoint for streaming time-series data to InfluxDB.

**Protocol:**

1. **Authenticate**: First message must be authentication
   ```
   $AUTH <jwt-token>
   ```

2. **Server Response**: On successful authentication
   ```
   AUTH_SUCCESS
   ```

3. **Send Data**: Send InfluxDB line protocol data
   ```
   temperature,location=room1 value=23.5 1234567890000000000
   ```

4. **Server Acknowledgment**: For each message
   ```
   OK
   ```

**Example Client (JavaScript):**
```javascript
const ws = new WebSocket('ws://localhost:8080/time-series');

ws.onopen = () => {
  // Authenticate
  ws.send('$AUTH eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...');
};

ws.onmessage = (event) => {
  console.log('Server:', event.data);

  if (event.data === 'AUTH_SUCCESS') {
    // Send line protocol data
    ws.send('temperature,location=room1 value=23.5');
  }
};
```

**Example Client (Go):**
```go
package main

import (
	"fmt"
	"log"
	"github.com/gorilla/websocket"
)

func main() {
	// Connect to WebSocket
	conn, _, err := websocket.DefaultDialer.Dial("ws://localhost:8080/time-series", nil)
	if err != nil {
		log.Fatal(err)
	}
	defer conn.Close()

	// Authenticate
	err = conn.WriteMessage(websocket.TextMessage, []byte("$AUTH your-jwt-token-here"))
	if err != nil {
		log.Fatal(err)
	}

	// Read auth response
	_, msg, _ := conn.ReadMessage()
	fmt.Println(string(msg)) // "AUTH_SUCCESS"

	// Send line protocol data
	conn.WriteMessage(websocket.TextMessage, []byte("temperature,location=room1 value=23.5"))

	// Read acknowledgment
	_, msg, _ = conn.ReadMessage()
	fmt.Println(string(msg)) // "OK"
}
```

## How It Works

1. **Authentication Flow**:
   - Client obtains JWT token from `/mock-auth` endpoint
   - Client connects to `/time-series` WebSocket
   - Client sends `$AUTH <token>` as first message
   - Server validates token and extracts `user` and `org` claims
   - Server responds with `AUTH_SUCCESS`

2. **Data Streaming**:
   - Client sends InfluxDB line protocol messages
   - Server automatically appends `user` and `org` tags to each message
   - Server writes enhanced data to InfluxDB
   - Server responds with `OK` for successful writes

3. **Example Transformation**:
   ```
   # Client sends:
   temperature,location=room1 value=23.5

   # Server writes to InfluxDB:
   temperature,location=room1,user=john_doe,org=acme_corp value=23.5
   ```

## Development

### Project Structure

- `cmd/server/main.go`: Application entry point and server setup
- `internal/config/`: Configuration loading and validation
- `internal/handlers/`: HTTP and WebSocket request handlers
- `internal/services/`: Business logic (JWT, InfluxDB)
- `internal/models/`: Data structures and types

### Adding New Features

The modular structure makes it easy to extend:

1. **New Endpoints**: Add handlers in `internal/handlers/`
2. **New Services**: Add services in `internal/services/`
3. **New Configuration**: Update `internal/config/config.go`
4. **Wire Up**: Register in `cmd/server/main.go`

### Testing

```bash
# Run tests
go test ./...

# Run with coverage
go test -cover ./...
```

## Production Considerations

- Change `JWT_SECRET` to a strong, random value (must match screenrecord-backend JWT_SECRET)
- Configure `CheckOrigin` in WebSocket upgrader for CORS
- Add rate limiting for endpoints
- Implement proper logging (structured logging)
- Add metrics and monitoring
- Use TLS/SSL for WebSocket connections
- `/mock-auth` is now protected by backend JWT authentication
- Add input validation and sanitization
- Consider connection pooling for InfluxDB

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
