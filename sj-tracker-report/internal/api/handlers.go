package api

import (
	"fmt"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"sj-tracker-report/internal/database"
	"sj-tracker-report/internal/models"
	"sj-tracker-report/internal/services"
	"sj-tracker-report/internal/utils"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/gin-gonic/gin"
)

// Handlers contains all HTTP handlers
type Handlers struct {
	reportService     *services.ReportService
	taskService       *services.TaskService
	weeklyEmailService *services.WeeklyEmailService
	mongoClient       *database.MongoDBClient
	chatTools         *services.ChatTools
	influxClient      *database.InfluxDBClient
}

// NewHandlers creates a new handlers instance
func NewHandlers(
	reportService *services.ReportService,
	taskService *services.TaskService,
	weeklyEmailService *services.WeeklyEmailService,
	mongoClient *database.MongoDBClient,
	chatTools *services.ChatTools,
	influxClient *database.InfluxDBClient,
) *Handlers {
	return &Handlers{
		reportService:      reportService,
		taskService:        taskService,
		weeklyEmailService: weeklyEmailService,
		mongoClient:        mongoClient,
		chatTools:          chatTools,
		influxClient:       influxClient,
	}
}

// GenerateReportHandler handles POST /api/reports/generate
func (h *Handlers) GenerateReportHandler(c *gin.Context) {
	var req models.GenerateReportRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	// Validate that accountId, orgId are provided (allow 0 for local version)
	// Note: Gin's binding:"required" treats 0 as empty for integers, so we validate manually
	if req.AccountID < 0 {
		c.JSON(http.StatusBadRequest, gin.H{"error": "accountId must be a non-negative number"})
		return
	}
	if req.OrgID < 0 {
		c.JSON(http.StatusBadRequest, gin.H{"error": "orgId must be a non-negative number"})
		return
	}
	// Validate users have valid IDs (allow 0 for local version)
	for i, user := range req.Users {
		if user.ID < 0 {
			c.JSON(http.StatusBadRequest, gin.H{"error": fmt.Sprintf("users[%d].id must be a non-negative number", i)})
			return
		}
	}

	// Check MongoDB cache - if report is cached, create a task with it immediately
	cachedReport, err := h.reportService.GetCachedReport(req)
	if err == nil && cachedReport != nil {
		// Create task and mark it as completed with cached report
		task, err := h.taskService.CreateTask(req)
		if err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to create task"})
			return
		}

		// Set the cached report immediately
		_ = h.taskService.SetTaskReport(task.ID, cachedReport)

		// Return task ID
		c.JSON(http.StatusOK, models.TaskResponse{
			TaskID: task.ID,
			Status: string(models.TaskStatusCompleted),
		})
		return
	}

	// No existing task or cache - create new task and start async generation
	task, err := h.taskService.CreateTask(req)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to create task"})
		return
	}

	// Start async report generation
	go func() {
		// Update status to processing
		_ = h.taskService.UpdateTaskStatus(task.ID, models.TaskStatusProcessing)

		// Generate report (this will check cache internally and cache the result)
		report, err := h.reportService.GenerateReport(req)
		if err != nil {
			_ = h.taskService.SetTaskError(task.ID, err)
			return
		}

		// Store report in task
		_ = h.taskService.SetTaskReport(task.ID, report)
	}()

	// Return task ID immediately
	c.JSON(http.StatusAccepted, models.TaskResponse{
		TaskID: task.ID,
		Status: string(task.Status),
	})
}

// GenerateReportSyncHandler handles POST /api/reports/generate-sync
// Synchronously generates and returns the report (waits for completion)
func (h *Handlers) GenerateReportSyncHandler(c *gin.Context) {
	var req models.GenerateReportRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	// Generate report synchronously (this will check cache internally)
	report, err := h.reportService.GenerateReport(req)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	// Return the complete report directly
	c.JSON(http.StatusOK, report)
}

// GetTaskStatusHandler handles GET /api/reports/status/:taskId
func (h *Handlers) GetTaskStatusHandler(c *gin.Context) {
	taskID := c.Param("taskId")
	if taskID == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "taskId is required"})
		return
	}

	task, err := h.taskService.GetTask(taskID)
	if err != nil {
		c.JSON(http.StatusNotFound, gin.H{"error": "task not found"})
		return
	}

	response := models.StatusResponse{
		TaskID: task.ID,
		Status: string(task.Status),
	}

	if task.Status == models.TaskStatusCompleted {
		response.Report = task.Report
	} else if task.Status == models.TaskStatusFailed {
		response.Error = task.Error
	}

	c.JSON(http.StatusOK, response)
}

