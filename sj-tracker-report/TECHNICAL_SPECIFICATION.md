# Technical Specification: ScreenRecord Tracker Report Backend

## Project Overview

A lightweight, scalable Go backend service that generates productivity reports from InfluxDB time monitoring data using OpenAI GPT-4.1 mini with structured JSON outputs.

## Project Structure

```
sj-tracker-report/
├── cmd/
│   └── server/
│       └── main.go                 # Application entry point
├── internal/
│   ├── api/
│   │   ├── handlers.go             # HTTP request handlers
│   │   ├── routes.go                # Route definitions
│   │   └── middleware.go            # HTTP middleware (logging, CORS, etc.)
│   ├── config/
│   │   └── config.go               # Configuration loading from .env
│   ├── database/
│   │   ├── influxdb.go             # InfluxDB client and queries
│   │   └── mongodb.go               # MongoDB client (deferred, placeholder)
│   ├── models/
│   │   ├── report.go                # Report data structures
│   │   ├── task.go                  # Task management structures
│   │   └── request.go               # API request/response structures
│   ├── services/
│   │   ├── report_service.go        # Report generation orchestration
│   │   ├── ai_service.go            # OpenAI integration
│   │   ├── data_service.go          # Data aggregation and transformation
│   │   └── task_service.go          # Task management
│   ├── validation/
│   │   └── schema.go                # JSON schema validation
│   └── utils/
│       ├── time.go                  # Time formatting utilities
│       └── uuid.go                  # UUID generation utilities
├── schemas/
│   └── report_schema.json           # JSON schema for report validation
├── prompts/
│   └── system_prompt.txt            # OpenAI system prompt
├── .env.example                     # Environment variable template
├── .gitignore
├── go.mod
├── go.sum
└── README.md
```

## Dependencies

### Required Go Packages:
- `github.com/gin-gonic/gin` - REST API framework
- `github.com/sashabaranov/go-openai` - OpenAI Go client
- `github.com/influxdata/influxdb-client-go/v2` - InfluxDB client
- `github.com/joho/godotenv` - Environment variable loading
- `github.com/google/uuid` - UUID generation
- `github.com/xeipuuv/gojsonschema` - JSON schema validation
- `go.mongodb.org/mongo-driver` - MongoDB driver (deferred, but include for future)

## Configuration

### Environment Variables (.env):
```env
# InfluxDB Configuration
INFLUXDB_URL=http://localhost:8086
INFLUXDB_TOKEN=your_token_here
INFLUXDB_ORG=your_org
INFLUXDB_BUCKET=your_bucket

# OpenAI Configuration
OPENAI_API_KEY=your_api_key_here

# Server Configuration
PORT=8080
HOST=0.0.0.0

# MongoDB Configuration (for future use)
MONGODB_URI=mongodb://localhost:27017
MONGODB_DATABASE=reports
MONGODB_COLLECTION=reports
```

## Data Structures

### Request/Response Models

**GenerateReportRequest** (`internal/models/request.go`):
```go
type GenerateReportRequest struct {
    Org       string   `json:"org" binding:"required"`
    Users     []string `json:"users" binding:"required,min=1"`
    StartDate string   `json:"startDate" binding:"required"` // YYYY-MM-DD
    EndDate   string   `json:"endDate" binding:"required"`   // YYYY-MM-DD
}
```

**TaskResponse** (`internal/models/request.go`):
```go
type TaskResponse struct {
    TaskID string `json:"taskId"`
    Status string `json:"status"` // "pending", "processing", "completed", "failed"
}
```

**StatusResponse** (`internal/models/request.go`):
```go
type StatusResponse struct {
    TaskID string      `json:"taskId"`
    Status string      `json:"status"` // "processing", "completed", "failed"
    Report interface{} `json:"report,omitempty"`
    Error  string      `json:"error,omitempty"`
}
```

### Task Management

