package services

import (
	"fmt"
	"os"
	"path/filepath"
	"sj-tracker-report/internal/database"
	"sj-tracker-report/internal/models"
	"sj-tracker-report/internal/utils"
	"sort"
	"strings"
	"time"
)

// ReportService orchestrates report generation
type ReportService struct {
	dataService *DataService
	aiService   *AIService
	mongoClient *database.MongoDBClient
}

// NewReportService creates a new report service
func NewReportService(dataService *DataService, aiService *AIService, mongoClient *database.MongoDBClient) *ReportService {
	return &ReportService{
		dataService: dataService,
		aiService:   aiService,
		mongoClient: mongoClient,
	}
}

// resolveGeminiAPIKey returns the Gemini API key to use: request key, then GEMINI_API_KEY env, then
// file at GEMINI_API_KEY_FILE env, then gemini_api_key.txt in APP_DATA_DIR (or current working directory).
// Used so the report backend can use the same key the desktop app stores when the frontend does not send one.
func resolveGeminiAPIKey(requestKey string) string {
	s := strings.TrimSpace(requestKey)
	if s != "" {
		fmt.Printf("[report] Gemini API key: using key from request body\n")
		return s
	}
	if s = strings.TrimSpace(os.Getenv("GEMINI_API_KEY")); s != "" {
		fmt.Printf("[report] Gemini API key: using key from GEMINI_API_KEY env\n")
		return s
	}
	// Prefer explicit key file path (set by start-bundled.bat on Windows)
	if keyPath := strings.TrimSpace(os.Getenv("GEMINI_API_KEY_FILE")); keyPath != "" {
		data, err := os.ReadFile(keyPath)
		if err != nil {
			fmt.Printf("[report] Gemini API key: GEMINI_API_KEY_FILE=%s read failed: %v\n", keyPath, err)
		} else {
			s = strings.TrimSpace(string(data))
			if s != "" {
				fmt.Printf("[report] Gemini API key: using key from file (GEMINI_API_KEY_FILE)\n")
				return s
			}
		}
	}
	dir := os.Getenv("APP_DATA_DIR")
	if dir == "" {
		dir, _ = os.Getwd()
	}
	if dir != "" {
		path := filepath.Join(dir, "gemini_api_key.txt")
		data, err := os.ReadFile(path)
		if err == nil {
			s = strings.TrimSpace(string(data))
			if s != "" {
				fmt.Printf("[report] Gemini API key: using key from file %s\n", path)
				return s
			}
		}
	}
	fmt.Printf("[report] Gemini API key: no key found (request empty, no env, no key file)\n")
	return ""
}

// GetCachedReport retrieves a cached report without generating a new one
func (s *ReportService) GetCachedReport(request models.GenerateReportRequest) (*models.Report, error) {
	if s.mongoClient == nil {
		return nil, fmt.Errorf("MongoDB client not available")
	}

	cacheKey := database.GenerateCacheKey(request.Org, request.OrgID, request.Users, request.StartDate, request.EndDate)
	return s.mongoClient.GetCachedReport(cacheKey)
}

// GetCachedWeeklyReport retrieves a cached weekly report without generating a new one
func (s *ReportService) GetCachedWeeklyReport(request models.GenerateWeeklyReportRequest) (*models.Report, error) {
	if s.mongoClient == nil {
		return nil, fmt.Errorf("MongoDB client not available")
	}

	// Parse week start date and calculate week range
	weekStartDate, err := utils.ParseDate(request.WeekStartDate)
	if err != nil {
		return nil, fmt.Errorf("invalid week start date: %w", err)
	}
	monday, sunday := utils.CalculateWeekRange(weekStartDate)
	startDateStr := utils.FormatDate(monday)
	endDateStr := utils.FormatDate(sunday)

	cacheKey := database.GenerateCacheKey(request.Org, request.OrgID, request.Users, startDateStr, endDateStr)
	return s.mongoClient.GetCachedWeeklyReport(cacheKey)
}