// GenerateWeeklyReportHandler handles POST /api/reports/generate-weekly
func (h *Handlers) GenerateWeeklyReportHandler(c *gin.Context) {
	var req models.GenerateWeeklyReportRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	// Validate that accountId, orgId are provided (allow 0 for local version)
	// Note: Gin's binding:"required" treats 0 as empty for integers, so we validate manually
	if req.AccountID < 0 {
		c.JSON(http.StatusBadRequest, gin.H{"error": "accountId must be a non-negative number"})
		return
	}
	if req.OrgID < 0 {
		c.JSON(http.StatusBadRequest, gin.H{"error": "orgId must be a non-negative number"})
		return
	}
	// Validate users have valid IDs (allow 0 for local version)
	for i, user := range req.Users {
		if user.ID < 0 {
			c.JSON(http.StatusBadRequest, gin.H{"error": fmt.Sprintf("users[%d].id must be a non-negative number", i)})
			return
		}
	}

	// Calculate week range for cache key
	weekStartDate, err := utils.ParseDate(req.WeekStartDate)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid week start date"})
		return
	}
	monday, sunday := utils.CalculateWeekRange(weekStartDate)
	startDateStr := utils.FormatDate(monday)
	endDateStr := utils.FormatDate(sunday)

	// Check MongoDB cache for weekly reports - if report is cached, create a task with it immediately
	cachedReport, err := h.reportService.GetCachedWeeklyReport(req)
	if err == nil && cachedReport != nil {
		// Create task and mark it as completed with cached report
		// Create a temporary GenerateReportRequest for task creation
		tempRequest := models.GenerateReportRequest{
			AccountID: req.AccountID,
			Users:     req.Users,
			Org:       req.Org,
			OrgID:     req.OrgID,
			StartDate: startDateStr,
			EndDate:   endDateStr,
		}
		task, err := h.taskService.CreateTask(tempRequest)
		if err != nil {
			c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to create task"})
			return
		}

		// Set the cached report immediately
		_ = h.taskService.SetTaskReport(task.ID, cachedReport)

		// Return task ID
		c.JSON(http.StatusOK, models.TaskResponse{
			TaskID: task.ID,
			Status: string(models.TaskStatusCompleted),
		})
		return
	}

	// No existing task or cache - create new task and start async generation
	// Create a temporary GenerateReportRequest for task creation
	tempRequest := models.GenerateReportRequest{
		AccountID: req.AccountID,
		Users:     req.Users,
		Org:       req.Org,
		OrgID:     req.OrgID,
		StartDate: startDateStr,
		EndDate:   endDateStr,
	}
	task, err := h.taskService.CreateTask(tempRequest)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to create task"})
		return
	}

	// Start async weekly report generation
	go func() {
		// Update status to processing
		_ = h.taskService.UpdateTaskStatus(task.ID, models.TaskStatusProcessing)

		// Generate weekly report (this will check cache internally and cache the result)
		report, err := h.reportService.GenerateWeeklyReport(req)
		if err != nil {
			_ = h.taskService.SetTaskError(task.ID, err)
			return
		}

		// Store report in task
		_ = h.taskService.SetTaskReport(task.ID, report)
	}()

	// Return task ID immediately
	c.JSON(http.StatusAccepted, models.TaskResponse{
		TaskID: task.ID,
		Status: string(task.Status),
	})
}

// GenerateWeeklyReportSyncHandler handles POST /api/reports/generate-weekly-sync
// Synchronously generates and returns the weekly report (waits for completion)
func (h *Handlers) GenerateWeeklyReportSyncHandler(c *gin.Context) {
	var req models.GenerateWeeklyReportRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	// Generate weekly report synchronously (this will check cache internally)
	report, err := h.reportService.GenerateWeeklyReportSync(req)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": err.Error()})
		return
	}

	// Return the complete report directly
	c.JSON(http.StatusOK, report)
}