**Task** (`internal/models/task.go`):
```go
type TaskStatus string

const (
    TaskStatusPending    TaskStatus = "pending"
    TaskStatusProcessing TaskStatus = "processing"
    TaskStatusCompleted  TaskStatus = "completed"
    TaskStatusFailed     TaskStatus = "failed"
)

type Task struct {
    ID        string     `json:"id"`
    Status    TaskStatus `json:"status"`
    Request   GenerateReportRequest `json:"request"`
    CreatedAt time.Time  `json:"createdAt"`
    UpdatedAt time.Time  `json:"updatedAt"`
    Error     string     `json:"error,omitempty"`
    Report    *Report    `json:"report,omitempty"`
}
```

### Report Models

**Report** (`internal/models/report.go`):
```go
type Report struct {
    Organizations []Organization `json:"organizations"`
    GeneratedAt   string         `json:"generatedAt"` // ISO 8601
    PeriodAnalyzed Period         `json:"periodAnalyzed"`
}

type Organization struct {
    OrganizationName string `json:"organizationName"`
    Users            []User  `json:"users"`
}

type User struct {
    UserName     string       `json:"userName"`
    OverallReport OverallReport `json:"overallReport"`
    DailyReports []DailyReport `json:"dailyReports"`
}

type OverallReport struct {
    PeriodStart              string  `json:"periodStart"`
    PeriodEnd                string  `json:"periodEnd"`
    TotalActiveHours         float64 `json:"totalActiveHours"`
    TotalActiveMinutes       float64 `json:"totalActiveMinutes"`
    TotalAfkHours            float64 `json:"totalAfkHours"`
    TotalAfkMinutes          float64 `json:"totalAfkMinutes"`
    AverageDailyActiveHours  float64 `json:"averageDailyActiveHours"`
    AverageDailyActiveMinutes float64 `json:"averageDailyActiveMinutes"`
    TotalDiscrepancies       int     `json:"totalDiscrepancies"`
    CriticalDiscrepancies    int     `json:"criticalDiscrepancies"`
    Summary                  string  `json:"summary"`
    Conclusion               string  `json:"conclusion"`
}

type DailyReport struct {
    Date                string            `json:"date"`
    HourlyBreakdown     []HourlyBreakdown `json:"hourlyBreakdown"`
    TotalActiveMinutes  float64           `json:"totalActiveMinutes"`
    TotalActiveHours    float64           `json:"totalActiveHours"`
    TotalAfkMinutes     float64           `json:"totalAfkMinutes"`
    TotalAfkHours       float64           `json:"totalAfkHours"`
    NotableDiscrepancies []Discrepancy     `json:"notableDiscrepancies"`
    Summary             string            `json:"summary"`
}

type HourlyBreakdown struct {
    Hour          int        `json:"hour"` // 0-23
    StartTime     string     `json:"startTime"` // HH:MM
    EndTime       string     `json:"endTime"`   // HH:MM
    ActiveMinutes float64    `json:"activeMinutes"`
    AfkMinutes    float64    `json:"afkMinutes"`
    AppUsage      []AppUsage `json:"appUsage"`
    TotalMinutes  int        `json:"totalMinutes"` // Always 60
}

type AppUsage struct {
    AppName      string   `json:"appName"`
    DurationMinutes float64 `json:"durationMinutes"`
    WindowTitles []string `json:"windowTitles,omitempty"`
}

type Discrepancy struct {
    Type           string  `json:"type"` // extended_afk, social_media, etc.
    Severity       string  `json:"severity"` // low, medium, high, critical
    StartTime      string  `json:"startTime"` // HH:MM
    EndTime        string  `json:"endTime"`   // HH:MM
    DurationMinutes float64 `json:"durationMinutes"`
    Description    string  `json:"description"`
    Context        string  `json:"context,omitempty"`
}

type Period struct {
    StartDate string `json:"startDate"` // YYYY-MM-DD
    EndDate   string `json:"endDate"`   // YYYY-MM-DD
}
```

### InfluxDB Data Models

**InfluxDBQueryResult** (`internal/database/influxdb.go`):
```go
type AFKStatus struct {
    Time     time.Time
    Duration int
    Status   string
    Hostname string
    Org      string
    User     string
}

type WindowActivity struct {
    Time     time.Time
    App      string
    Duration int
    Title    string
    Hostname string
    Org      string
    User     string
}

type AppUsage struct {
    Time            time.Time
    AppName         string
    DurationSeconds int
    EventCount      int
    Hostname        string
    Org             string
    User            string
}

type DailyMetrics struct {
    Time            time.Time
    Date            time.Time
    ActiveSeconds   int
    AfkSeconds      int
    AppSwitches     int
    IdleSeconds     int
    UtilizationRatio float64
    Hostname        string
    Org             string
    User            string
}
```

