# Local Development Setup Guide

This guide will help you run the screenrecord-collector service locally for testing.

## Prerequisites

- **Go 1.21 or higher** - [Download Go](https://golang.org/dl/)
- **InfluxDB 3.x** - Running instance (local or remote)
- **S3-compatible storage** - AWS S3, MinIO, or other S3-compatible service
- **Access credentials** for both services

## Quick Start

### 1. Install Dependencies

```bash
cd /Users/alexanderwestlake/repos/screenrecord-open/sj-collector
go mod download
```

### 2. Create Environment File

Create a `.env` file in the project root:

```bash
# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# JWT Configuration
JWT_SECRET=your-secret-key-change-in-production

# InfluxDB Configuration (REQUIRED)
INFLUXDB_URL=http://localhost:8181
INFLUXDB_TOKEN=your-influxdb-token-here
INFLUXDB_DATABASE=screenrecord-metrics-dev

# S3 Configuration (REQUIRED)
S3_BUCKET=your-bucket-name
S3_REGION=us-east-1
S3_ACCESS_KEY_ID=your-access-key
S3_SECRET_ACCESS_KEY=your-secret-key
S3_ENDPOINT=  # Optional: Leave empty for AWS S3, or set for MinIO (e.g., http://localhost:9000)
```

### 3. Run the Server

```bash
go run ./cmd/server
```

The server will start on `http://localhost:8080` (or your configured port).

## Testing the Service

### 1. Health Check

```bash
curl http://localhost:8080/health
```

Expected response:
```json
{"status":"ok"}
```

### 2. Get JWT Token (Mock Auth)

**Note:** The `/mock-auth` endpoint now requires authentication with a valid JWT token from screenrecord-backend. You must first obtain a token from screenrecord-backend, then use it to access this endpoint.

```bash
# First, obtain a backend JWT token from screenrecord-backend
# Then use it to access mock-auth:
curl -X POST http://localhost:8080/mock-auth \
  -H "Authorization: Bearer YOUR_BACKEND_JWT_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "user": "John Doe",
    "user_id": "123",
    "org": "Acme Corp",
    "org_id": "456",
    "account_id": "789"
  }'
```

Expected response:
```json
{
  "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
}
```

**Important:** The `JWT_SECRET` environment variable must match the secret used by screenrecord-backend for token validation to work.

Save the token for the next steps.

### 3. Test WebSocket Time Series Endpoint

Using a WebSocket client (like `websocat` or a browser console):

```javascript
// In browser console or Node.js
const ws = new WebSocket('ws://localhost:8080/time-series');

ws.onopen = () => {
  // Authenticate with the token from step 2
  ws.send('$AUTH YOUR_JWT_TOKEN_HERE');
};

ws.onmessage = (event) => {
  console.log('Server:', event.data);
  
  if (event.data === 'AUTH_SUCCESS') {
    // Send test data
    ws.send('afk_status,status=active,hostname=test-laptop duration=3600');
  }
  
  if (event.data === 'OK') {
    console.log('Data written successfully!');
  }
};
```

Or using `websocat` (install via `brew install websocat`):

```bash
echo '$AUTH YOUR_JWT_TOKEN_HERE' | websocat ws://localhost:8080/time-series
# After AUTH_SUCCESS, send:
echo 'afk_status,status=active,hostname=test-laptop duration=3600' | websocat ws://localhost:8080/time-series
```

### 4. Test Screenshot Upload

```bash
# Create a test image first
echo "test" > test.png

# Upload screenshot
curl -X POST http://localhost:8080/screenshots \
  -H "Authorization: Bearer YOUR_JWT_TOKEN_HERE" \
  -F "file=@test.png" \
  -F "timestamp=$(date +%s)" \
  -F "monitor_idx=0"
```

## Using MinIO for Local S3 Testing

If you don't have AWS S3, you can use MinIO locally:

### Install and Run MinIO

```bash
# Install MinIO (macOS)
brew install minio/stable/minio

# Start MinIO server
minio server ~/minio-data --console-address ":9001"
```

MinIO will be available at:
- **API**: `http://localhost:9000`
- **Console**: `http://localhost:9001` (default credentials: minioadmin/minioadmin)

### Configure MinIO in .env

```bash
S3_ENDPOINT=http://localhost:9000
S3_BUCKET=test-bucket
S3_ACCESS_KEY_ID=minioadmin
S3_SECRET_ACCESS_KEY=minioadmin
S3_REGION=us-east-1
```

### Create Bucket in MinIO

1. Open MinIO Console: `http://localhost:9001`
2. Login with `minioadmin` / `minioadmin`
3. Create a new bucket (e.g., `test-bucket`)

## Using Local InfluxDB

### Option 1: Docker (Easiest)

```bash
# Run InfluxDB 3 in Docker
docker run -d \
  --name influxdb \
  -p 8181:8181 \
  -e INFLUXDB_DATABASE=screenrecord-metrics-dev \
  quay.io/influxdb/influxdb:3.0.0
```

### Option 2: Download InfluxDB 3

1. Download from [InfluxDB Downloads](https://www.influxdata.com/downloads/)
2. Extract and run:
```bash
./influxdb3
```

### Get InfluxDB Token

1. Access InfluxDB UI (usually at `http://localhost:8086` or check your setup)
2. Create a token with write permissions
3. Use it in your `.env` file

## Troubleshooting

### "INFLUXDB_URL is required" Error

Make sure your `.env` file exists and contains:
```bash
INFLUXDB_URL=http://localhost:8181
INFLUXDB_TOKEN=your-token
INFLUXDB_DATABASE=your-database
```

### "S3_BUCKET is required" Error

Make sure your `.env` file contains S3 credentials:
```bash
S3_BUCKET=your-bucket
S3_ACCESS_KEY_ID=your-key
S3_SECRET_ACCESS_KEY=your-secret
```

### WebSocket Connection Fails

- Check that the server is running: `curl http://localhost:8080/health`
- Verify the WebSocket URL: `ws://localhost:8080/time-series` (not `http://`)
- Make sure you're sending `$AUTH <token>` as the first message

### InfluxDB Write Errors

- Verify your InfluxDB token has write permissions
- Check that the database exists
- Ensure InfluxDB is accessible at the configured URL

## Development Tips

### Hot Reload (Optional)

Install `air` for automatic reloading during development:

```bash
go install github.com/cosmtrek/air@latest
air
```

### View Logs

The server logs all WebSocket connections and InfluxDB writes. Watch the console output for:
- Connection establishment
- Authentication success/failure
- Data write confirmations
- Error messages

### Test with Test Data

See `test-data/README.md` for sample data you can upload to InfluxDB for testing queries.

## Next Steps

Once running locally, you can:
1. Test the new auth endpoint with all the new fields
2. Verify that InfluxDB tags include `user`, `org`, `user_id`, `org_id`, `account_id`
3. Test WebSocket streaming with real data
4. Test screenshot uploads


