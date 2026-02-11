package database

import (
	"context"
	"fmt"
	"io"
	"log"
	"net/http"
	"strconv"
	"strings"
	"time"
)

// InfluxDBClient wraps HTTP client for InfluxDB 2.0 queries
type InfluxDBClient struct {
	httpClient *http.Client
	url        string
	token      string
	org        string
	bucket     string
}

// AFKStatus represents AFK status data from InfluxDB
type AFKStatus struct {
	Time     time.Time
	Duration int
	Status   string
	Hostname string
	Org      string
	User     string
}

// WindowActivity represents window activity data from InfluxDB
type WindowActivity struct {
	Time     time.Time
	App      string
	Duration int
	Title    string
	Hostname string
	Org      string
	User     string
}

// AppUsageData represents app usage data from InfluxDB
type AppUsageData struct {
	Time            time.Time
	AppName         string
	DurationSeconds int
	EventCount      int
	Hostname        string
	Org             string
	User            string
}

// DailyMetrics represents daily metrics data from InfluxDB
type DailyMetrics struct {
	Time             time.Time
	Date             time.Time
	ActiveSeconds    int
	AfkSeconds       int
	AppSwitches      int
	IdleSeconds      int
	UtilizationRatio float64
	Hostname         string
	Org              string
	User             string
}

// ScreenTimeline represents screen timeline data from InfluxDB
type ScreenTimeline struct {
	Time             time.Time
	Display          int
	App              string
	Hostname         string
	Description      string
	ProductiveScore  int
	AppTitle         string
	DurationSeconds  int
	AccountID        int
	Org              string
	OrgID            int
	User             string
	UserID           int
	SegmentID        string
	TimeOffset       string // Format: "00:15", "00:20" (MM:SS)
}

// AudioTranscript represents audio transcript data from InfluxDB
// This structure holds all fields from the audio_transcript measurement
type AudioTranscript struct {
	Time      time.Time
	AccountID int
	OrgID     int
	UserID    int
	Org       string
	User      string
	Hostname  string
	Fields    map[string]interface{} // All other fields (text, speaker, duration_ms, audio_url, etc.)
}

// QueryResponse represents the response from InfluxDB 2.0 Flux query
type QueryResponse struct {
	Results []struct {
		StatementID int `json:"statement_id"`
		Series      []struct {
			Name    string                   `json:"name"`
			Columns []string                 `json:"columns"`
			Values  [][]interface{}          `json:"values"`
		} `json:"series"`
	} `json:"results"`
}

// NewInfluxDBClient creates a new InfluxDB 2.0 HTTP client
func NewInfluxDBClient(url, token, org, bucket string) (*InfluxDBClient, error) {
	// Ensure URL has proper format
	hostURL := strings.TrimSpace(url)
	if hostURL == "" {
		return nil, fmt.Errorf("InfluxDB URL is required")
	}

	// Remove trailing slash if present
	hostURL = strings.TrimSuffix(hostURL, "/")

	// Validate token
	if token == "" {
		return nil, fmt.Errorf("InfluxDB token is required")
	}

	// Validate org
	if org == "" {
		return nil, fmt.Errorf("InfluxDB org is required")
	}

	// Validate bucket
	if bucket == "" {
		return nil, fmt.Errorf("InfluxDB bucket is required")
	}

	return &InfluxDBClient{
		httpClient: &http.Client{
			Timeout: 30 * time.Second,
		},
		url:    hostURL,
		token:  token,
		org:    org,
		bucket: bucket,
	}, nil
}