## API Endpoints

### 1. Generate Report
- **Method:** POST
- **Path:** `/api/reports/generate`
- **Handler:** `GenerateReportHandler` in `internal/api/handlers.go`
- **Request Body:**
```json
{
  "org": "Turbo",
  "users": ["ben"],
  "startDate": "2025-11-19",
  "endDate": "2025-11-19"
}
```
- **Response:** `TaskResponse`
```json
{
  "taskId": "uuid-here",
  "status": "pending"
}
```
- **Logic:**
  1. Validate request
  2. Generate cache key: `org:user:startDate:endDate`
  3. Check MongoDB cache (deferred - skip for now)
  4. Generate UUID for task
  5. Create task in-memory with status "pending"
  6. Start async goroutine to process task
  7. Return TaskResponse immediately

### 2. Get Task Status
- **Method:** GET
- **Path:** `/api/reports/status/:taskId`
- **Handler:** `GetTaskStatusHandler` in `internal/api/handlers.go`
- **Response:**
  - If processing: `StatusResponse` with status "processing"
  - If completed: `StatusResponse` with status "completed" and full Report
  - If failed: `StatusResponse` with status "failed" and error message
- **Logic:**
  1. Extract taskId from URL parameter
  2. Check in-memory task store
  3. If not found in-memory, check MongoDB (deferred)
  4. Return appropriate status response

## Core Services

### Task Service (`internal/services/task_service.go`)

**Functions:**
- `NewTaskService() *TaskService` - Initialize task service with in-memory map
- `CreateTask(request GenerateReportRequest) (*Task, error)` - Create new task
- `GetTask(taskID string) (*Task, error)` - Retrieve task by ID
- `UpdateTaskStatus(taskID string, status TaskStatus) error` - Update task status
- `SetTaskError(taskID string, err error)` - Mark task as failed
- `SetTaskReport(taskID string, report *Report)` - Store completed report
- `DeleteTask(taskID string)` - Remove task from memory (after completion)

**In-Memory Storage:**
```go
type TaskService struct {
    tasks map[string]*Task
    mutex sync.RWMutex
}
```

### Data Service (`internal/services/data_service.go`)

**Functions:**
- `NewDataService(influxClient *influxdb2.Client) *DataService` - Initialize
- `QueryAFKStatus(org, user string, startDate, endDate time.Time) ([]AFKStatus, error)` - Query afk_status measurement
- `QueryWindowActivity(org, user string, startDate, endDate time.Time) ([]WindowActivity, error)` - Query window_activity measurement
- `QueryAppUsage(org, user string, startDate, endDate time.Time) ([]AppUsage, error)` - Query app_usage measurement
- `QueryDailyMetrics(org, user string, startDate, endDate time.Time) ([]DailyMetrics, error)` - Query daily_metrics measurement
- `AggregateDataForAI(afkData []AFKStatus, windowData []WindowActivity, appData []AppUsage, metrics []DailyMetrics) (string, error)` - Transform data into AI-friendly format

**Flux Query Templates:**
- Use InfluxDB Flux query language
- Filter by org, user tags
- Filter by time range
- Return structured results

### AI Service (`internal/services/ai_service.go`)

**Functions:**
- `NewAIService(apiKey string) *AIService` - Initialize OpenAI client
- `GenerateReport(dataContext string, request GenerateReportRequest) (*Report, error)` - Generate report using OpenAI
- `buildSystemPrompt() string` - Load system prompt from file
- `buildUserPrompt(dataContext string, request GenerateReportRequest) string` - Build user prompt with data
- `validateReportJSON(jsonStr string) (*Report, error)` - Validate against JSON schema

**OpenAI Configuration:**
- Model: "gpt-4o-mini" (or "gpt-4.1-mini" if available)
- Temperature: 0.3 (for deterministic output)
- MaxTokens: 4000
- ResponseFormat: JSON schema mode
- Use structured outputs with JSON schema