// GenerateReport generates a complete report for the given request
func (s *ReportService) GenerateReport(request models.GenerateReportRequest) (*models.Report, error) {
	if len(request.Users) == 0 {
		return nil, fmt.Errorf("at least one user is required")
	}

	// Check cache first (if MongoDB client is available)
	if s.mongoClient != nil {
		cacheKey := database.GenerateCacheKey(request.Org, request.OrgID, request.Users, request.StartDate, request.EndDate)
		cachedReport, err := s.mongoClient.GetCachedReport(cacheKey)
		if err != nil {
			// Log error but continue with generation
			fmt.Printf("WARNING: Failed to check cache: %v\n", err)
		} else if cachedReport != nil {
			fmt.Printf("Cache hit for key: %s\n", cacheKey)
			return cachedReport, nil
		}
		fmt.Printf("Cache miss for key: %s, generating new report\n", cacheKey)
	}

	// Parse dates
	startDate, err := utils.ParseDate(request.StartDate)
	if err != nil {
		return nil, fmt.Errorf("invalid start date: %w", err)
	}

	endDate, err := utils.ParseDate(request.EndDate)
	if err != nil {
		return nil, fmt.Errorf("invalid end date: %w", err)
	}

	// Add one day to endDate to include the full end date
	endDate = endDate.Add(24 * time.Hour)

	// Resolve Gemini API key (request body, then env, then app data file for desktop/bundled use)
	geminiKey := resolveGeminiAPIKey(request.GeminiAPIKey)

	// Build report for all users
	var allUsers []models.User

	for _, userReq := range request.Users {
		// Query all data from InfluxDB for this user using IDs for filtering
		afkData, windowData, appData, metricsData, timelineData, err := s.dataService.QueryAllData(
			request.AccountID,
			request.OrgID,
			userReq.ID,
			startDate,
			endDate,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to query data for user %s (ID: %d): %w", userReq.Name, userReq.ID, err)
		}

		// Log what we got
		fmt.Printf("DEBUG: User %s (ID: %d) - Retrieved data - AFK: %d records, Window: %d records, App: %d records, Metrics: %d records, Timeline: %d records\n",
			userReq.Name, userReq.ID, len(afkData), len(windowData), len(appData), len(metricsData), len(timelineData))

		// Aggregate hourly data
		hourlyData := s.dataService.AggregateHourlyData(afkData, windowData, timelineData, startDate, endDate)

		// Build user report (use user name for the report)
		userReport := s.buildUserReportFromData(hourlyData, userReq.Name, request, startDate, endDate)

		// Use AI to enhance this user's report
		rawDataContext := s.dataService.AggregateDataForAI(afkData, windowData, appData, metricsData, timelineData)
		err = s.aiService.EnhanceUserReportWithAI(geminiKey, &userReport, rawDataContext, request, userReq.Name)
		if err != nil {
			// Log error but don't fail - we still have a valid report structure
			fmt.Printf("WARNING: Failed to enhance report with AI for user %s (ID: %d): %v\n", userReq.Name, userReq.ID, err)
		}

		allUsers = append(allUsers, userReport)
	}

	// Build final report with all users
	org := models.Organization{
		OrganizationName: request.Org,
		Users:            allUsers,
	}

	// Calculate user rankings if there are 2+ users
	if len(allUsers) >= 2 {
		userRanking := s.calculateUserRankings(allUsers)
		org.UserRanking = userRanking

		// Enhance rankings with AI-generated insights
		err = s.aiService.EnhanceRankingsWithAI(geminiKey, userRanking, allUsers, request)
		if err != nil {
			// Log error but don't fail - rankings are still valid
			fmt.Printf("WARNING: Failed to enhance rankings with AI: %v\n", err)
		}
	}

	report := &models.Report{
		Organizations: []models.Organization{org},
		GeneratedAt: time.Now().UTC().Format(time.RFC3339),
		PeriodAnalyzed: models.Period{
			StartDate: request.StartDate,
			EndDate:   request.EndDate,
		},
	}

	// Cache the generated report (if MongoDB client is available)
	if s.mongoClient != nil {
		err = s.mongoClient.CacheReport(request.Org, request.OrgID, request.Users, request.StartDate, request.EndDate, report)
		if err != nil {
			// Log error but don't fail - the report is still valid
			fmt.Printf("WARNING: Failed to cache report: %v\n", err)
		} else {
			fmt.Printf("Report cached successfully\n")
		}
	}

	return report, nil
}