// parseTime attempts to parse a time value from various formats
func parseTime(value interface{}) (time.Time, error) {
	// Try string formats first
	if t, ok := value.(string); ok {
		formats := []string{
			time.RFC3339,
			time.RFC3339Nano,
			"2006-01-02 15:04:05",
			"2006-01-02T15:04:05Z",
			"2006-01-02T15:04:05",
			"2006-01-02T15:04:05.000Z",
			"2006-01-02T15:04:05.000000Z",
		}
		
		for _, format := range formats {
			if parsedTime, err := time.Parse(format, t); err == nil {
				return parsedTime, nil
			}
		}
		
		return time.Time{}, fmt.Errorf("could not parse time string: %s", t)
	}
	
	// Try numeric formats (Unix timestamps)
	if t, ok := value.(float64); ok {
		// Assume seconds if < year 2100, otherwise nanoseconds
		if t < 4102444800 { // Jan 1, 2100 in seconds
			return time.Unix(int64(t), 0), nil
		}
		return time.Unix(0, int64(t)), nil
	}
	
	if t, ok := value.(int64); ok {
		// Assume seconds if < year 2100, otherwise nanoseconds
		if t < 4102444800 {
			return time.Unix(t, 0), nil
		}
		return time.Unix(0, t), nil
	}
	
	if t, ok := value.(int); ok {
		return time.Unix(int64(t), 0), nil
	}
	
	return time.Time{}, fmt.Errorf("time value is not a recognized type: %T", value)
}

// Close closes the InfluxDB client connection (no-op for HTTP client)
func (c *InfluxDBClient) Close() {
	// HTTP client doesn't need explicit closing
}

// QueryFluxRaw executes a raw Flux query and returns the raw results as maps
// This is useful for arbitrary queries that don't fit the structured query methods
func (c *InfluxDBClient) QueryFluxRaw(ctx context.Context, fluxQuery string) ([]map[string]interface{}, error) {
	return c.query(ctx, fluxQuery)
}

// query executes a Flux query against InfluxDB 2.0 via HTTP
func (c *InfluxDBClient) query(ctx context.Context, fluxQuery string) ([]map[string]interface{}, error) {
	// InfluxDB 2.0 Flux query endpoint
	queryURL := fmt.Sprintf("%s/api/v2/query?org=%s", c.url, c.org)

	// Create HTTP request with Flux query as body
	req, err := http.NewRequestWithContext(ctx, "POST", queryURL, strings.NewReader(fluxQuery))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}

	// Set headers for InfluxDB 2.0
	req.Header.Set("Content-Type", "application/vnd.flux")
	req.Header.Set("Authorization", fmt.Sprintf("Token %s", c.token))
	// InfluxDB 2.0 returns CSV by default, which we'll parse

	// Execute request
	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("failed to execute request: %w", err)
	}
	defer resp.Body.Close()

	// Read response body
	bodyBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	// Check status code
	if resp.StatusCode != http.StatusOK {
		// Log configuration details for debugging (without exposing full token)
		tokenPreview := ""
		if len(c.token) > 0 {
			if len(c.token) > 8 {
				tokenPreview = c.token[:4] + "..." + c.token[len(c.token)-4:]
			} else {
				tokenPreview = "***"
			}
		}
		log.Printf("InfluxDB query failed - URL: %s, Org: %s, Token: %s (preview)", c.url, c.org, tokenPreview)
		return nil, fmt.Errorf("InfluxDB query failed with status %d: %s", resp.StatusCode, string(bodyBytes))
	}

	// InfluxDB 2.0 returns CSV format by default
	return c.parseCSVResponse(bodyBytes)
}

// parseCSVResponse parses CSV response from InfluxDB 2.0
// InfluxDB 2.0 returns annotated CSV with format:
// #group,false,false,true,true,false,false,true,true,true
// #datatype,string,long,dateTime:RFC3339,dateTime:RFC3339,dateTime:RFC3339,double,string,string,string
// #default,_result,,,,,,,,
// ,result,table,_start,_stop,_time,_value,account_id,org_id,user_id
func (c *InfluxDBClient) parseCSVResponse(bodyBytes []byte) ([]map[string]interface{}, error) {
	lines := strings.Split(string(bodyBytes), "\n")
	if len(lines) < 4 {
		return []map[string]interface{}{}, nil
	}

	// Find the header line (starts with comma after annotations)
	var headerLineIndex int
	var headers []string
	for i, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		// First non-annotation, non-empty line should be the header
		headers = strings.Split(line, ",")
		headerLineIndex = i
		break
	}

	if len(headers) == 0 {
		return []map[string]interface{}{}, nil
	}

	// Clean headers
	for i, h := range headers {
		headers[i] = strings.TrimSpace(h)
	}

	// Parse data rows (after header)
	var results []map[string]interface{}
	for i := headerLineIndex + 1; i < len(lines); i++ {
		line := strings.TrimSpace(lines[i])
		if line == "" {
			continue
		}
		
		// Parse CSV line (handle quoted values)
		values := c.parseCSVLine(line)
		if len(values) == 0 {
			continue
		}

		row := make(map[string]interface{})
		for j, header := range headers {
			if j < len(values) {
				val := strings.TrimSpace(values[j])
				if val == "" {
					continue
				}
				// Try to parse as number if possible
				if intVal, err := strconv.Atoi(val); err == nil {
					row[header] = intVal
				} else if floatVal, err := strconv.ParseFloat(val, 64); err == nil {
					row[header] = floatVal
				} else {
					row[header] = val
				}
			}
		}
		if len(row) > 0 {
			results = append(results, row)
		}
	}

	return results, nil
}