### Report Service (`internal/services/report_service.go`)

**Functions:**
- `NewReportService(dataService *DataService, aiService *AIService) *ReportService` - Initialize
- `GenerateReport(request GenerateReportRequest) (*Report, error)` - Main orchestration function
  - Query InfluxDB for all data
  - Aggregate and transform data
  - Call AI service to generate report
  - Validate report structure
  - Return validated report

## InfluxDB Integration

### Client Setup (`internal/database/influxdb.go`)

**Functions:**
- `NewInfluxDBClient(url, token, org string) (*influxdb2.Client, error)` - Create client
- `CloseInfluxDBClient(client *influxdb2.Client)` - Close connection

### Query Functions

**QueryAFKStatus:**
```flux
from(bucket: "bucket")
  |> range(start: startDate, stop: endDate)
  |> filter(fn: (r) => r._measurement == "afk_status")
  |> filter(fn: (r) => r.org == org and r.user == user)
  |> pivot(rowKey:["_time"], columnKey: ["_field"], valueColumn: "_value")
```

**QueryWindowActivity:**
```flux
from(bucket: "bucket")
  |> range(start: startDate, stop: endDate)
  |> filter(fn: (r) => r._measurement == "window_activity")
  |> filter(fn: (r) => r.org == org and r.user == user)
  |> pivot(rowKey:["_time"], columnKey: ["_field"], valueColumn: "_value")
```

**QueryAppUsage:**
```flux
from(bucket: "bucket")
  |> range(start: startDate, stop: endDate)
  |> filter(fn: (r) => r._measurement == "app_usage")
  |> filter(fn: (r) => r.org == org and r.user == user)
  |> pivot(rowKey:["_time"], columnKey: ["_field"], valueColumn: "_value")
```

**QueryDailyMetrics:**
```flux
from(bucket: "bucket")
  |> range(start: startDate, stop: endDate)
  |> filter(fn: (r) => r._measurement == "daily_metrics")
  |> filter(fn: (r) => r.org == org and r.user == user)
  |> pivot(rowKey:["_time"], columnKey: ["_field"], valueColumn: "_value")
```

## JSON Schema Validation

### Schema File (`schemas/report_schema.json`)

Create comprehensive JSON schema matching the Report structure:
- Organizations array
- Users array with overallReport and dailyReports
- Hourly breakdowns (0-23 hours)
- All required fields with types
- String format validations (dates, times)

### Validation Function (`internal/validation/schema.go`)

**Functions:**
- `LoadSchema(schemaPath string) (*gojsonschema.Schema, error)` - Load JSON schema
- `ValidateReport(reportJSON string, schema *gojsonschema.Schema) error` - Validate report against schema
- `ValidateAndParseReport(reportJSON string) (*Report, error)` - Validate and unmarshal

## Utility Functions

### Time Utilities (`internal/utils/time.go`)

**Functions:**
- `FormatTime(t time.Time) string` - Format as HH:MM (24-hour)
- `FormatDate(t time.Time) string` - Format as YYYY-MM-DD
- `SecondsToMinutes(seconds int) float64` - Convert and round to 2 decimals
- `MinutesToHours(minutes float64) float64` - Convert and round to 2 decimals
- `ParseDate(dateStr string) (time.Time, error)` - Parse YYYY-MM-DD
- `GenerateHourRange(hour int) (startTime, endTime string)` - Generate HH:MM for hour

### UUID Utilities (`internal/utils/uuid.go`)

**Functions:**
- `GenerateUUID() string` - Generate UUID v4 string

## Configuration Management

### Config (`internal/config/config.go`)

**Structure:**
```go
type Config struct {
    InfluxDB InfluxDBConfig
    OpenAI   OpenAIConfig
    Server   ServerConfig
    MongoDB  MongoDBConfig // For future use
}

type InfluxDBConfig struct {
    URL    string
    Token  string
    Org    string
    Bucket string
}

type OpenAIConfig struct {
    APIKey     string
    Model      string
    Temperature float64
    MaxTokens  int
}

type ServerConfig struct {
    Port string
    Host string
}

type MongoDBConfig struct {
    URI        string
    Database   string
    Collection string
}
```