// OptInWeeklyReportsHandler handles POST /api/reports/weekly/opt-in
func (h *Handlers) OptInWeeklyReportsHandler(c *gin.Context) {
	var req models.OptInWeeklyReportsRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	if h.mongoClient == nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "MongoDB client not available"})
		return
	}

	if h.weeklyEmailService == nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "Weekly email service not available"})
		return
	}

	// Parse optional trigger time
	var nextTriggerTime *time.Time
	if req.NextTriggerTime != nil && *req.NextTriggerTime != "" {
		parsedTime, err := time.Parse(time.RFC3339, *req.NextTriggerTime)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": fmt.Sprintf("invalid nextTriggerTime format: %v. Use ISO 8601 format (e.g., 2025-01-15T14:30:00Z)", err)})
			return
		}
		nextTriggerTime = &parsedTime
	}

	// Add to MongoDB
	err := h.mongoClient.AddOptedAccount(req.AccountID, req.OrgID, req.OrgName, req.Email, req.Users, nextTriggerTime)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": fmt.Sprintf("failed to opt in: %v", err)})
		return
	}

	// Schedule the cron job
	_, err = h.weeklyEmailService.ScheduleWeeklyReport(req.AccountID, req.OrgID, req.OrgName, req.Email, nextTriggerTime)
	if err != nil {
		// Log error but don't fail - the account is still opted in
		fmt.Printf("WARNING: Failed to schedule weekly report for account %d, org %d: %v\n", req.AccountID, req.OrgID, err)
		c.JSON(http.StatusOK, gin.H{
			"message": "Opted in successfully, but failed to schedule cron job",
			"error":   err.Error(),
		})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"message":   "Successfully opted in to weekly reports",
		"accountId": req.AccountID,
		"orgId":     req.OrgID,
	})
}

// OptOutWeeklyReportsHandler handles POST /api/reports/weekly/opt-out
func (h *Handlers) OptOutWeeklyReportsHandler(c *gin.Context) {
	var req models.OptOutWeeklyReportsRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	if h.mongoClient == nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "MongoDB client not available"})
		return
	}

	// Remove from MongoDB
	err := h.mongoClient.RemoveOptedAccount(req.AccountID, req.OrgID)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": fmt.Sprintf("failed to opt out: %v", err)})
		return
	}

	// Note: We can't easily unschedule a specific cron without storing entry IDs
	// For now, we'll just remove from MongoDB. On next server restart, it won't be scheduled.
	// TODO: Store cron entry IDs in MongoDB for proper unscheduling

	c.JSON(http.StatusOK, gin.H{
		"message":   "Successfully opted out of weekly reports",
		"accountId": req.AccountID,
		"orgId":     req.OrgID,
	})
}

// SendWeeklyReportEmailHandler handles POST /api/reports/weekly/send-email
// Manually sends a weekly report email for a specific week (no cron scheduling)
func (h *Handlers) SendWeeklyReportEmailHandler(c *gin.Context) {
	var req models.SendWeeklyReportEmailRequest
	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	if h.weeklyEmailService == nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "Weekly email service not available"})
		return
	}

	// Send the email asynchronously
	go func() {
		err := h.weeklyEmailService.SendWeeklyReportEmailForWeek(
			req.AccountID,
			req.OrgID,
			req.OrgName,
			req.Email,
			req.Users,
			req.WeekStartDate,
		)
		if err != nil {
			fmt.Printf("ERROR: Failed to send weekly report email: %v\n", err)
		}
	}()

	c.JSON(http.StatusAccepted, gin.H{
		"message":   "Weekly report email generation and sending initiated",
		"accountId": req.AccountID,
		"orgId":     req.OrgID,
		"week":      req.WeekStartDate,
	})
}

// GetOptedInAccountsHandler handles GET /api/reports/weekly/opted-in/:accountId
// Returns all organizations that have opted into weekly reports for the given account
func (h *Handlers) GetOptedInAccountsHandler(c *gin.Context) {
	accountIDStr := c.Param("accountId")
	accountID, err := strconv.Atoi(accountIDStr)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid accountId"})
		return
	}

	if h.mongoClient == nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "MongoDB client not available"})
		return
	}

	accounts, err := h.mongoClient.GetOptedAccountsByAccountID(accountID)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": fmt.Sprintf("failed to get opted accounts: %v", err)})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"accountId": accountID,
		"accounts":  accounts,
	})
}

// ListToolsHandler handles GET /api/chat/tools
// Returns list of all available tools with their schemas
func (h *Handlers) ListToolsHandler(c *gin.Context) {
	if h.chatTools == nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "chat tools not initialized"})
		return
	}

	tools := h.chatTools.GetAllTools()
	toolSchemas := make([]map[string]interface{}, len(tools))

	for i, tool := range tools {
		toolSchemas[i] = map[string]interface{}{
			"name":        tool.Name,
			"description": tool.Description,
			"parameters":  tool.Parameters,
		}
	}

	c.JSON(http.StatusOK, gin.H{
		"tools": toolSchemas,
	})
}