// GenerateWeeklyReport generates a weekly report for the organization (aggregated across all users)
func (s *ReportService) GenerateWeeklyReport(request models.GenerateWeeklyReportRequest) (*models.Report, error) {
	if len(request.Users) == 0 {
		return nil, fmt.Errorf("at least one user is required")
	}

	// Determine start and end dates
	var startDate, endDate time.Time
	var startDateStr, endDateStr string
	var err error

	if request.CustomStartDate != nil && request.CustomEndDate != nil {
		// Use custom exact dates if provided
		startDate, err = time.Parse(time.RFC3339, *request.CustomStartDate)
		if err != nil {
			return nil, fmt.Errorf("invalid custom start date format: %w", err)
		}
		endDate, err = time.Parse(time.RFC3339, *request.CustomEndDate)
		if err != nil {
			return nil, fmt.Errorf("invalid custom end date format: %w", err)
		}
		// Format dates for display (date only for start, include time context)
		startDateStr = startDate.Format("2006-01-02")
		endDateStr = endDate.Format("2006-01-02")
		fmt.Printf("Using custom date range: %s to %s\n", *request.CustomStartDate, *request.CustomEndDate)
	} else {
		// Default: Calculate week range (Monday to Sunday)
		weekStartDate, err := utils.ParseDate(request.WeekStartDate)
		if err != nil {
			return nil, fmt.Errorf("invalid week start date: %w", err)
		}
		monday, sunday := utils.CalculateWeekRange(weekStartDate)
		startDate = monday
		endDate = sunday
		startDateStr = utils.FormatDate(startDate)
		endDateStr = utils.FormatDate(endDate)
	}

	// Check cache first (if MongoDB client is available) - use weekly reports collection
	if s.mongoClient != nil {
		cacheKey := database.GenerateCacheKey(request.Org, request.OrgID, request.Users, startDateStr, endDateStr)
		cachedReport, err := s.mongoClient.GetCachedWeeklyReport(cacheKey)
		if err != nil {
			// Log error but continue with generation
			fmt.Printf("WARNING: Failed to check weekly report cache: %v\n", err)
		} else if cachedReport != nil {
			fmt.Printf("Weekly report cache hit for key: %s\n", cacheKey)
			return cachedReport, nil
		}
		fmt.Printf("Weekly report cache miss for key: %s, generating new weekly report\n", cacheKey)
	}

	// Add one day to endDate to include the full end date (only for date-based queries, not custom datetime)
	if request.CustomStartDate == nil || request.CustomEndDate == nil {
		endDate = endDate.Add(24 * time.Hour)
	}

	// Resolve Gemini API key (request body, then env, then app data file for desktop/bundled use)
	geminiKey := resolveGeminiAPIKey(request.GeminiAPIKey)

	// Aggregate data across ALL users for organization-level analysis
	var allUsersData []models.User
	var allRawDataContexts []string

	// Create a temporary GenerateReportRequest for buildUserReportFromData
	tempRequest := models.GenerateReportRequest{
		AccountID:    request.AccountID,
		Users:        request.Users,
		Org:          request.Org,
		OrgID:        request.OrgID,
		StartDate:    startDateStr,
		EndDate:      endDateStr,
		GeminiAPIKey: geminiKey,
	}

	// Collect data from all users (for structure) but we'll generate org-level summaries
	for _, userReq := range request.Users {
		// Query all data from InfluxDB for this user using IDs for filtering
		afkData, windowData, appData, metricsData, timelineData, err := s.dataService.QueryAllData(
			request.AccountID,
			request.OrgID,
			userReq.ID,
			startDate,
			endDate,
		)
		if err != nil {
			return nil, fmt.Errorf("failed to query data for user %s (ID: %d): %w", userReq.Name, userReq.ID, err)
		}

		// Log what we got
		fmt.Printf("DEBUG: User %s (ID: %d) - Retrieved data - AFK: %d records, Window: %d records, App: %d records, Metrics: %d records, Timeline: %d records\n",
			userReq.Name, userReq.ID, len(afkData), len(windowData), len(appData), len(metricsData), len(timelineData))

		// Aggregate hourly data
		hourlyData := s.dataService.AggregateHourlyData(afkData, windowData, timelineData, startDate, endDate)

		// Build user report structure (use user name for the report)
		userReport := s.buildUserReportFromData(hourlyData, userReq.Name, tempRequest, startDate, endDate)

		// Collect raw data context for organization-level aggregation
		rawDataContext := s.dataService.AggregateDataForAI(afkData, windowData, appData, metricsData, timelineData)
		allRawDataContexts = append(allRawDataContexts, rawDataContext)

		allUsersData = append(allUsersData, userReport)
	}

	// Build final report with all users
	org := models.Organization{
		OrganizationName: request.Org,
		Users:            allUsersData,
	}

	// Calculate user rankings if there are 2+ users
	if len(allUsersData) >= 2 {
		userRanking := s.calculateUserRankings(allUsersData)
		org.UserRanking = userRanking

		// Enhance rankings with AI-generated insights
		err = s.aiService.EnhanceRankingsWithAI(geminiKey, userRanking, allUsersData, tempRequest)
		if err != nil {
			// Log error but don't fail - rankings are still valid
			fmt.Printf("WARNING: Failed to enhance rankings with AI: %v\n", err)
		}
	}

	report := &models.Report{
		Organizations: []models.Organization{org},
		GeneratedAt: time.Now().UTC().Format(time.RFC3339),
		PeriodAnalyzed: models.Period{
			StartDate: startDateStr,
			EndDate:   endDateStr,
		},
	}

	// Build weekly user summaries (condensed format)
	weeklyUserSummaries := s.buildWeeklyUserSummaries(allUsersData)

	// Build weekly organization summary with top/bottom 5 rankings
	weeklySummary := s.buildWeeklyOrganizationSummary(allUsersData, weeklyUserSummaries)

	// Generate AI-enhanced summaries
	combinedRawDataContext := strings.Join(allRawDataContexts, "\n\n--- USER SEPARATOR ---\n\n")
	err = s.aiService.EnhanceWeeklyReportSummaries(geminiKey, report, weeklySummary, weeklyUserSummaries, combinedRawDataContext, request, startDateStr, endDateStr)
	if err != nil {
		// Log error but don't fail - we still have a valid report structure
		fmt.Printf("WARNING: Failed to enhance weekly report summaries with AI: %v\n", err)
	}

	// Attach weekly summary and user summaries to organization
	org.WeeklySummary = weeklySummary
	org.WeeklyUserSummaries = weeklyUserSummaries
	report.Organizations[0] = org

	// Cache the generated weekly report (if MongoDB client is available) - use weekly reports collection
	if s.mongoClient != nil {
		err = s.mongoClient.CacheWeeklyReport(request.Org, request.OrgID, request.Users, startDateStr, endDateStr, report)
		if err != nil {
			// Log error but don't fail - the report is still valid
			fmt.Printf("WARNING: Failed to cache weekly report: %v\n", err)
		} else {
			fmt.Printf("Weekly report cached successfully in weekly_reports collection\n")
		}
	}

	return report, nil
}

