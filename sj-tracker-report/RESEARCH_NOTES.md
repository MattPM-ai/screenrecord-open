# Research Notes - ScreenRecord Tracker Report Backend

## Database Schema

### Data Source: InfluxDB (Time-Series Database)

**Note:** PostgreSQL CREATE TABLE statements provided were for reference only. Actual data is stored in InfluxDB.

### Measurements (Tables) Identified:
1. **app_usage** - Application-level usage tracking
   - Fields: app_name, duration_seconds, event_count
   - Tags: hostname, org, user
   - Timestamp: time
   
2. **afk_status** - Away-from-keyboard status
   - Fields: duration, status (AFK/active)
   - Tags: hostname, org, user
   - Timestamp: time
   
3. **window_activity** - Window-level activity tracking
   - Fields: app, duration, title
   - Tags: hostname, org, user
   - Timestamp: time
   
4. **daily_metrics** - Pre-aggregated daily metrics
   - Fields: active_seconds, afk_seconds, app_switches, idle_seconds, utilization_ratio
   - Tags: hostname, org, user
   - Timestamp: time (or date field)

### InfluxDB Structure:
- **Measurements:** Equivalent to tables (app_usage, afk_status, window_activity, daily_metrics)
- **Tags:** Indexed metadata (hostname, org, user) - used for filtering
- **Fields:** Actual data values (duration, app_name, etc.) - not indexed
- **Timestamp:** Time-series data point timestamp
- **Query Language:** Flux (InfluxDB's query language)

### Observations:
- Multi-tenant structure: hostname, org, user as tags for efficient filtering
- Time-based data optimized for time-series queries
- Duration metrics in seconds
- daily_metrics provides pre-aggregated summaries
- InfluxDB optimized for time-range queries and aggregations

## Report Output Structure

### Hierarchical Organization:
```
organizations[]
  └── users[]
      ├── overallReport
      │   ├── periodStart/End
      │   ├── totalActiveHours/Minutes
      │   ├── totalAfkHours/Minutes
      │   ├── averageDailyActiveHours/Minutes
      │   ├── totalDiscrepancies
      │   ├── criticalDiscrepancies
      │   ├── summary (AI-generated)
      │   └── conclusion (AI-generated)
      └── dailyReports[]
          ├── date
          ├── hourlyBreakdown[]
          │   ├── hour, startTime, endTime
          │   ├── activeMinutes, afkMinutes
          │   └── appUsage[]
          │       ├── appName
          │       ├── durationMinutes
          │       └── windowTitles[]
          ├── totalActiveMinutes/Hours
          ├── totalAfkMinutes/Hours
          ├── notableDiscrepancies[]
          │   ├── type (extended_afk, social_media, etc.)
          │   ├── severity (high, medium, low)
          │   ├── startTime, endTime
          │   ├── durationMinutes
          │   ├── description (AI-generated)
          │   └── context (AI-generated)
          └── summary (AI-generated)
```

### Key Features:
- Multi-organization, multi-user structure
- Time period analysis (start/end dates)
- Hourly granularity with app usage breakdown
- Window title tracking per app
- Discrepancy detection and flagging
- AI-generated summaries and conclusions
- Metrics in both hours and minutes

### Discrepancy Types Observed:
- `extended_afk` - Long AFK periods during work hours
- `social_media` - Non-work app usage during work hours

### Severity Levels:
- `high` - Critical issues
- `medium` - Notable issues
- `low` - Minor issues (inferred)

## Technical Requirements (From Initial Request)

1. **Backend Requirements:**
   - **CONFIRMED:** Go language
   - Lightweight but highly scalable
   - Generate reports from time monitoring data
   - Use OpenAI GPT-4.1 mini (Go client, not Vercel AI SDK)
   - Use Go schema validation library (not Zod)

2. **Data Sources:**
   - **PRIMARY:** InfluxDB (time-series database for raw activity data)
   - **CACHE:** MongoDB (stores previously generated reports)
   - Note: PostgreSQL CREATE TABLE statements were just for setup reference, not actual database

3. **API Architecture:**
   - **CONFIRMED:** Lightweight REST API (Go)
   - **REQUIREMENT:** Highly scalable
   - **PATTERN:** Async task-based with UUID tracking
   - **AUTH:** Deferred (to be implemented later)

## API Design Pattern

### Async Task-Based Architecture:

**Endpoint 1: Generate Report (POST)**
- Request: `POST /api/reports/generate`
- Parameters: `org`, `user`, `startDate`, `endDate` (or user array for org-level)
- Behavior:
  - If report already exists (same params), return existing UUID immediately
  - If not cached, create async task, return UUID immediately
- Response: `{ "taskId": "uuid", "status": "pending" | "completed" }`

**Endpoint 2: Check Task Status (GET)**
- Request: `GET /api/reports/status/{uuid}`
- Behavior:
  - If task still generating: `{ "status": "processing", "taskId": "uuid" }`
  - If task completed: Return full report JSON
  - If task failed: `{ "status": "failed", "error": "..." }`
- Response: Report JSON or status object

### Cache Key Strategy:
- Generate cache key from: `org + user + startDate + endDate`
- Store in MongoDB with UUID as document ID
- Query MongoDB before generating new report

## Go Technology Stack Research

### Required Libraries:

1. **REST API Framework:**
   - **Gin** (gin-gonic.com) - Lightweight, high-performance
   - Alternative: `net/http` (standard library), `gorilla/mux`

2. **OpenAI Integration:**
   - **openai-go** (github.com/sashabaranov/go-openai) - Popular Go client
   - **openai-go** (github.com/openai/openai-go) - Official OpenAI Go library
   - Note: Need to implement structured output (JSON schema mode) manually
   - Vercel AI SDK's `generateObject` equivalent: Use OpenAI's structured outputs feature with JSON schema

3. **JSON Schema Validation:**
   - **gojsonschema** (github.com/xeipuuv/gojsonschema) - JSON Schema Draft 4
   - **jsonschema** (github.com/google/jsonschema-go) - Google's implementation
   - Alternative: Manual struct validation with `encoding/json` + custom validation

4. **InfluxDB Client:**
   - **influxdb-client-go** (github.com/influxdata/influxdb-client-go/v2) - Official InfluxDB Go client
   - Uses Flux query language for time-series queries

5. **MongoDB Client:**
   - **mongo-driver** (go.mongodb.org/mongo-driver) - Official MongoDB Go driver

6. **Task Management:**
   - In-memory task queue or simple goroutine-based async processing
   - Store task status in MongoDB or in-memory map
   - Consider: Redis for distributed task queue (if scaling horizontally)

## Configuration & Deployment

### Environment Variables (.env):
- **InfluxDB:** Connection details via .env (URL, token, bucket, org)
- **OpenAI:** API key via .env
- **MongoDB:** Connection details via .env (to be configured later)

### InfluxDB Measurements:
- `afk_status`
- `app_usage`
- `daily_metrics`
- `window_activity`

### Task Management Strategy:
- **Live Tasks:** In-memory map/struct for active tasks
- **Completed Tasks:** Stored in MongoDB as part of the report document
- **Task Lifecycle:**
  1. Task created → stored in-memory with UUID
  2. Task processing → status tracked in-memory
  3. Task completed → report stored in MongoDB with UUID as document ID
  4. Task removed from in-memory store (can be retrieved from MongoDB)

### Deployment:
- **Environment:** Self-hosted on user's server (dev purposes)
- **Configuration:** Environment variables (.env file)
- **MongoDB:** To be set up later (can defer implementation)

## Implementation Priorities

### Phase 1 (Core Functionality):
1. InfluxDB connection and querying
2. Data aggregation and transformation
3. OpenAI integration with structured outputs
4. JSON schema validation
5. In-memory task management
6. REST API endpoints (generate, status)

### Phase 2 (Caching - Deferred):
1. MongoDB connection
2. Report caching logic
3. Cache lookup before generation

## Remaining Implementation Decisions

1. **Task Timeout/Retry:**
   - Task timeout duration?
   - Retry logic for failed OpenAI calls?
   - Concurrent task limits?

2. **Error Handling:**
   - Error response format?
   - Logging strategy?
   - Task failure handling?

3. **OpenAI Configuration:**
   - Temperature settings for deterministic output?
   - Max tokens for report generation?
   - Retry logic for API failures?

## AI Integration Requirements

### Model Configuration:
- **Model:** GPT-4.1 mini
- **SDK:** Go OpenAI client (openai-go library)
- **Validation:** Go JSON schema validation library (gojsonschema or google/jsonschema)
- **Structured Output:** OpenAI's JSON schema mode (manual implementation, no Vercel AI SDK)

### Prompt Requirements (From Prototype):

**System Role:**
- Specialized AI for analyzing work-related activity data
- Generates structured, deterministic activity reports in JSON format

**Key Processing Requirements:**

1. **Data Analysis:**
   - Scrupulous detail in analysis
   - Cross-reference multiple data sources
   - Pattern recognition across days/hours
   - Context awareness (time of day, working hours)
   - Precise calculations with double-checking

2. **Hour-by-Hour Breakdown:**
   - Process ALL data for each day
   - Group by hour (0-23) for each day
   - Calculate active minutes (non-AFK) and AFK minutes per hour
   - Convert seconds to minutes (divide by 60, round to 2 decimals)
   - Include all apps used, sorted by duration (longest first)
   - Include window titles for context
   - Every hour (0-23) must be represented, even if 0 activity
   - Format times as HH:MM (24-hour format)
   - Flag hours where active + AFK ≠ 60 minutes (±1 min margin)

3. **Discrepancy Detection Types:**
   - `EXTENDED_AFK`: >30 min during core hours (9 AM-5 PM), >60 min outside
   - `SOCIAL_MEDIA`: Facebook, Twitter/X, Instagram, TikTok, LinkedIn (personal), Reddit, Discord, Slack (non-work)
   - `MEDIA_CONSUMPTION`: YouTube, Netflix, Spotify, gaming, streaming
   - `EXCESSIVE_IDLE`: Prolonged inactivity from idle_seconds
   - `LOW_PRODUCTIVITY_APPS`: Clearly non-work applications
   - `SUSPICIOUS_PATTERN`: Unusual timing, minimal work patterns

4. **Severity Levels:**
   - `LOW`: Minor distractions, brief social media (< 15 min)
   - `MEDIUM`: Regular non-work activity (15-60 min), extended breaks
   - `HIGH`: Significant non-work time (1-2 hours), frequent distractions
   - `CRITICAL`: Majority unproductive, minimal work, time theft patterns

5. **Time Formatting:**
   - ALL durations in BOTH minutes AND hours
   - Seconds → minutes: divide by 60, round to 2 decimals
   - Minutes → hours: divide by 60, round to 2 decimals
   - Time strings: HH:MM format (24-hour)
   - Dates: YYYY-MM-DD format

6. **Daily Report Requirements:**
   - Complete hourly breakdown (all 24 hours)
   - Daily totals (active/AFK minutes and hours)
   - All notable discrepancies for the day
   - Brief summary (2-3 sentences)

7. **Overall Report Requirements:**
   - Total active hours/minutes (exclude AFK)
   - Total AFK hours/minutes (exclude active)
   - Average daily active hours/minutes (sum of active / number of days)
   - Discrepancy summary (total count, critical count)
   - Summary (3-5 sentences): patterns, trends, observations
   - **Conclusion (CRITICAL):**
     - MUST be critical if working time insufficient
     - Typical expectation: 6-8 hours active time per day
     - If average daily active hours < 6, conclusion MUST be critical
     - Clear assessment of productivity adequacy

8. **Response Format:**
   - MUST be valid JSON only (no markdown, no code fences, no commentary)
   - Strictly conform to provided JSON structure
   - Every hour (0-23) represented for each day
   - Every day in period must have daily report

### Backend Implementation Considerations (Go):

**Data Flow:**
1. API receives report generation request (org, user, date range)
2. Check MongoDB cache for existing report (cache key: org+user+dates)
3. If cached: Return UUID immediately
4. If not cached:
   a. Create async task with UUID
   b. Query InfluxDB for raw data (afk_status, window_activity, app_usage, daily_metrics)
   c. Aggregate/transform data for AI processing
   d. Send structured data + prompt to OpenAI via Go client
   e. OpenAI generates report using JSON schema mode
   f. Validate response against JSON schema using Go validation library
   g. Store validated report in MongoDB cache
   h. Mark task as completed
5. Client polls status endpoint to retrieve report when ready

**Key Implementation Points:**
- Go backend handles all data aggregation (InfluxDB queries)
- Use Go JSON schema validation library (not Zod)
- Implement OpenAI structured outputs with JSON schema mode
- Ensure deterministic output through proper prompt engineering + schema validation
- Handle all 24 hours per day requirement in data processing
- Implement discrepancy detection logic (or rely on AI with proper context)
- Convert all time units (seconds → minutes → hours) in Go
- Format all dates/times according to specifications
- Async task management with goroutines or task queue
- MongoDB caching with UUID-based document storage