// ExecuteToolHandler handles POST /api/chat/tools/execute
// Executes a tool with given parameters
func (h *Handlers) ExecuteToolHandler(c *gin.Context) {
	if h.chatTools == nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "chat tools not initialized"})
		return
	}

	var req struct {
		ToolName string                 `json:"tool_name" binding:"required"`
		Params   map[string]interface{} `json:"params" binding:"required"`
	}

	if err := c.ShouldBindJSON(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	// Find the tool
	tools := h.chatTools.GetAllTools()
	var tool *services.Tool
	for i := range tools {
		if tools[i].Name == req.ToolName {
			tool = &tools[i]
			break
		}
	}

	if tool == nil {
		c.JSON(http.StatusNotFound, gin.H{"error": fmt.Sprintf("tool '%s' not found", req.ToolName)})
		return
	}

	// Execute the tool
	result, err := tool.Execute(req.Params)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{
			"error":   "tool execution failed",
			"message": err.Error(),
		})
		return
	}

	c.JSON(http.StatusOK, gin.H{
		"result": result,
	})
}

// GetTimelineHandler handles GET /api/timeline
// Returns screen timeline data for a user on a specific date
// No authentication required for open-source local version
func (h *Handlers) GetTimelineHandler(c *gin.Context) {
	var req models.TimelineRequest
	if err := c.ShouldBindQuery(&req); err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": err.Error()})
		return
	}

	// Validate that userId is provided (allow 0 for local version)
	// Note: Gin's binding:"required" treats 0 as empty for integers, so we validate manually
	// Check if userId was actually provided in query string
	userIdStr := c.Query("userId")
	if userIdStr == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "userId is required"})
		return
	}
	if req.UserID < 0 {
		c.JSON(http.StatusBadRequest, gin.H{"error": "userId must be a non-negative number"})
		return
	}

	// Validate date is provided
	if req.Date == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "date is required (YYYY-MM-DD format)"})
		return
	}

	if h.influxClient == nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "InfluxDB client not available"})
		return
	}

	// Parse date
	date, err := utils.ParseDate(req.Date)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid date format, expected YYYY-MM-DD"})
		return
	}

	// Set time range for the specific date (start of day to end of day)
	startDate := time.Date(date.Year(), date.Month(), date.Day(), 0, 0, 0, 0, date.Location())
	endDate := time.Date(date.Year(), date.Month(), date.Day(), 23, 59, 59, 999999999, date.Location())

	// Use orgID = 0 for local version (allow 0 for open-source)
	orgID := 0

	// Query screen timeline data
	timelineData, err := h.influxClient.QueryScreenTimeline(req.AccountID, orgID, req.UserID, startDate, endDate)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": fmt.Sprintf("failed to query timeline: %v", err)})
		return
	}

	// Convert to response format
	events := make([]models.TimelineEvent, 0, len(timelineData))
	for _, timeline := range timelineData {
		events = append(events, models.TimelineEvent{
			Time:            timeline.Time.Format(time.RFC3339),
			App:             timeline.App,
			AppTitle:        timeline.AppTitle,
			Description:     timeline.Description,
			ProductiveScore: timeline.ProductiveScore,
			DurationSeconds: timeline.DurationSeconds,
		})
	}

	response := models.TimelineResponse{
		UserID:    req.UserID,
		AccountID: req.AccountID,
		Date:      req.Date,
		Events:    events,
	}

	c.JSON(http.StatusOK, response)
}