// GenerateWeeklyReportSync generates a weekly report synchronously (for scheduled emails)
// This is the same as GenerateWeeklyReport but doesn't require polling
func (s *ReportService) GenerateWeeklyReportSync(request models.GenerateWeeklyReportRequest) (*models.Report, error) {
	return s.GenerateWeeklyReport(request)
}

// calculateUserRankings calculates comparative rankings for multiple users
func (s *ReportService) calculateUserRankings(users []models.User) *models.UserRanking {
	if len(users) < 2 {
		return nil
	}

	// Create ranking entries for each user
	rankings := make([]models.UserRank, len(users))
	for i, user := range users {
		// Calculate active percentage: totalActive / (totalActive + totalAfk) * 100
		totalTime := user.OverallReport.TotalActiveHours + user.OverallReport.TotalAfkHours
		var activePercentage float64
		if totalTime > 0 {
			activePercentage = (user.OverallReport.TotalActiveHours / totalTime) * 100
		}

		rankings[i] = models.UserRank{
			UserName:                user.UserName,
			TotalActiveHours:        user.OverallReport.TotalActiveHours,
			AverageDailyActiveHours: user.OverallReport.AverageDailyActiveHours,
			TotalAfkHours:           user.OverallReport.TotalAfkHours,
			ActivePercentage:        activePercentage,
			TotalDiscrepancies:      user.OverallReport.TotalDiscrepancies,
			CriticalDiscrepancies:   user.OverallReport.CriticalDiscrepancies,
		}
	}

	// Sort by multiple criteria:
	// 1. Active percentage (descending - higher is better)
	// 2. Total active hours (descending - more is better)
	// 3. Average daily active hours (descending - more is better)
	// 4. Total discrepancies (ascending - fewer is better)
	// 5. Critical discrepancies (ascending - fewer is better)
	sort.Slice(rankings, func(i, j int) bool {
		ri, rj := rankings[i], rankings[j]

		// Primary: Active percentage (higher is better)
		if ri.ActivePercentage != rj.ActivePercentage {
			return ri.ActivePercentage > rj.ActivePercentage
		}

		// Secondary: Total active hours
		if ri.TotalActiveHours != rj.TotalActiveHours {
			return ri.TotalActiveHours > rj.TotalActiveHours
		}

		// Tertiary: Average daily active hours
		if ri.AverageDailyActiveHours != rj.AverageDailyActiveHours {
			return ri.AverageDailyActiveHours > rj.AverageDailyActiveHours
		}

		// Quaternary: Total discrepancies (fewer is better)
		if ri.TotalDiscrepancies != rj.TotalDiscrepancies {
			return ri.TotalDiscrepancies < rj.TotalDiscrepancies
		}

		// Quinary: Critical discrepancies (fewer is better)
		return ri.CriticalDiscrepancies < rj.CriticalDiscrepancies
	})

	// Assign ranks (1 = best, 2 = second, etc.)
	// Handle ties by giving same rank
	currentRank := 1
	for i := range rankings {
		if i > 0 {
			// Check if this user is different from previous
			prev := rankings[i-1]
			curr := rankings[i]
			if prev.ActivePercentage != curr.ActivePercentage ||
				prev.TotalActiveHours != curr.TotalActiveHours ||
				prev.AverageDailyActiveHours != curr.AverageDailyActiveHours ||
				prev.TotalDiscrepancies != curr.TotalDiscrepancies ||
				prev.CriticalDiscrepancies != curr.CriticalDiscrepancies {
				currentRank = i + 1
			}
		}
		rankings[i].Rank = currentRank
	}

	return &models.UserRanking{
		Rankings: rankings,
		Summary:  "", // Will be filled by AI
	}
}