**Functions:**
- `LoadConfig() (*Config, error)` - Load from .env file
- `ValidateConfig(config *Config) error` - Validate required fields

## Main Application

### Entry Point (`cmd/server/main.go`)

**Flow:**
1. Load configuration from .env
2. Initialize InfluxDB client
3. Initialize services (DataService, AIService, ReportService, TaskService)
4. Initialize Gin router
5. Register routes
6. Start HTTP server

## Error Handling

### Error Types
- `ErrInvalidRequest` - Invalid API request
- `ErrTaskNotFound` - Task ID not found
- `ErrInfluxDBQuery` - InfluxDB query failure
- `ErrAIGeneration` - OpenAI API failure
- `ErrValidation` - JSON schema validation failure

### Error Response Format
```json
{
  "error": "error message",
  "code": "ERROR_CODE"
}
```

## Logging

Use standard Go `log` package or structured logging:
- Log all API requests
- Log task creation and completion
- Log InfluxDB queries
- Log OpenAI API calls
- Log errors with context

## Implementation Checklist

1. **Project Setup**
   - [ ] Initialize Go module (`go mod init`)
   - [ ] Create project directory structure
   - [ ] Create `.env.example` file
   - [ ] Create `.gitignore` file
   - [ ] Install all required dependencies

2. **Configuration**
   - [ ] Implement `internal/config/config.go` with .env loading
   - [ ] Create `.env.example` with all required variables
   - [ ] Add config validation

3. **Data Models**
   - [ ] Implement `internal/models/request.go` (request/response structs)
   - [ ] Implement `internal/models/task.go` (task management)
   - [ ] Implement `internal/models/report.go` (report structures)

4. **InfluxDB Integration**
   - [ ] Implement `internal/database/influxdb.go` client setup
   - [ ] Implement Flux query functions for all 4 measurements
   - [ ] Implement data parsing from InfluxDB results
   - [ ] Test InfluxDB connection and queries

5. **Data Service**
   - [ ] Implement `internal/services/data_service.go`
   - [ ] Implement query functions for each measurement
   - [ ] Implement data aggregation function
   - [ ] Test data retrieval and transformation

6. **JSON Schema**
   - [ ] Create `schemas/report_schema.json` matching Report structure
   - [ ] Implement `internal/validation/schema.go` validation functions
   - [ ] Test schema validation

7. **AI Service**
   - [ ] Implement `internal/services/ai_service.go`
   - [ ] Create `prompts/system_prompt.txt` with full prompt
   - [ ] Implement OpenAI client integration
   - [ ] Implement structured outputs with JSON schema
   - [ ] Implement report generation function
   - [ ] Test OpenAI API integration

8. **Report Service**
   - [ ] Implement `internal/services/report_service.go`
   - [ ] Implement orchestration logic
   - [ ] Test end-to-end report generation

9. **Task Service**
   - [ ] Implement `internal/services/task_service.go` with in-memory storage
   - [ ] Implement all task management functions
   - [ ] Test task lifecycle

10. **Utilities**
    - [ ] Implement `internal/utils/time.go` time formatting functions
    - [ ] Implement `internal/utils/uuid.go` UUID generation
    - [ ] Test utility functions

11. **API Handlers**
    - [ ] Implement `internal/api/handlers.go` with both endpoints
    - [ ] Implement request validation
    - [ ] Implement error handling
    - [ ] Test handlers

12. **API Routes**
    - [ ] Implement `internal/api/routes.go` route registration
    - [ ] Implement `internal/api/middleware.go` (CORS, logging)
    - [ ] Test API endpoints

13. **Main Application**
    - [ ] Implement `cmd/server/main.go` application bootstrap
    - [ ] Wire all services together
    - [ ] Test server startup

14. **Integration Testing**
    - [ ] Test report generation flow end-to-end
    - [ ] Test task status polling
    - [ ] Test error scenarios
    - [ ] Test concurrent requests

15. **Documentation**
    - [ ] Create README.md with setup instructions
    - [ ] Document API endpoints
    - [ ] Document environment variables

