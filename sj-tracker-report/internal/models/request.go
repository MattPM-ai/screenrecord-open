package models

// UserRequest represents a user in the report generation request
type UserRequest struct {
	Name string `json:"name" binding:"required"`
	ID   int    `json:"id"` // Allow 0 for local version
}

// GenerateReportRequest represents the request to generate a report
type GenerateReportRequest struct {
	AccountID    int           `json:"accountId"` // Allow 0 for local version
	Users        []UserRequest `json:"users" binding:"required,min=1"`
	Org          string        `json:"org" binding:"required"`
	OrgID        int           `json:"orgId"` // Allow 0 for local version
	StartDate    string        `json:"startDate" binding:"required"` // YYYY-MM-DD
	EndDate      string        `json:"endDate" binding:"required"`   // YYYY-MM-DD
	GeminiAPIKey string        `json:"geminiApiKey" binding:"required"` // User's Gemini API key
}

// TaskResponse represents the response when creating a task
type TaskResponse struct {
	TaskID string `json:"taskId"`
	Status string `json:"status"` // "pending", "processing", "completed", "failed"
}

// GenerateWeeklyReportRequest represents the request to generate a weekly report
type GenerateWeeklyReportRequest struct {
	AccountID    int           `json:"accountId"` // Allow 0 for local version
	Users        []UserRequest `json:"users" binding:"required,min=1"`
	Org          string        `json:"org" binding:"required"`
	OrgID        int           `json:"orgId"` // Allow 0 for local version
	WeekStartDate string       `json:"weekStartDate" binding:"required"` // YYYY-MM-DD - Monday of the week (or start date if custom period)
	GeminiAPIKey  string       `json:"geminiApiKey" binding:"required"` // User's Gemini API key
	// Optional: Custom start/end dates for exact period (overrides Monday-Sunday calculation)
	CustomStartDate *string `json:"customStartDate,omitempty"` // ISO 8601 datetime (e.g., "2025-12-01T16:30:00Z")
	CustomEndDate   *string `json:"customEndDate,omitempty"`   // ISO 8601 datetime (e.g., "2025-12-08T16:30:00Z")
}

// StatusResponse represents the response when checking task status
type StatusResponse struct {
	TaskID string      `json:"taskId"`
	Status string      `json:"status"` // "processing", "completed", "failed"
	Report interface{} `json:"report,omitempty"`
	Error  string      `json:"error,omitempty"`
}

// OptInWeeklyReportsRequest represents the request to opt into weekly email reports
type OptInWeeklyReportsRequest struct {
	AccountID      int           `json:"accountId"` // Allow 0 for local version
	OrgID          int           `json:"orgId"` // Allow 0 for local version
	OrgName        string        `json:"orgName" binding:"required"`
	Email          string        `json:"email" binding:"required,email"`
	Users          []UserRequest `json:"users" binding:"required,min=1"`
	NextTriggerTime *string      `json:"nextTriggerTime,omitempty"` // Optional ISO 8601 datetime override for testing (e.g., "2025-01-15T14:30:00Z")
}

// OptOutWeeklyReportsRequest represents the request to opt out of weekly email reports
type OptOutWeeklyReportsRequest struct {
	AccountID int `json:"accountId"` // Allow 0 for local version
	OrgID     int `json:"orgId"` // Allow 0 for local version
}

// SendWeeklyReportEmailRequest represents the request to manually send a weekly report email
type SendWeeklyReportEmailRequest struct {
	AccountID    int           `json:"accountId"` // Allow 0 for local version
	OrgID        int           `json:"orgId"` // Allow 0 for local version
	OrgName      string        `json:"orgName" binding:"required"`
	Email        string        `json:"email" binding:"required,email"`
	Users        []UserRequest `json:"users" binding:"required,min=1"`
	WeekStartDate string       `json:"weekStartDate" binding:"required"` // YYYY-MM-DD - Monday of the week
}

// TimelineRequest represents the request to get timeline data
type TimelineRequest struct {
	UserID    int    `form:"userId"` // Allow 0 for local version
	AccountID int    `form:"accountId"` // Allow 0 for local version
	Date      string `form:"date" binding:"required"` // YYYY-MM-DD
}

// TimelineResponse represents the timeline response
type TimelineResponse struct {
	UserID    int                `json:"userId"`
	AccountID int                `json:"accountId"`
	Date      string             `json:"date"` // YYYY-MM-DD
	Events    []TimelineEvent    `json:"events"`
}


// AudioTranscriptRequest represents the request to get audio transcripts
type AudioTranscriptRequest struct {
	UserID    int    `form:"userId" binding:"required"`
	AccountID int    `form:"accountId" binding:"required"`
	OrgID     int    `form:"orgId"`     // Optional
	Date      string `form:"date"`      // Optional: YYYY-MM-DD format to filter by specific date
}

// AudioTranscriptResponse represents the audio transcript response
type AudioTranscriptResponse struct {
	UserID     int                           `json:"userId"`
	AccountID  int                           `json:"accountId"`
	OrgID      int                           `json:"orgId,omitempty"`
	Transcripts []AudioTranscriptGroup       `json:"transcripts"`
}

// AudioTranscriptGroup represents a group of transcripts with the same audio URL
type AudioTranscriptGroup struct {
	AudioURL   string                 `json:"audioUrl"`
	Transcripts []AudioTranscriptRecord `json:"transcripts"` // Sorted by time
}

// AudioTranscriptRecord represents a single audio transcript record
type AudioTranscriptRecord struct {
	Time      string                 `json:"time"`      // ISO 8601 timestamp
	AccountID int                    `json:"accountId"`
	OrgID     int                    `json:"orgId,omitempty"`
	UserID    int                    `json:"userId"`
	Org       string                 `json:"org,omitempty"`
	User      string                 `json:"user,omitempty"`
	Hostname  string                 `json:"hostname,omitempty"`
	Fields    map[string]interface{} `json:"fields"` // All other fields from the table
}