// buildUserReportFromData builds a user report from aggregated hourly data
func (s *ReportService) buildUserReportFromData(
	hourlyData map[string]map[int]*HourlyAggregation,
	userName string,
	request models.GenerateReportRequest,
	startDate, endDate time.Time,
) models.User {
	// Build hourly breakdowns
	hourlyBreakdowns := s.dataService.BuildHourlyBreakdownFromAggregation(hourlyData)

	// Build daily reports
	dailyReports := []models.DailyReport{}
	currentDate := startDate
	for currentDate.Before(endDate) {
		dateStr := utils.FormatDate(currentDate)
		
		// Get hourly breakdown for this date
		hourlyList, exists := hourlyBreakdowns[dateStr]
		if !exists {
			// Create empty hourly breakdown for this date
			hourlyList = make([]models.HourlyBreakdown, 24)
			for hour := 0; hour < 24; hour++ {
				startTime, endTime := utils.GenerateHourRange(hour)
				hourlyList[hour] = models.HourlyBreakdown{
					Hour:          hour,
					StartTime:     startTime,
					EndTime:       endTime,
					ActiveMinutes: 0,
					AfkMinutes:    0,
					AppUsage:      []models.AppUsage{},
					TotalMinutes:  60,
				}
			}
		}

		// Calculate daily totals
		var totalActiveMinutes, totalAfkMinutes float64
		for _, hourly := range hourlyList {
			totalActiveMinutes += hourly.ActiveMinutes
			totalAfkMinutes += hourly.AfkMinutes
		}

		dailyReports = append(dailyReports, models.DailyReport{
			Date:                dateStr,
			HourlyBreakdown:     hourlyList,
			TotalActiveMinutes:  totalActiveMinutes,
			TotalActiveHours:    utils.MinutesToHours(totalActiveMinutes),
			TotalAfkMinutes:     totalAfkMinutes,
			TotalAfkHours:       utils.MinutesToHours(totalAfkMinutes),
			NotableDiscrepancies: []models.Discrepancy{},
			Summary:             "", // Will be filled by AI
		})

		currentDate = currentDate.Add(24 * time.Hour)
	}

	// Calculate overall totals
	var totalActiveMinutes, totalAfkMinutes float64
	for _, daily := range dailyReports {
		totalActiveMinutes += daily.TotalActiveMinutes
		totalAfkMinutes += daily.TotalAfkMinutes
	}
	dayCount := float64(len(dailyReports))

	// Build user
	user := models.User{
		UserName: userName,
		OverallReport: models.OverallReport{
			PeriodStart:              request.StartDate,
			PeriodEnd:                request.EndDate,
			TotalActiveHours:         utils.MinutesToHours(totalActiveMinutes),
			TotalActiveMinutes:       totalActiveMinutes,
			TotalAfkHours:            utils.MinutesToHours(totalAfkMinutes),
			TotalAfkMinutes:          totalAfkMinutes,
			AverageDailyActiveHours:  utils.MinutesToHours(totalActiveMinutes / dayCount),
			AverageDailyActiveMinutes: totalActiveMinutes / dayCount,
			TotalDiscrepancies:       0, // Will be calculated by AI
			CriticalDiscrepancies:    0, // Will be calculated by AI
			Summary:                  "", // Will be filled by AI
			Conclusion:               "", // Will be filled by AI
		},
		DailyReports: dailyReports,
	}

	return user
}