// parseCSVLine parses a CSV line, handling quoted values
func (c *InfluxDBClient) parseCSVLine(line string) []string {
	var values []string
	var current strings.Builder
	inQuotes := false

	for i, char := range line {
		switch char {
		case '"':
			if inQuotes && i+1 < len(line) && line[i+1] == '"' {
				// Escaped quote
				current.WriteRune('"')
				i++ // Skip next quote
			} else {
				// Toggle quote state
				inQuotes = !inQuotes
			}
		case ',':
			if inQuotes {
				current.WriteRune(char)
			} else {
				values = append(values, current.String())
				current.Reset()
			}
		default:
			current.WriteRune(char)
		}
	}
	// Add last value
	values = append(values, current.String())

	return values
}

// QueryAFKStatus queries the afk_status measurement
func (c *InfluxDBClient) QueryAFKStatus(accountID, orgID, userID int, startDate, endDate time.Time) ([]AFKStatus, error) {
	// Flux query for InfluxDB 2.0
	query := fmt.Sprintf(`from(bucket: "%s")
  |> range(start: %s, stop: %s)
  |> filter(fn: (r) => r["_measurement"] == "afk_status")
  |> filter(fn: (r) => r["account_id"] == "%d")
  |> filter(fn: (r) => r["org_id"] == "%d")
  |> filter(fn: (r) => r["user_id"] == "%d")
  |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
  |> sort(columns: ["_time"])`,
		c.bucket,
		startDate.Format(time.RFC3339),
		endDate.Format(time.RFC3339),
		accountID,
		orgID,
		userID,
	)

	rows, err := c.query(context.Background(), query)
	if err != nil {
		return nil, fmt.Errorf("failed to query afk_status: %w", err)
	}

	var records []AFKStatus
	for _, row := range rows {
		afkStatus := AFKStatus{}

		// Parse time using helper function (Flux uses _time)
		if timeVal, exists := row["_time"]; exists {
			if parsedTime, err := parseTime(timeVal); err == nil {
				afkStatus.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in AFK status: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else if timeVal, exists := row["time"]; exists {
			// Fallback to "time" field
			if parsedTime, err := parseTime(timeVal); err == nil {
				afkStatus.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in AFK status: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else {
			log.Printf("WARNING: Time field not found in AFK status row")
		}

		// Parse duration
		if duration, ok := row["duration"].(float64); ok {
			afkStatus.Duration = int(duration)
		} else if duration, ok := row["duration"].(int64); ok {
			afkStatus.Duration = int(duration)
		} else if duration, ok := row["duration"].(int); ok {
			afkStatus.Duration = duration
		}

		// Parse string fields
		afkStatus.Status = getStringValue(row, "status")
		afkStatus.Hostname = getStringValue(row, "hostname")
		afkStatus.Org = getStringValue(row, "org")
		afkStatus.User = getStringValue(row, "user")

		records = append(records, afkStatus)
	}

	return records, nil
}

// QueryWindowActivity queries the window_activity measurement
func (c *InfluxDBClient) QueryWindowActivity(accountID, orgID, userID int, startDate, endDate time.Time) ([]WindowActivity, error) {
	// Flux query for InfluxDB 2.0
	query := fmt.Sprintf(`from(bucket: "%s")
  |> range(start: %s, stop: %s)
  |> filter(fn: (r) => r["_measurement"] == "window_activity")
  |> filter(fn: (r) => r["account_id"] == "%d")
  |> filter(fn: (r) => r["org_id"] == "%d")
  |> filter(fn: (r) => r["user_id"] == "%d")
  |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
  |> sort(columns: ["_time"])`,
		c.bucket,
		startDate.Format(time.RFC3339),
		endDate.Format(time.RFC3339),
		accountID,
		orgID,
		userID,
	)

	rows, err := c.query(context.Background(), query)
	if err != nil {
		return nil, fmt.Errorf("failed to query window_activity: %w", err)
	}

	var records []WindowActivity
	for _, row := range rows {
		windowActivity := WindowActivity{}

		// Parse time using helper function (Flux uses _time)
		if timeVal, exists := row["_time"]; exists {
			if parsedTime, err := parseTime(timeVal); err == nil {
				windowActivity.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in window activity: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else if timeVal, exists := row["time"]; exists {
			// Fallback to "time" field
			if parsedTime, err := parseTime(timeVal); err == nil {
				windowActivity.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in window activity: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else {
			log.Printf("WARNING: Time field not found in window activity row")
		}

		// Parse duration
		if duration, ok := row["duration"].(float64); ok {
			windowActivity.Duration = int(duration)
		} else if duration, ok := row["duration"].(int64); ok {
			windowActivity.Duration = int(duration)
		} else if duration, ok := row["duration"].(int); ok {
			windowActivity.Duration = duration
		}

		// Parse string fields
		windowActivity.App = getStringValue(row, "app")
		windowActivity.Title = getStringValue(row, "title")
		windowActivity.Hostname = getStringValue(row, "hostname")
		windowActivity.Org = getStringValue(row, "org")
		windowActivity.User = getStringValue(row, "user")

		records = append(records, windowActivity)
	}

	return records, nil
}

// QueryAppUsage queries the app_usage measurement
func (c *InfluxDBClient) QueryAppUsage(accountID, orgID, userID int, startDate, endDate time.Time) ([]AppUsageData, error) {
	// Flux query for InfluxDB 2.0
	query := fmt.Sprintf(`from(bucket: "%s")
  |> range(start: %s, stop: %s)
  |> filter(fn: (r) => r["_measurement"] == "app_usage")
  |> filter(fn: (r) => r["account_id"] == "%d")
  |> filter(fn: (r) => r["org_id"] == "%d")
  |> filter(fn: (r) => r["user_id"] == "%d")
  |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
  |> sort(columns: ["_time"])`,
		c.bucket,
		startDate.Format(time.RFC3339),
		endDate.Format(time.RFC3339),
		accountID,
		orgID,
		userID,
	)

	rows, err := c.query(context.Background(), query)
	if err != nil {
		return nil, fmt.Errorf("failed to query app_usage: %w", err)
	}

	var records []AppUsageData
	for _, row := range rows {
		appUsage := AppUsageData{}

		// Parse time using helper function (Flux uses _time)
		if timeVal, exists := row["_time"]; exists {
			if parsedTime, err := parseTime(timeVal); err == nil {
				appUsage.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in app usage: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else if timeVal, exists := row["time"]; exists {
			// Fallback to "time" field
			if parsedTime, err := parseTime(timeVal); err == nil {
				appUsage.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in app usage: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else {
			log.Printf("WARNING: Time field not found in app usage row")
		}

		// Parse duration_seconds
		if duration, ok := row["duration_seconds"].(float64); ok {
			appUsage.DurationSeconds = int(duration)
		} else if duration, ok := row["duration_seconds"].(int64); ok {
			appUsage.DurationSeconds = int(duration)
		} else if duration, ok := row["duration_seconds"].(int); ok {
			appUsage.DurationSeconds = duration
		}

		// Parse event_count
		if eventCount, ok := row["event_count"].(float64); ok {
			appUsage.EventCount = int(eventCount)
		} else if eventCount, ok := row["event_count"].(int64); ok {
			appUsage.EventCount = int(eventCount)
		} else if eventCount, ok := row["event_count"].(int); ok {
			appUsage.EventCount = eventCount
		}

		// Parse string fields
		appUsage.AppName = getStringValue(row, "app_name")
		appUsage.Hostname = getStringValue(row, "hostname")
		appUsage.Org = getStringValue(row, "org")
		appUsage.User = getStringValue(row, "user")

		records = append(records, appUsage)
	}

	return records, nil
}

// QueryDailyMetrics queries the daily_metrics measurement
func (c *InfluxDBClient) QueryDailyMetrics(accountID, orgID, userID int, startDate, endDate time.Time) ([]DailyMetrics, error) {
	// Flux query for InfluxDB 2.0
	query := fmt.Sprintf(`from(bucket: "%s")
  |> range(start: %s, stop: %s)
  |> filter(fn: (r) => r["_measurement"] == "daily_metrics")
  |> filter(fn: (r) => r["account_id"] == "%d")
  |> filter(fn: (r) => r["org_id"] == "%d")
  |> filter(fn: (r) => r["user_id"] == "%d")
  |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
  |> sort(columns: ["_time"])`,
		c.bucket,
		startDate.Format(time.RFC3339),
		endDate.Format(time.RFC3339),
		accountID,
		orgID,
		userID,
	)

	rows, err := c.query(context.Background(), query)
	if err != nil {
		return nil, fmt.Errorf("failed to query daily_metrics: %w", err)
	}

	var records []DailyMetrics
	for _, row := range rows {
		dailyMetrics := DailyMetrics{}

		// Parse time using helper function (Flux uses _time)
		if timeVal, exists := row["_time"]; exists {
			if parsedTime, err := parseTime(timeVal); err == nil {
				dailyMetrics.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in daily metrics: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else if timeVal, exists := row["time"]; exists {
			// Fallback to "time" field
			if parsedTime, err := parseTime(timeVal); err == nil {
				dailyMetrics.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in daily metrics: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else {
			log.Printf("WARNING: Time field not found in daily metrics row")
		}

		// Parse date
		if dateStr := getStringValue(row, "date"); dateStr != "" {
			if date, err := time.Parse("2006-01-02", dateStr); err == nil {
				dailyMetrics.Date = date
			}
		}

		// Parse numeric fields
		if val, ok := row["active_seconds"].(float64); ok {
			dailyMetrics.ActiveSeconds = int(val)
		} else if val, ok := row["active_seconds"].(int64); ok {
			dailyMetrics.ActiveSeconds = int(val)
		} else if val, ok := row["active_seconds"].(int); ok {
			dailyMetrics.ActiveSeconds = val
		}

		if val, ok := row["afk_seconds"].(float64); ok {
			dailyMetrics.AfkSeconds = int(val)
		} else if val, ok := row["afk_seconds"].(int64); ok {
			dailyMetrics.AfkSeconds = int(val)
		} else if val, ok := row["afk_seconds"].(int); ok {
			dailyMetrics.AfkSeconds = val
		}

		if val, ok := row["app_switches"].(float64); ok {
			dailyMetrics.AppSwitches = int(val)
		} else if val, ok := row["app_switches"].(int64); ok {
			dailyMetrics.AppSwitches = int(val)
		} else if val, ok := row["app_switches"].(int); ok {
			dailyMetrics.AppSwitches = val
		}

		if val, ok := row["idle_seconds"].(float64); ok {
			dailyMetrics.IdleSeconds = int(val)
		} else if val, ok := row["idle_seconds"].(int64); ok {
			dailyMetrics.IdleSeconds = int(val)
		} else if val, ok := row["idle_seconds"].(int); ok {
			dailyMetrics.IdleSeconds = val
		}

		if val, ok := row["utilization_ratio"].(float64); ok {
			dailyMetrics.UtilizationRatio = val
		} else if val, ok := row["utilization_ratio"].(float32); ok {
			dailyMetrics.UtilizationRatio = float64(val)
		}

		// Parse string fields
		dailyMetrics.Hostname = getStringValue(row, "hostname")
		dailyMetrics.Org = getStringValue(row, "org")
		dailyMetrics.User = getStringValue(row, "user")

		records = append(records, dailyMetrics)
	}

	return records, nil
}

// QueryScreenTimeline queries the screen_timeline measurement
func (c *InfluxDBClient) QueryScreenTimeline(accountID, orgID, userID int, startDate, endDate time.Time) ([]ScreenTimeline, error) {
	// Flux query for InfluxDB 2.0
	query := fmt.Sprintf(`from(bucket: "%s")
  |> range(start: %s, stop: %s)
  |> filter(fn: (r) => r["_measurement"] == "screen_timeline")
  |> filter(fn: (r) => r["account_id"] == "%d")
  |> filter(fn: (r) => r["org_id"] == "%d")
  |> filter(fn: (r) => r["user_id"] == "%d")
  |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
  |> sort(columns: ["_time"])`,
		c.bucket,
		startDate.Format(time.RFC3339),
		endDate.Format(time.RFC3339),
		accountID,
		orgID,
		userID,
	)

	rows, err := c.query(context.Background(), query)
	if err != nil {
		return nil, fmt.Errorf("failed to query screen_timeline: %w", err)
	}

	var records []ScreenTimeline
	for _, row := range rows {
		timeline := ScreenTimeline{}

		// Parse time using helper function (Flux uses _time)
		if timeVal, exists := row["_time"]; exists {
			if parsedTime, err := parseTime(timeVal); err == nil {
				timeline.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in screen timeline: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else if timeVal, exists := row["time"]; exists {
			// Fallback to "time" field
			if parsedTime, err := parseTime(timeVal); err == nil {
				timeline.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in screen timeline: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else {
			log.Printf("WARNING: Time field not found in screen timeline row")
		}

		// Parse display
		if val, ok := row["display"].(float64); ok {
			timeline.Display = int(val)
		} else if val, ok := row["display"].(int64); ok {
			timeline.Display = int(val)
		} else if val, ok := row["display"].(int); ok {
			timeline.Display = val
		}

		// Parse productive_score
		if val, ok := row["productive_score"].(float64); ok {
			timeline.ProductiveScore = int(val)
		} else if val, ok := row["productive_score"].(int64); ok {
			timeline.ProductiveScore = int(val)
		} else if val, ok := row["productive_score"].(int); ok {
			timeline.ProductiveScore = val
		}

		// Parse duration_seconds
		if val, ok := row["duration_seconds"].(float64); ok {
			timeline.DurationSeconds = int(val)
		} else if val, ok := row["duration_seconds"].(int64); ok {
			timeline.DurationSeconds = int(val)
		} else if val, ok := row["duration_seconds"].(int); ok {
			timeline.DurationSeconds = val
		}

		// Parse account_id
		if val, ok := row["account_id"].(float64); ok {
			timeline.AccountID = int(val)
		} else if val, ok := row["account_id"].(int64); ok {
			timeline.AccountID = int(val)
		} else if val, ok := row["account_id"].(int); ok {
			timeline.AccountID = val
		}

		// Parse org_id
		if val, ok := row["org_id"].(float64); ok {
			timeline.OrgID = int(val)
		} else if val, ok := row["org_id"].(int64); ok {
			timeline.OrgID = int(val)
		} else if val, ok := row["org_id"].(int); ok {
			timeline.OrgID = val
		}

		// Parse user_id
		if val, ok := row["user_id"].(float64); ok {
			timeline.UserID = int(val)
		} else if val, ok := row["user_id"].(int64); ok {
			timeline.UserID = int(val)
		} else if val, ok := row["user_id"].(int); ok {
			timeline.UserID = val
		}

		// Parse string fields
		timeline.App = getStringValue(row, "app")
		timeline.Hostname = getStringValue(row, "hostname")
		timeline.Description = getStringValue(row, "description")
		timeline.AppTitle = getStringValue(row, "app_title")
		timeline.Org = getStringValue(row, "org")
		timeline.User = getStringValue(row, "user")
		timeline.SegmentID = getStringValue(row, "segment_id")
		timeline.TimeOffset = getStringValue(row, "time_offset")

		records = append(records, timeline)
	}

	return records, nil
}

// Helper function to safely extract string values from query results
func getStringValue(row map[string]interface{}, key string) string {
	if val, ok := row[key].(string); ok {
		return val
	}
	return ""
}

// QueryAudioTranscript queries the audio_transcript measurement by user ID
// Returns all rows for the specified user with all fields preserved
// If startDate and endDate are provided, filters by that date range. Otherwise uses -30d to now()
func (c *InfluxDBClient) QueryAudioTranscript(accountID, orgID, userID int, startDate, endDate *time.Time) ([]AudioTranscript, error) {
	// Determine date range for query
	var startDateStr, endDateStr string
	if startDate != nil && endDate != nil {
		// Use provided date range
		startDateStr = startDate.Format(time.RFC3339)
		endDateStr = endDate.Format(time.RFC3339)
	} else {
		// Default: last 30 days
		startDateStr = "-30d"
		endDateStr = "now()"
	}

	// Flux query for InfluxDB 2.0
	// orgID is optional (0 means don't filter by org_id)
	query := fmt.Sprintf(`from(bucket: "%s")
  |> range(start: %s, stop: %s)
  |> filter(fn: (r) => r["_measurement"] == "audio_transcript")
  |> filter(fn: (r) => r["account_id"] == "%d")
  |> filter(fn: (r) => r["user_id"] == "%d")
  |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
  |> sort(columns: ["_time"])`,
		c.bucket,
		startDateStr,
		endDateStr,
		accountID,
		userID,
	)

	// Add org_id filter only if orgID is not 0
	if orgID != 0 {
		query = fmt.Sprintf(`from(bucket: "%s")
  |> range(start: %s, stop: %s)
  |> filter(fn: (r) => r["_measurement"] == "audio_transcript")
  |> filter(fn: (r) => r["account_id"] == "%d")
  |> filter(fn: (r) => r["org_id"] == "%d")
  |> filter(fn: (r) => r["user_id"] == "%d")
  |> pivot(rowKey: ["_time"], columnKey: ["_field"], valueColumn: "_value")
  |> sort(columns: ["_time"])`,
			c.bucket,
			startDateStr,
			endDateStr,
			accountID,
			orgID,
			userID,
		)
	}

	rows, err := c.query(context.Background(), query)
	if err != nil {
		return nil, fmt.Errorf("failed to query audio_transcript: %w", err)
	}

	var records []AudioTranscript
	for _, row := range rows {
		transcript := AudioTranscript{
			Fields: make(map[string]interface{}),
		}

		// Parse time using helper function (Flux uses _time)
		if timeVal, exists := row["_time"]; exists {
			if parsedTime, err := parseTime(timeVal); err == nil {
				transcript.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in audio transcript: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		} else if timeVal, exists := row["time"]; exists {
			// Fallback to "time" field
			if parsedTime, err := parseTime(timeVal); err == nil {
				transcript.Time = parsedTime
			} else {
				log.Printf("WARNING: Could not parse time in audio transcript: %v (type: %T), error: %v", timeVal, timeVal, err)
			}
		}

		// Parse account_id
		if val, ok := row["account_id"].(float64); ok {
			transcript.AccountID = int(val)
		} else if val, ok := row["account_id"].(int64); ok {
			transcript.AccountID = int(val)
		} else if val, ok := row["account_id"].(int); ok {
			transcript.AccountID = val
		}

		// Parse org_id
		if val, ok := row["org_id"].(float64); ok {
			transcript.OrgID = int(val)
		} else if val, ok := row["org_id"].(int64); ok {
			transcript.OrgID = int(val)
		} else if val, ok := row["org_id"].(int); ok {
			transcript.OrgID = val
		}

		// Parse user_id
		if val, ok := row["user_id"].(float64); ok {
			transcript.UserID = int(val)
		} else if val, ok := row["user_id"].(int64); ok {
			transcript.UserID = int(val)
		} else if val, ok := row["user_id"].(int); ok {
			transcript.UserID = val
		}

		// Parse string fields
		transcript.Org = getStringValue(row, "org")
		transcript.User = getStringValue(row, "user")
		transcript.Hostname = getStringValue(row, "hostname")

		// Store all other fields in the Fields map
		for key, value := range row {
			// Skip fields we've already parsed
			if key != "_time" && key != "time" && key != "account_id" && key != "org_id" && key != "user_id" && key != "org" && key != "user" && key != "hostname" {
				transcript.Fields[key] = value
			}
		}

		records = append(records, transcript)
	}

	return records, nil
}