// GetAudioTranscriptsHandler handles GET /api/audio-transcripts
// Returns all audio transcripts for a specific user, grouped by audio URL and sorted by time
func (h *Handlers) GetAudioTranscriptsHandler(c *gin.Context) {
	// Manually extract query parameters (like timeline handler does)
	// This allows userId=0 and accountId=0 for local version
	userIdStr := c.Query("userId")
	if userIdStr == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "userId is required"})
		return
	}
	
	accountIdStr := c.Query("accountId")
	if accountIdStr == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "accountId is required"})
		return
	}
	
	userId, err := strconv.Atoi(userIdStr)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "userId must be a valid number"})
		return
	}
	
	accountId, err := strconv.Atoi(accountIdStr)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "accountId must be a valid number"})
		return
	}
	
	// Validate that userId and accountId are non-negative (allow 0 for local version)
	if userId < 0 {
		c.JSON(http.StatusBadRequest, gin.H{"error": "userId must be a non-negative number"})
		return
	}
	if accountId < 0 {
		c.JSON(http.StatusBadRequest, gin.H{"error": "accountId must be a non-negative number"})
		return
	}
	
	// Optional parameters
	orgId := 0
	orgIdStr := c.Query("orgId")
	if orgIdStr != "" {
		orgId, err = strconv.Atoi(orgIdStr)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "orgId must be a valid number"})
			return
		}
	}
	
	date := c.Query("date")

	if h.influxClient == nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "InfluxDB client not available"})
		return
	}

	// Parse date if provided
	var startDate, endDate *time.Time
	if date != "" {
		parsedDate, err := utils.ParseDate(date)
		if err != nil {
			c.JSON(http.StatusBadRequest, gin.H{"error": "invalid date format, expected YYYY-MM-DD"})
			return
		}
		// Set time range for the specific date (start of day to end of day)
		start := time.Date(parsedDate.Year(), parsedDate.Month(), parsedDate.Day(), 0, 0, 0, 0, parsedDate.Location())
		end := time.Date(parsedDate.Year(), parsedDate.Month(), parsedDate.Day(), 23, 59, 59, 999999999, parsedDate.Location())
		startDate = &start
		endDate = &end
	}

	// Query audio transcripts from InfluxDB
	transcripts, err := h.influxClient.QueryAudioTranscript(accountId, orgId, userId, startDate, endDate)
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": fmt.Sprintf("failed to query audio transcripts: %v", err)})
		return
	}

	// Convert to response format and group by audio_url
	transcriptRecords := make([]models.AudioTranscriptRecord, 0, len(transcripts))
	for _, transcript := range transcripts {
		transcriptRecords = append(transcriptRecords, models.AudioTranscriptRecord{
			Time:      transcript.Time.Format(time.RFC3339),
			AccountID: transcript.AccountID,
			OrgID:     transcript.OrgID,
			UserID:    transcript.UserID,
			Org:       transcript.Org,
			User:      transcript.User,
			Hostname:  transcript.Hostname,
			Fields:    transcript.Fields,
		})
	}

	// Group transcripts by audio_path (the field name used in InfluxDB) and convert local paths to serving URLs
	groupsByURL := make(map[string][]models.AudioTranscriptRecord)
	for _, record := range transcriptRecords {
		audioURL := ""
		// Check for audio_path field (the actual field name in InfluxDB)
		// Also check audio_url for backwards compatibility
		var audioPathStr string
		var found bool
		
		if pathVal, ok := record.Fields["audio_path"]; ok {
			if pathStr, ok := pathVal.(string); ok && pathStr != "" {
				audioPathStr = pathStr
				found = true
			}
		} else if urlVal, ok := record.Fields["audio_url"]; ok {
			// Fallback to audio_url for backwards compatibility
			if urlStr, ok := urlVal.(string); ok && urlStr != "" {
				audioPathStr = urlStr
				found = true
			}
		}
		
		if found {
			// Convert local file path to a URL that can be served by the backend
			// If it's already a URL (http/https), use it as-is
			// Otherwise, convert local path to /api/audio-file?path=...
			if strings.HasPrefix(audioPathStr, "http://") || strings.HasPrefix(audioPathStr, "https://") {
				audioURL = audioPathStr
			} else if audioPathStr != "" {
				// It's a local file path - convert to serving URL
				// URL encode the path parameter
				encodedPath := url.QueryEscape(audioPathStr)
				audioURL = fmt.Sprintf("/api/audio-file?path=%s", encodedPath)
			}
		}
		// Use empty string as key if no audio_path/audio_url found
		groupsByURL[audioURL] = append(groupsByURL[audioURL], record)
	}

	// Sort transcripts within each group by time, then create groups
	transcriptGroups := make([]models.AudioTranscriptGroup, 0, len(groupsByURL))
	for audioURL, records := range groupsByURL {
		// Sort records by time (ascending)
		sort.Slice(records, func(i, j int) bool {
			timeI, errI := time.Parse(time.RFC3339, records[i].Time)
			timeJ, errJ := time.Parse(time.RFC3339, records[j].Time)
			if errI != nil || errJ != nil {
				return records[i].Time < records[j].Time // Fallback to string comparison
			}
			return timeI.Before(timeJ)
		})

		transcriptGroups = append(transcriptGroups, models.AudioTranscriptGroup{
			AudioURL:    audioURL,
			Transcripts: records,
		})
	}

	// Sort groups by the first transcript's time (earliest first)
	sort.Slice(transcriptGroups, func(i, j int) bool {
		if len(transcriptGroups[i].Transcripts) == 0 || len(transcriptGroups[j].Transcripts) == 0 {
			return false
		}
		timeI, errI := time.Parse(time.RFC3339, transcriptGroups[i].Transcripts[0].Time)
		timeJ, errJ := time.Parse(time.RFC3339, transcriptGroups[j].Transcripts[0].Time)
		if errI != nil || errJ != nil {
			return transcriptGroups[i].Transcripts[0].Time < transcriptGroups[j].Transcripts[0].Time
		}
		return timeI.Before(timeJ)
	})

	response := models.AudioTranscriptResponse{
		UserID:     userId,
		AccountID:  accountId,
		OrgID:      orgId,
		Transcripts: transcriptGroups,
	}

	c.JSON(http.StatusOK, response)
}