// populateHourlyBreakdowns replaces AI-generated hourly breakdowns with pre-calculated data
func (s *ReportService) populateHourlyBreakdowns(
	report *models.Report,
	hourlyBreakdowns map[string][]models.HourlyBreakdown,
	request models.GenerateReportRequest,
) {
	if report == nil || len(report.Organizations) == 0 {
		return
	}

	for i := range report.Organizations {
		org := &report.Organizations[i]
		for j := range org.Users {
			user := &org.Users[j]
			
			// Replace daily reports with our pre-calculated hourly breakdowns
			for k := range user.DailyReports {
				dailyReport := &user.DailyReports[k]
				
				// Get pre-calculated hourly breakdown for this date
				if breakdowns, exists := hourlyBreakdowns[dailyReport.Date]; exists {
					dailyReport.HourlyBreakdown = breakdowns
					
					// Recalculate daily totals from hourly breakdown
					var totalActiveMinutes, totalAfkMinutes float64
					for _, hourly := range breakdowns {
						totalActiveMinutes += hourly.ActiveMinutes
						totalAfkMinutes += hourly.AfkMinutes
					}
					dailyReport.TotalActiveMinutes = totalActiveMinutes
					dailyReport.TotalActiveHours = utils.MinutesToHours(totalActiveMinutes)
					dailyReport.TotalAfkMinutes = totalAfkMinutes
					dailyReport.TotalAfkHours = utils.MinutesToHours(totalAfkMinutes)
				}
			}
			
			// Recalculate overall report totals from daily reports
			var totalActiveMinutes, totalAfkMinutes float64
			dayCount := float64(len(user.DailyReports))
			for _, daily := range user.DailyReports {
				totalActiveMinutes += daily.TotalActiveMinutes
				totalAfkMinutes += daily.TotalAfkMinutes
			}
			
			user.OverallReport.TotalActiveMinutes = totalActiveMinutes
			user.OverallReport.TotalActiveHours = utils.MinutesToHours(totalActiveMinutes)
			user.OverallReport.TotalAfkMinutes = totalAfkMinutes
			user.OverallReport.TotalAfkHours = utils.MinutesToHours(totalAfkMinutes)
			
			if dayCount > 0 {
				user.OverallReport.AverageDailyActiveMinutes = totalActiveMinutes / dayCount
				user.OverallReport.AverageDailyActiveHours = utils.MinutesToHours(user.OverallReport.AverageDailyActiveMinutes)
			}
		}
	}
}

// calculateDistractedTime calculates total time spent on unproductive activities from discrepancies
func (s *ReportService) calculateDistractedTime(user models.User) float64 {
	var totalDistractedMinutes float64
	
	// Sum up duration from all discrepancies that represent unproductive active time
	// These are: social_media, media_consumption, low_productivity_apps
	unproductiveTypes := map[string]bool{
		"social_media":      true,
		"media_consumption": true,
		"low_productivity_apps": true,
	}
	
	for _, daily := range user.DailyReports {
		for _, discrepancy := range daily.NotableDiscrepancies {
			if unproductiveTypes[discrepancy.Type] {
				totalDistractedMinutes += discrepancy.DurationMinutes
			}
		}
	}
	
	return totalDistractedMinutes
}

// buildWeeklyUserSummaries creates condensed user summaries for weekly reports
func (s *ReportService) buildWeeklyUserSummaries(users []models.User) []models.WeeklyUserSummary {
	summaries := make([]models.WeeklyUserSummary, 0, len(users))
	
	for _, user := range users {
		// Calculate activity ratio
		totalTime := user.OverallReport.TotalActiveHours + user.OverallReport.TotalAfkHours
		var activityRatio float64
		if totalTime > 0 {
			activityRatio = (user.OverallReport.TotalActiveHours / totalTime) * 100
		}
		
		// Calculate distracted time
		distractedMinutes := s.calculateDistractedTime(user)
		distractedHours := utils.MinutesToHours(distractedMinutes)
		
		summary := models.WeeklyUserSummary{
			UserName:              user.UserName,
			ActivityRatio:          activityRatio,
			ActiveHours:            user.OverallReport.TotalActiveHours,
			ActiveMinutes:          user.OverallReport.TotalActiveMinutes,
			AfkHours:               user.OverallReport.TotalAfkHours,
			AfkMinutes:             user.OverallReport.TotalAfkMinutes,
			TotalDiscrepancies:     user.OverallReport.TotalDiscrepancies,
			CriticalDiscrepancies:  user.OverallReport.CriticalDiscrepancies,
			DistractedTimeHours:    distractedHours,
			DistractedTimeMinutes:  distractedMinutes,
			ProductivitySummary:    "", // Will be filled by AI
		}
		
		summaries = append(summaries, summary)
	}
	
	return summaries
}

// buildWeeklyOrganizationSummary creates the weekly organization summary with top/bottom X rankings
// For orgs with 10+ employees: top 5 / bottom 5
// For orgs with <10 employees: split evenly (e.g., 4 employees = top 2 / bottom 2)
func (s *ReportService) buildWeeklyOrganizationSummary(users []models.User, userSummaries []models.WeeklyUserSummary) *models.WeeklyOrganizationSummary {
	// Create weekly user ranks for sorting
	weeklyRanks := make([]models.WeeklyUserRank, 0, len(users))
	
	for _, summary := range userSummaries {
		weeklyRanks = append(weeklyRanks, models.WeeklyUserRank{
			UserName:          summary.UserName,
			ActiveHours:       summary.ActiveHours,
			ActivityRatio:     summary.ActivityRatio,
			TotalDiscrepancies: summary.TotalDiscrepancies,
		})
	}
	
	// Sort by activity ratio (descending) for top performers
	sort.Slice(weeklyRanks, func(i, j int) bool {
		if weeklyRanks[i].ActivityRatio != weeklyRanks[j].ActivityRatio {
			return weeklyRanks[i].ActivityRatio > weeklyRanks[j].ActivityRatio
		}
		// Tie-breaker: more active hours is better
		return weeklyRanks[i].ActiveHours > weeklyRanks[j].ActiveHours
	})
	
	// Calculate how many to show in top/bottom
	totalEmployees := len(weeklyRanks)
	var topCount, bottomCount int
	
	if totalEmployees >= 10 {
		// For 10+ employees: top 5 / bottom 5
		topCount = 5
		bottomCount = 5
	} else {
		// For <10 employees: split evenly down the middle
		topCount = totalEmployees / 2
		bottomCount = totalEmployees / 2
		// If odd number, the middle employee goes to top
		if totalEmployees%2 == 1 {
			topCount++
		}
	}
	
	// Get top X
	topX := make([]models.WeeklyUserRank, 0, topCount)
	for i := 0; i < len(weeklyRanks) && i < topCount; i++ {
		rank := weeklyRanks[i]
		rank.Rank = i + 1
		topX = append(topX, rank)
	}
	
	// Get bottom X (reverse order)
	bottomX := make([]models.WeeklyUserRank, 0, bottomCount)
	rankNum := 1
	for i := len(weeklyRanks) - 1; i >= 0 && len(bottomX) < bottomCount; i-- {
		rank := weeklyRanks[i]
		rank.Rank = rankNum
		bottomX = append(bottomX, rank)
		rankNum++
	}
	
	return &models.WeeklyOrganizationSummary{
		ProductivitySummary: "", // Will be filled by AI
		Top5Employees:       topX,
		Bottom5Employees:    bottomX,
	}
}