// ServeAudioFileHandler handles GET /api/audio-file
// Serves local audio files from the filesystem
// Security: Validates that the file path is within the expected audio directory
func (h *Handlers) ServeAudioFileHandler(c *gin.Context) {
	// Get the file path from query parameter
	filePath := c.Query("path")
	if filePath == "" {
		c.JSON(http.StatusBadRequest, gin.H{"error": "path parameter is required"})
		return
	}

	// Validate and resolve the file path
	// Audio files can be stored in multiple locations:
	// - ~/.screenjournal/audio/ (legacy/default)
	// - ~/Library/Application Support/com.screenjournal.tracker/audio/ (macOS Tauri app)
	// - %APPDATA%/com.screenjournal.tracker/audio/ (Windows Tauri app)
	// We need to ensure the path is within one of these directories to prevent directory traversal attacks
	
	// Clean the provided path
	cleanedPath := filepath.Clean(filePath)
	
	// Must be an absolute path
	if !filepath.IsAbs(cleanedPath) {
		c.JSON(http.StatusBadRequest, gin.H{"error": "path must be absolute"})
		return
	}
	
	// Resolve any symlinks and get the absolute path
	absPath, err := filepath.Abs(cleanedPath)
	if err != nil {
		c.JSON(http.StatusBadRequest, gin.H{"error": "invalid file path"})
		return
	}
	
	// Security check: ensure path doesn't contain ".." after cleaning
	if strings.Contains(absPath, "..") {
		c.JSON(http.StatusForbidden, gin.H{"error": "access denied: invalid path"})
		return
	}
	
	// Get the user's home directory
	homeDir, err := os.UserHomeDir()
	if err != nil {
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to get home directory"})
		return
	}
	
	// List of allowed base directories for audio files
	allowedBaseDirs := []string{
		filepath.Join(homeDir, ".screenjournal", "audio"),                    // Legacy/default
		filepath.Join(homeDir, "Library", "Application Support", "com.screenjournal.tracker", "audio"), // macOS Tauri
	}
	
	// On Windows, also check AppData
	if runtime.GOOS == "windows" {
		appData := os.Getenv("APPDATA")
		if appData != "" {
			allowedBaseDirs = append(allowedBaseDirs, filepath.Join(appData, "com.screenjournal.tracker", "audio"))
		}
	}
	
	// Check if the file path is within any of the allowed base directories
	pathAllowed := false
	for _, baseDir := range allowedBaseDirs {
		baseAbs, err := filepath.Abs(baseDir)
		if err != nil {
			continue // Skip invalid base dir
		}
		
		// Check if the path is within this base directory
		relPath, err := filepath.Rel(baseAbs, absPath)
		if err == nil && !strings.HasPrefix(relPath, "..") {
			pathAllowed = true
			break
		}
	}
	
	if !pathAllowed {
		c.JSON(http.StatusForbidden, gin.H{"error": "access denied: file path outside allowed directories"})
		return
	}

	// Check if file exists
	fileInfo, err := os.Stat(absPath)
	if err != nil {
		if os.IsNotExist(err) {
			c.JSON(http.StatusNotFound, gin.H{"error": "audio file not found"})
			return
		}
		c.JSON(http.StatusInternalServerError, gin.H{"error": "failed to access file"})
		return
	}

	// Ensure it's a file, not a directory
	if fileInfo.IsDir() {
		c.JSON(http.StatusBadRequest, gin.H{"error": "path points to a directory, not a file"})
		return
	}

	// Set appropriate headers for audio file
	c.Header("Content-Type", "audio/mp4")
	c.Header("Content-Length", fmt.Sprintf("%d", fileInfo.Size()))
	c.Header("Accept-Ranges", "bytes")

	// Serve the file
	c.File(absPath)
}
