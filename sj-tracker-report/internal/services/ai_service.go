package services

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"sj-tracker-report/internal/config"
	"sj-tracker-report/internal/models"
	"sj-tracker-report/internal/utils"
	"os"
	"strconv"
	"strings"
	"time"
)

// defaultGeminiModel is the Gemini model used for report AI when config does not specify one.
// Use a stable model supported by generateContent (v1beta).
const defaultGeminiModel = "gemini-2.5-flash"

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// fixReportStructure fixes common issues in the AI-generated report
func fixReportStructure(report *models.Report) {
	for i := range report.Organizations {
		for j := range report.Organizations[i].Users {
			user := &report.Organizations[i].Users[j]
			
			// Fix periodStart and periodEnd if empty
			if user.OverallReport.PeriodStart == "" && report.PeriodAnalyzed.StartDate != "" {
				user.OverallReport.PeriodStart = report.PeriodAnalyzed.StartDate
			}
			if user.OverallReport.PeriodEnd == "" && report.PeriodAnalyzed.EndDate != "" {
				user.OverallReport.PeriodEnd = report.PeriodAnalyzed.EndDate
			}
			
			// Fix conclusion if empty (generate a basic one)
			if user.OverallReport.Conclusion == "" {
				avgHours := user.OverallReport.AverageDailyActiveHours
				if avgHours < 6 {
					user.OverallReport.Conclusion = fmt.Sprintf("Critical: Average daily active hours (%.2f) is below the expected minimum of 6 hours per day. Working time is insufficient.", avgHours)
				} else if avgHours < 7 {
					user.OverallReport.Conclusion = fmt.Sprintf("Adequate: Average daily active hours (%.2f) meets minimum expectations but could be improved.", avgHours)
				} else {
					user.OverallReport.Conclusion = fmt.Sprintf("Good: Average daily active hours (%.2f) meets or exceeds expectations for full-time work.", avgHours)
				}
			}
			
			// Fix totalMinutes in hourly breakdown (should always be 60)
			for k := range user.DailyReports {
				for l := range user.DailyReports[k].HourlyBreakdown {
					hourly := &user.DailyReports[k].HourlyBreakdown[l]
					if hourly.TotalMinutes == 0 {
						hourly.TotalMinutes = 60
					}
					
					// Fix startTime and endTime if empty
					if hourly.StartTime == "" || hourly.EndTime == "" {
						startTime, endTime := utils.GenerateHourRange(hourly.Hour)
						hourly.StartTime = startTime
						hourly.EndTime = endTime
					}
					
					// Remove appUsage entries with empty appName
					validAppUsage := []models.AppUsage{}
					for _, app := range hourly.AppUsage {
						if app.AppName != "" {
							validAppUsage = append(validAppUsage, app)
						}
					}
					hourly.AppUsage = validAppUsage
				}
				
				// Fix null notableDiscrepancies to empty array
				if user.DailyReports[k].NotableDiscrepancies == nil {
					user.DailyReports[k].NotableDiscrepancies = []models.Discrepancy{}
				}
			}
		}
	}
}

// normalizeReportData fixes type mismatches in the AI-generated JSON
func normalizeReportData(data map[string]interface{}) ([]byte, error) {
	// Recursively fix the data structure
	fixTypes(data)
	return json.Marshal(data)
}

// fixTypes recursively fixes type mismatches in the report structure
func fixTypes(v interface{}) {
	switch val := v.(type) {
	case map[string]interface{}:
		// Fix hour field - it should be an integer 0-23, not a time string
		if hourVal, ok := val["hour"]; ok {
			if hourStr, ok := hourVal.(string); ok {
				// If it's a time string like "00:00", extract the hour part
				if strings.Contains(hourStr, ":") {
					parts := strings.Split(hourStr, ":")
					if len(parts) > 0 {
						if hourInt, err := strconv.Atoi(parts[0]); err == nil && hourInt >= 0 && hourInt <= 23 {
							val["hour"] = hourInt
						}
					}
				} else {
					// Try parsing as integer string
					if hourInt, err := strconv.Atoi(hourStr); err == nil {
						val["hour"] = hourInt
					}
				}
			} else if hourFloat, ok := hourVal.(float64); ok {
				val["hour"] = int(hourFloat)
			}
		}
		
		// Fix other integer fields that might come as strings
		intFields := []string{"totalMinutes", "totalDiscrepancies", "criticalDiscrepancies"}
		for _, field := range intFields {
			if fieldVal, ok := val[field]; ok {
				if fieldStr, ok := fieldVal.(string); ok {
					if fieldInt, err := strconv.Atoi(fieldStr); err == nil {
						val[field] = fieldInt
					}
				} else if fieldFloat, ok := fieldVal.(float64); ok {
					// Sometimes JSON unmarshals integers as float64
					val[field] = int(fieldFloat)
				}
			}
		}
		
		// Fix float fields that might come as strings
		floatFields := []string{"totalActiveHours", "totalActiveMinutes", "totalAfkHours", "totalAfkMinutes", 
			"averageDailyActiveHours", "averageDailyActiveMinutes", "activeMinutes", "afkMinutes", 
			"durationMinutes", "utilization_ratio"}
		for _, field := range floatFields {
			if fieldVal, ok := val[field]; ok {
				if fieldStr, ok := fieldVal.(string); ok {
					if fieldFloat, err := strconv.ParseFloat(fieldStr, 64); err == nil {
						val[field] = fieldFloat
					}
				}
			}
		}
		
		// Recursively process nested structures
		for _, nestedVal := range val {
			fixTypes(nestedVal)
		}
	case []interface{}:
		for _, item := range val {
			fixTypes(item)
		}
	}
}

// AIService handles Gemini API interactions
type AIService struct {
	config     config.OpenAIConfig // Reusing config struct for model/temperature settings
	schemaPath string
}

// NewAIService creates a new AI service (no longer needs API key at initialization)
func NewAIService(cfg config.OpenAIConfig, schemaPath string) *AIService {
	return &AIService{
		config:     cfg,
		schemaPath: schemaPath,
	}
}

// Gemini API request/response structures
type geminiRequest struct {
	Contents []geminiContent `json:"contents"`
	GenerationConfig geminiGenerationConfig `json:"generationConfig"`
}

type geminiContent struct {
	Parts []geminiPart `json:"parts"`
}

type geminiPart struct {
	Text string `json:"text"`
}

type geminiGenerationConfig struct {
	Temperature     float64 `json:"temperature"`
	MaxOutputTokens int     `json:"maxOutputTokens"`
	ResponseMimeType string `json:"responseMimeType,omitempty"`
}

type geminiResponse struct {
	Candidates []geminiCandidate `json:"candidates"`
}

type geminiCandidate struct {
	Content geminiContent `json:"content"`
}

// callGeminiAPI makes an HTTP request to the Gemini API
func callGeminiAPI(apiKey string, model string, systemPrompt string, userPrompt string, temperature float64, maxTokens int) (string, error) {
	// Combine system and user prompts (Gemini doesn't have separate system/user roles in the same way)
	fullPrompt := systemPrompt + "\n\n" + userPrompt
	
	// Default model if not specified
	if model == "" {
		model = defaultGeminiModel
	}
	
	// Default max tokens
	if maxTokens <= 0 {
		maxTokens = 8192
	}
	
	// Build request
	reqBody := geminiRequest{
		Contents: []geminiContent{
			{
				Parts: []geminiPart{
					{Text: fullPrompt},
				},
			},
		},
		GenerationConfig: geminiGenerationConfig{
			Temperature:     temperature,
			MaxOutputTokens:  maxTokens,
			ResponseMimeType: "application/json",
		},
	}
	
	jsonData, err := json.Marshal(reqBody)
	if err != nil {
		return "", fmt.Errorf("failed to marshal request: %w", err)
	}
	
	// Make HTTP request
	url := fmt.Sprintf("https://generativelanguage.googleapis.com/v1beta/models/%s:generateContent?key=%s", model, apiKey)
	req, err := http.NewRequestWithContext(context.Background(), "POST", url, bytes.NewBuffer(jsonData))
	if err != nil {
		return "", fmt.Errorf("failed to create request: %w", err)
	}
	
	req.Header.Set("Content-Type", "application/json")
	
	client := &http.Client{Timeout: 120 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return "", fmt.Errorf("Gemini API request failed: %w", err)
	}
	defer resp.Body.Close()
	
	if resp.StatusCode != http.StatusOK {
		bodyBytes, _ := io.ReadAll(resp.Body)
		return "", fmt.Errorf("Gemini API error (status %d): %s", resp.StatusCode, string(bodyBytes))
	}
	
	// Parse response
	var geminiResp geminiResponse
	if err := json.NewDecoder(resp.Body).Decode(&geminiResp); err != nil {
		return "", fmt.Errorf("failed to parse Gemini response: %w", err)
	}
	
	if len(geminiResp.Candidates) == 0 || len(geminiResp.Candidates[0].Content.Parts) == 0 {
		return "", fmt.Errorf("no response from Gemini")
	}
	
	return geminiResp.Candidates[0].Content.Parts[0].Text, nil
}

// GenerateReport generates a report using Gemini with structured outputs
func (s *AIService) GenerateReport(apiKey string, dataContext string, request models.GenerateReportRequest) (*models.Report, error) {
	// Build system prompt
	systemPrompt, err := s.buildSystemPrompt()
	if err != nil {
		return nil, fmt.Errorf("failed to build system prompt: %w", err)
	}

	// Build user prompt
	userPrompt := s.buildUserPrompt(dataContext, request)

	// Load JSON schema for structured output
	schemaData, err := os.ReadFile(s.schemaPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read schema file: %w", err)
	}

	var schemaMap map[string]interface{}
	if err := json.Unmarshal(schemaData, &schemaMap); err != nil {
		return nil, fmt.Errorf("failed to parse schema: %w", err)
	}

	// Set MaxTokens - default to 8192 for Gemini
	maxTokens := s.config.MaxTokens
	if maxTokens <= 0 {
		maxTokens = 8192
	}
	
	// Use Gemini model (default to defaultGeminiModel)
	model := s.config.Model
	if model == "" || strings.HasPrefix(model, "gpt-") {
		model = defaultGeminiModel
	}
	
	// Build full prompt with JSON requirement
	fullUserPrompt := userPrompt + "\n\nIMPORTANT: Respond with ONLY valid JSON matching the required structure. No markdown, no code fences, no explanatory text. The organizations array MUST be present, non-null, and non-empty. Generate the complete report structure even if data is limited."

	// Make Gemini API call
	reportJSON, err := callGeminiAPI(apiKey, model, systemPrompt, fullUserPrompt, s.config.Temperature, maxTokens)
	if err != nil {
		return nil, fmt.Errorf("Gemini API error: %w", err)
	}
	
	// Clean up the JSON - remove markdown code fences if present
	reportJSON = strings.TrimSpace(reportJSON)
	if strings.HasPrefix(reportJSON, "```json") {
		reportJSON = strings.TrimPrefix(reportJSON, "```json")
		reportJSON = strings.TrimSuffix(reportJSON, "```")
		reportJSON = strings.TrimSpace(reportJSON)
	} else if strings.HasPrefix(reportJSON, "```") {
		reportJSON = strings.TrimPrefix(reportJSON, "```")
		reportJSON = strings.TrimSuffix(reportJSON, "```")
		reportJSON = strings.TrimSpace(reportJSON)
	}

	// Parse JSON into Report struct with custom unmarshaling to handle type mismatches
	var reportData map[string]interface{}
	if err := json.Unmarshal([]byte(reportJSON), &reportData); err != nil {
		responsePreview := reportJSON
		if len(responsePreview) > 500 {
			responsePreview = responsePreview[:500] + "..."
		}
		return nil, fmt.Errorf("failed to parse report JSON: %w (response preview: %s)", err, responsePreview)
	}
	
	// Normalize the data structure to fix type mismatches
	normalizedJSON, err := normalizeReportData(reportData)
	if err != nil {
		return nil, fmt.Errorf("failed to normalize report data: %w", err)
	}
	
	// Now unmarshal into the struct
	var report models.Report
	if err := json.Unmarshal(normalizedJSON, &report); err != nil {
		responsePreview := string(normalizedJSON)
		if len(responsePreview) > 500 {
			responsePreview = responsePreview[:500] + "..."
		}
		return nil, fmt.Errorf("failed to unmarshal normalized report: %w (preview: %s)", err, responsePreview)
	}
	
	// Validate that organizations array is not null and not empty
	if report.Organizations == nil {
		// Try to fix it - create a minimal structure
		report.Organizations = []models.Organization{
			{
				OrganizationName: "Unknown",
				Users: []models.User{},
			},
		}
		return nil, fmt.Errorf("AI generated report with null organizations array - this should not happen. Please check the AI prompt and ensure it always generates the required structure")
	}
	
	if len(report.Organizations) == 0 {
		return nil, fmt.Errorf("AI generated report with empty organizations array")
	}

	// Set generatedAt timestamp
	report.GeneratedAt = time.Now().UTC().Format(time.RFC3339)
	
	// Post-process the report to fix common issues
	fixReportStructure(&report)

	return &report, nil
}

// EnhanceReportWithAI enhances an existing report structure with AI-generated text fields
func (s *AIService) EnhanceReportWithAI(
	apiKey string,
	report *models.Report,
	rawDataContext string,
	request models.GenerateReportRequest,
) error {
	// Convert report to JSON for context
	reportJSON, err := json.MarshalIndent(report, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal report: %w", err)
	}

	// Extract app usage summary for better discrepancy detection
	appUsageSummary := s.extractAppUsageSummary(report)

	// Build prompt for AI to fill in text fields
	prompt := fmt.Sprintf(`You are a productivity analyst detecting discrepancies and unproductive activity patterns in work monitoring data.

EXISTING REPORT STRUCTURE (all numeric data is already calculated):
%s

APP USAGE SUMMARY (analyze this carefully for discrepancies):
%s

RAW DATA CONTEXT (for additional context):
%s

YOUR TASK - DISCREPANCY DETECTION:
Carefully analyze the app usage data above, paying special attention to WINDOW TITLES to detect unproductive use of productive apps.

CRITICAL: Create ONE discrepancy entry per unproductive activity that includes:
- ALL time periods where that activity occurred (list all periods from the data)
- TOTAL cumulative duration across all periods
- Analyze WINDOW TITLES to detect unproductive browsing in browsers (Chrome, Firefox, Edge, Safari)

SEVERITY JUDGMENT - TWO FACTORS:
1. TOTAL daily usage: Judge based on total cumulative duration across all periods
2. CONCENTRATED usage: Check if any single hour has > 30 minutes of unproductive activity
   - If an unproductive app takes up > 30 minutes in a single hour, this is CRITICAL severity
   - If an unproductive app takes up > 20 minutes in a single hour, this is HIGH severity
   - Concentrated usage is more problematic than spread-out usage
   - Example: 40 minutes of Spotify in one hour = CRITICAL, even if total daily is only 40 minutes

Look for:

1. SOCIAL_MEDIA: 
   - Direct apps: Facebook, Twitter/X, Instagram, TikTok, LinkedIn (personal), Reddit, Discord, Slack (non-work), Snapchat, WhatsApp (personal)
   - Browser usage: ONLY flag Chrome/Firefox/Edge/Safari if window titles contain:
     * facebook.com, twitter.com, x.com, instagram.com, tiktok.com, reddit.com, linkedin.com (personal), discord.com, snapchat.com
     * Look for URLs, page titles mentioning social media
   - DO NOT flag browsers without unproductive window titles - browsers are productive tools
   - IMPORTANT: Check window titles for productive context:
     * Reddit: Work-related subreddits (r/programming, r/webdev, r/sysadmin, r/learnprogramming) = potentially productive
     * Twitter/X: Professional networking, industry news = potentially productive
     * LinkedIn: Generally productive unless clearly personal use
     * Discord/Slack: Work-related channels = productive
   - ALWAYS flag social media usage as a discrepancy
   - If productive context found: Flag with LOW severity and note in description/context about potential productive use
   - Aggregate ALL occurrences (both direct apps and browser usage)
   - List ALL time periods with durations
   - Calculate TOTAL duration across all periods
   - Check for CONCENTRATED usage: If any single hour has > 30 min of social media = CRITICAL
   - Base severity on BOTH total daily usage AND concentrated usage:
     * Concentrated (> 30 min in one hour) = CRITICAL
     * Concentrated (> 20 min in one hour) = HIGH
     * Total daily: < 15 min = low, 15-60 min = medium, 60-120 min = high, > 120 min = critical
   - Use the HIGHER severity from concentrated vs total daily
   - Adjust severity down if productive context is evident (but still flag concentrated usage)

2. MEDIA_CONSUMPTION: 
   - Direct apps: YouTube, Netflix, Hulu, Spotify, gaming platforms (Steam, Epic Games, etc.), Twitch
   - Browser usage: ONLY flag Chrome/Firefox/Edge/Safari if window titles contain:
     * youtube.com, netflix.com, hulu.com, spotify.com, twitch.tv, steam, epic games
     * Look for video/streaming content URLs
   - DO NOT flag browsers without unproductive window titles - browsers are productive tools
   - IMPORTANT: Check window titles for productive context:
     * YouTube: Educational content (tutorials, courses, tech talks, professional development, documentation) = potentially productive
     * Spotify: Background music during work = generally acceptable (may skip if clearly background)
     * Twitch: Tech streams, coding streams = potentially productive
   - ALWAYS flag media consumption as a discrepancy (except possibly background music)
   - If productive context found: Flag with LOW severity and note in description/context about potential productive use
   - Same aggregation approach - one entry per activity type with all periods
   - Check for CONCENTRATED usage: If any single hour has > 30 min of media consumption = CRITICAL
   - Base severity on BOTH total daily usage AND concentrated usage:
     * Concentrated (> 30 min in one hour) = CRITICAL
     * Concentrated (> 20 min in one hour) = HIGH
     * Total daily: < 15 min = low, 15-60 min = medium, 60-120 min = high, > 120 min = critical
   - Use the HIGHER severity from concentrated vs total daily
   - Adjust for productive context (but still flag concentrated usage)

3. LOW_PRODUCTIVITY_APPS: 
   - Direct apps: Games, entertainment apps, shopping sites (Amazon, eBay for non-work), dating apps
   - Browser usage: ONLY flag Chrome/Firefox/Edge/Safari if window titles contain:
     * amazon.com, ebay.com, shopping sites, dating sites, gaming sites
     * Entertainment/news sites used during work hours
   - DO NOT flag browsers without unproductive window titles - browsers are productive tools
   - Aggregate all occurrences per activity type
   - Consider total daily usage when setting severity

4. EXTENDED_AFK: Already calculated in the report - flag periods > 30 min during work hours

5. SUSPICIOUS_PATTERN: 
   - Minimal app usage suggesting minimal actual work
   - Only activity at start/end of day
   - Patterns inconsistent with productive work

WINDOW TITLE ANALYSIS:
- When you see browsers (Chrome, Firefox, Edge, Safari) in the app list, ALWAYS check the window titles
- Window titles often contain URLs or page titles that reveal the actual content being viewed
- Examples: "Reddit - Dive into anything" indicates Reddit usage, "YouTube" indicates YouTube, "Facebook" indicates Facebook
- Look for domain names, site names, or content indicators in window titles
- CRITICAL: Browsers (Chrome, Firefox, Edge, Safari) should NOT be flagged as unproductive UNLESS window titles show unproductive content
- Browsers are productive tools when used for work - only flag them if window titles indicate social media, entertainment, shopping, etc.
- If window titles are missing or unclear, do NOT flag browser usage as unproductive
- If window titles show unproductive content, create a discrepancy for that specific content (e.g., "Chrome (Reddit usage detected via window titles)")

PRODUCTIVE USE CONTEXT:
- Some platforms can be used productively. Check window titles for context:
  * Reddit: Look for work-related subreddits (r/programming, r/webdev, r/sysadmin, r/learnprogramming, etc.)
  * YouTube: Look for educational content (tutorials, courses, tech talks, professional development)
  * LinkedIn: Generally productive for professional networking
  * Discord/Slack: Could be work-related channels/teams
  * Twitter/X: Could be professional networking or industry news
- CRITICAL: ALWAYS flag potentially unproductive activities (social media, games, media consumption) as discrepancies
- If window titles suggest productive use:
  * STILL CREATE THE DISCREPANCY (don't skip it)
  * Use LOWER severity (e.g., if would be "medium", make it "low" if productive context found)
  * Note in description: "Potentially productive use - window titles suggest [educational/professional] content"
  * Note in context: "Window titles indicate work-related content, but usage should still be monitored"
- The goal is transparency - flag everything, but provide context about potential productive use

For each discrepancy:
- startTime: First occurrence start time
- endTime: Last occurrence end time  
- durationMinutes: TOTAL cumulative duration across ALL occurrences
- description: Include app name (or "Chrome/Firefox/Edge" if detected via window titles), total duration, list all time periods with individual durations (e.g., "09:00-10:00 (40 min), 14:00-15:00 (5 min)"), and mention window titles if relevant. If productive context is evident, mention it (e.g., "Reddit usage detected, but window titles suggest work-related subreddits (r/programming, r/webdev)"). If concentrated usage is detected, highlight it (e.g., "40 minutes of Spotify in a single hour (09:00-10:00) - concentrated usage")
- context: Additional context about the usage pattern. If productive use is possible, explicitly state: "Potentially productive use - window titles suggest [educational/professional] content" or "Could be productive - window titles indicate [specific productive context]". If concentrated usage detected, note: "Concentrated usage detected - [X] minutes in a single hour indicates significant distraction"
- severity: Based on BOTH total daily usage AND concentrated usage. Use the HIGHER severity:
  * If > 30 min in one hour = CRITICAL (regardless of total daily)
  * If > 20 min in one hour = HIGH (unless total daily would be critical)
  * Otherwise base on total daily: < 15 min = low, 15-60 min = medium, 60-120 min = high, > 120 min = critical
  * Reduce severity if productive context is evident, but concentrated usage should still be flagged appropriately

YOUR TASK - TEXT GENERATION:
1. Generate a "summary" string for the overallReport (2-3 sentences summarizing the period)
2. Generate a "conclusion" string for the overallReport (1-2 sentences with assessment)
3. Generate a "summary" string for each dailyReport (1-2 sentences per day)
4. Count totalDiscrepancies and criticalDiscrepancies in overallReport

RESPOND WITH ONLY JSON in this exact format:
{
  "overallReport": {
    "summary": "string here",
    "conclusion": "string here",
    "totalDiscrepancies": 0,
    "criticalDiscrepancies": 0
  },
  "dailyReports": [
    {
      "date": "YYYY-MM-DD",
      "summary": "string here",
      "notableDiscrepancies": [
        {
          "type": "extended_afk|social_media|media_consumption|excessive_idle|low_productivity_apps|suspicious_pattern",
          "severity": "low|medium|high|critical",
          "startTime": "HH:MM",
          "endTime": "HH:MM",
          "durationMinutes": 0,
          "description": "App name, total duration, and list all time periods (e.g., 'Reddit used for 18.2 minutes total across periods: 09:00-10:00 (2.2 min), 10:00-11:00 (11 min), 14:00-15:00 (5 min)')",
          "context": "Additional context about usage pattern (e.g., 'Used across 3 separate periods during core work hours')"
        }
      ]
    }
  ]
}

IMPORTANT: 
- Only include the fields above. Do not modify numeric data or structure.
- Be thorough in discrepancy detection - analyze ALL app usage data
- ALWAYS flag potentially unproductive activities (social media, games, media consumption) - never skip them
- If productive context is evident, still flag it but with lower severity and note the productive context
- Include specific app names in discrepancy descriptions
- Use exact times from the hourly breakdown data`, string(reportJSON), appUsageSummary, rawDataContext)

	// Call Gemini API
	model := s.config.Model
	if model == "" || strings.HasPrefix(model, "gpt-") {
		model = defaultGeminiModel
	}
	
	systemPrompt := "You are a productivity analyst. Generate concise, professional summaries and identify discrepancies in work activity data."
	responseJSON, err := callGeminiAPI(apiKey, model, systemPrompt, prompt, s.config.Temperature, 4000)
	if err != nil {
		return fmt.Errorf("Gemini API error: %w", err)
	}
	responseJSON = strings.TrimSpace(responseJSON)
	if strings.HasPrefix(responseJSON, "```json") {
		responseJSON = strings.TrimPrefix(responseJSON, "```json")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	} else if strings.HasPrefix(responseJSON, "```") {
		responseJSON = strings.TrimPrefix(responseJSON, "```")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	}

	var enhancement struct {
		OverallReport struct {
			Summary              string `json:"summary"`
			Conclusion           string `json:"conclusion"`
			TotalDiscrepancies   int    `json:"totalDiscrepancies"`
			CriticalDiscrepancies int   `json:"criticalDiscrepancies"`
		} `json:"overallReport"`
		DailyReports []struct {
			Date                string            `json:"date"`
			Summary             string            `json:"summary"`
			NotableDiscrepancies []models.Discrepancy `json:"notableDiscrepancies"`
		} `json:"dailyReports"`
	}

	if err := json.Unmarshal([]byte(responseJSON), &enhancement); err != nil {
		return fmt.Errorf("failed to parse AI enhancement: %w", err)
	}

	// Apply enhancements to report
	if len(report.Organizations) > 0 && len(report.Organizations[0].Users) > 0 {
		user := &report.Organizations[0].Users[0]
		
		// Update overall report
		user.OverallReport.Summary = enhancement.OverallReport.Summary
		user.OverallReport.Conclusion = enhancement.OverallReport.Conclusion
		user.OverallReport.TotalDiscrepancies = enhancement.OverallReport.TotalDiscrepancies
		user.OverallReport.CriticalDiscrepancies = enhancement.OverallReport.CriticalDiscrepancies

		// Update daily reports
		for i := range user.DailyReports {
			daily := &user.DailyReports[i]
			
			// Find matching enhancement
			for _, enh := range enhancement.DailyReports {
				if enh.Date == daily.Date {
					daily.Summary = enh.Summary
					daily.NotableDiscrepancies = enh.NotableDiscrepancies
					break
				}
			}
		}
	}

	return nil
}

// EnhanceUserReportWithAI enhances a single user's report with AI-generated text fields
func (s *AIService) EnhanceUserReportWithAI(
	apiKey string,
	user *models.User,
	rawDataContext string,
	request models.GenerateReportRequest,
	userName string,
) error {
	// Create a temporary report structure with just this user for AI processing
	tempReport := &models.Report{
		Organizations: []models.Organization{
			{
				OrganizationName: request.Org,
				Users:            []models.User{*user},
			},
		},
		GeneratedAt: time.Now().UTC().Format(time.RFC3339),
		PeriodAnalyzed: models.Period{
			StartDate: request.StartDate,
			EndDate:   request.EndDate,
		},
	}

	// Convert report to JSON for context
	reportJSON, err := json.MarshalIndent(tempReport, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal report: %w", err)
	}

	// Extract app usage summary for this user
	appUsageSummary := s.extractAppUsageSummary(tempReport)

	// Build prompt for AI to fill in text fields
	prompt := fmt.Sprintf(`You are a productivity analyst detecting discrepancies and unproductive activity patterns in work monitoring data.

EXISTING REPORT STRUCTURE (all numeric data is already calculated):
%s

APP USAGE SUMMARY (analyze this carefully for discrepancies):
%s

RAW DATA CONTEXT (for additional context):
%s

YOUR TASK - DISCREPANCY DETECTION:
Carefully analyze the app usage data above, paying special attention to WINDOW TITLES to detect unproductive use of productive apps.

CRITICAL: Create ONE discrepancy entry per unproductive activity that includes:
- ALL time periods where that activity occurred (list all periods from the data)
- TOTAL cumulative duration across all periods
- Analyze WINDOW TITLES to detect unproductive browsing in browsers (Chrome, Firefox, Edge, Safari)

SEVERITY JUDGMENT - TWO FACTORS:
1. TOTAL daily usage: Judge based on total cumulative duration across all periods
2. CONCENTRATED usage: Check if any single hour has > 30 minutes of unproductive activity
   - If an unproductive app takes up > 30 minutes in a single hour, this is CRITICAL severity
   - If an unproductive app takes up > 20 minutes in a single hour, this is HIGH severity
   - Concentrated usage is more problematic than spread-out usage
   - Example: 40 minutes of Spotify in one hour = CRITICAL, even if total daily is only 40 minutes

Look for:

1. SOCIAL_MEDIA: 
   - Direct apps: Facebook, Twitter/X, Instagram, TikTok, LinkedIn (personal), Reddit, Discord, Slack (non-work), Snapchat, WhatsApp (personal)
   - Browser usage: ONLY flag Chrome/Firefox/Edge/Safari if window titles contain:
     * facebook.com, twitter.com, x.com, instagram.com, tiktok.com, reddit.com, linkedin.com (personal), discord.com, snapchat.com
     * Look for URLs, page titles mentioning social media
   - IMPORTANT: Check window titles for productive context:
     * Reddit: Work-related subreddits (r/programming, r/webdev, r/sysadmin, r/learnprogramming) = potentially productive
     * Twitter/X: Professional networking, industry news = potentially productive
     * LinkedIn: Generally productive unless clearly personal use
     * Discord/Slack: Work-related channels = productive
   - ALWAYS flag social media usage as a discrepancy
   - If productive context found: Flag with LOW severity and note in description/context about potential productive use
   - Aggregate ALL occurrences (both direct apps and browser usage)
   - List ALL time periods with durations
   - Calculate TOTAL duration across all periods
   - Check for CONCENTRATED usage: If any single hour has > 30 min of social media = CRITICAL
   - Base severity on BOTH total daily usage AND concentrated usage:
     * Concentrated (> 30 min in one hour) = CRITICAL
     * Concentrated (> 20 min in one hour) = HIGH
     * Total daily: < 15 min = low, 15-60 min = medium, 60-120 min = high, > 120 min = critical
   - Use the HIGHER severity from concentrated vs total daily
   - Adjust severity down if productive context is evident (but still flag concentrated usage)

2. MEDIA_CONSUMPTION: 
   - Direct apps: YouTube, Netflix, Hulu, Spotify, gaming platforms (Steam, Epic Games, etc.), Twitch
   - Browser usage: ONLY flag Chrome/Firefox/Edge/Safari if window titles contain:
     * youtube.com, netflix.com, hulu.com, spotify.com, twitch.tv, steam, epic games
     * Look for video/streaming content URLs
   - IMPORTANT: Check window titles for productive context:
     * YouTube: Educational content (tutorials, courses, tech talks, professional development, documentation) = potentially productive
     * Spotify: Background music during work = generally acceptable (may skip if clearly background)
     * Twitch: Tech streams, coding streams = potentially productive
   - ALWAYS flag media consumption as a discrepancy (except possibly background music)
   - If productive context found: Flag with LOW severity and note in description/context about potential productive use
   - Same aggregation approach - one entry per activity type with all periods
   - Check for CONCENTRATED usage: If any single hour has > 30 min of media consumption = CRITICAL
   - Base severity on BOTH total daily usage AND concentrated usage:
     * Concentrated (> 30 min in one hour) = CRITICAL
     * Concentrated (> 20 min in one hour) = HIGH
     * Total daily: < 15 min = low, 15-60 min = medium, 60-120 min = high, > 120 min = critical
   - Use the HIGHER severity from concentrated vs total daily
   - Adjust for productive context (but still flag concentrated usage)

3. LOW_PRODUCTIVITY_APPS: 
   - Direct apps: Games, entertainment apps, shopping sites (Amazon, eBay for non-work), dating apps
   - Browser usage: ONLY flag Chrome/Firefox/Edge/Safari if window titles contain:
     * amazon.com, ebay.com, shopping sites, dating sites, gaming sites
     * Entertainment/news sites used during work hours
   - DO NOT flag browsers without unproductive window titles - browsers are productive tools
   - Aggregate all occurrences per activity type
   - Consider total daily usage when setting severity

4. EXTENDED_AFK: Already calculated in the report - flag periods > 30 min during work hours

5. SUSPICIOUS_PATTERN: 
   - Minimal app usage suggesting minimal actual work
   - Only activity at start/end of day
   - Patterns inconsistent with productive work

WINDOW TITLE ANALYSIS:
- When you see browsers (Chrome, Firefox, Edge, Safari) in the app list, ALWAYS check the window titles
- Window titles often contain URLs or page titles that reveal the actual content being viewed
- Examples: "Reddit - Dive into anything" indicates Reddit usage, "YouTube" indicates YouTube, "Facebook" indicates Facebook
- Look for domain names, site names, or content indicators in window titles
- CRITICAL: Browsers (Chrome, Firefox, Edge, Safari) should NOT be flagged as unproductive UNLESS window titles show unproductive content
- Browsers are productive tools when used for work - only flag them if window titles indicate social media, entertainment, shopping, etc.
- If window titles are missing or unclear, do NOT flag browser usage as unproductive
- If window titles show unproductive content, create a discrepancy for that specific content (e.g., "Chrome (Reddit usage detected via window titles)")

PRODUCTIVE USE CONTEXT:
- Some platforms can be used productively. Check window titles for context:
  * Reddit: Look for work-related subreddits (r/programming, r/webdev, r/sysadmin, r/learnprogramming, etc.)
  * YouTube: Look for educational content (tutorials, courses, tech talks, professional development)
  * LinkedIn: Generally productive for professional networking
  * Discord/Slack: Could be work-related channels/teams
  * Twitter/X: Could be professional networking or industry news
- CRITICAL: ALWAYS flag potentially unproductive activities (social media, games, media consumption) as discrepancies
- If window titles suggest productive use:
  * STILL CREATE THE DISCREPANCY (don't skip it)
  * Use LOWER severity (e.g., if would be "medium", make it "low" if productive context found)
  * Note in description: "Potentially productive use - window titles suggest [educational/professional] content"
  * Note in context: "Window titles indicate work-related content, but usage should still be monitored"
- The goal is transparency - flag everything, but provide context about potential productive use

For each discrepancy:
- startTime: First occurrence start time
- endTime: Last occurrence end time  
- durationMinutes: TOTAL cumulative duration across ALL occurrences
- description: Include app name (or "Chrome/Firefox/Edge" if detected via window titles), total duration, list all time periods with individual durations (e.g., "09:00-10:00 (40 min), 14:00-15:00 (5 min)"), and mention window titles if relevant. If productive context is evident, mention it (e.g., "Reddit usage detected, but window titles suggest work-related subreddits (r/programming, r/webdev)"). If concentrated usage is detected, highlight it (e.g., "40 minutes of Spotify in a single hour (09:00-10:00) - concentrated usage")
- context: Additional context about the usage pattern. If productive use is possible, explicitly state: "Potentially productive use - window titles suggest [educational/professional] content" or "Could be productive - window titles indicate [specific productive context]". If concentrated usage detected, note: "Concentrated usage detected - [X] minutes in a single hour indicates significant distraction"
- severity: Based on BOTH total daily usage AND concentrated usage. Use the HIGHER severity:
  * If > 30 min in one hour = CRITICAL (regardless of total daily)
  * If > 20 min in one hour = HIGH (unless total daily would be critical)
  * Otherwise base on total daily: < 15 min = low, 15-60 min = medium, 60-120 min = high, > 120 min = critical
  * Reduce severity if productive context is evident, but concentrated usage should still be flagged appropriately

YOUR TASK - TEXT GENERATION:
1. Generate a "summary" string for the overallReport (2-3 sentences summarizing the period)
2. Generate a "conclusion" string for the overallReport (1-2 sentences with assessment)
3. Generate a "summary" string for each dailyReport (1-2 sentences per day)
4. Count totalDiscrepancies and criticalDiscrepancies in overallReport

RESPOND WITH ONLY JSON in this exact format:
{
  "overallReport": {
    "summary": "string here",
    "conclusion": "string here",
    "totalDiscrepancies": 0,
    "criticalDiscrepancies": 0
  },
  "dailyReports": [
    {
      "date": "YYYY-MM-DD",
      "summary": "string here",
      "notableDiscrepancies": [
        {
          "type": "extended_afk|social_media|media_consumption|excessive_idle|low_productivity_apps|suspicious_pattern",
          "severity": "low|medium|high|critical",
          "startTime": "HH:MM",
          "endTime": "HH:MM",
          "durationMinutes": 0,
          "description": "App name, total duration, and list all time periods (e.g., 'Reddit used for 18.2 minutes total across periods: 09:00-10:00 (2.2 min), 10:00-11:00 (11 min), 14:00-15:00 (5 min)')",
          "context": "Additional context about usage pattern (e.g., 'Used across 3 separate periods during core work hours')"
        }
      ]
    }
  ]
}

IMPORTANT: 
- Only include the fields above. Do not modify numeric data or structure.
- Be thorough in discrepancy detection - analyze ALL app usage data
- ALWAYS flag potentially unproductive activities (social media, games, media consumption) - never skip them
- If productive context is evident, still flag it but with lower severity and note the productive context
- Include specific app names in discrepancy descriptions
- Use exact times from the hourly breakdown data`, string(reportJSON), appUsageSummary, rawDataContext)

	// Call Gemini API
	model := s.config.Model
	if model == "" || strings.HasPrefix(model, "gpt-") {
		model = defaultGeminiModel
	}
	
	systemPrompt := "You are a productivity analyst. Generate concise, professional summaries and identify discrepancies in work activity data."
	responseJSON, err := callGeminiAPI(apiKey, model, systemPrompt, prompt, s.config.Temperature, 4000)
	if err != nil {
		return fmt.Errorf("Gemini API error: %w", err)
	}
	responseJSON = strings.TrimSpace(responseJSON)
	if strings.HasPrefix(responseJSON, "```json") {
		responseJSON = strings.TrimPrefix(responseJSON, "```json")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	} else if strings.HasPrefix(responseJSON, "```") {
		responseJSON = strings.TrimPrefix(responseJSON, "```")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	}

	var enhancement struct {
		OverallReport struct {
			Summary              string `json:"summary"`
			Conclusion           string `json:"conclusion"`
			TotalDiscrepancies   int    `json:"totalDiscrepancies"`
			CriticalDiscrepancies int   `json:"criticalDiscrepancies"`
		} `json:"overallReport"`
		DailyReports []struct {
			Date                string            `json:"date"`
			Summary             string            `json:"summary"`
			NotableDiscrepancies []models.Discrepancy `json:"notableDiscrepancies"`
		} `json:"dailyReports"`
	}

	if err := json.Unmarshal([]byte(responseJSON), &enhancement); err != nil {
		return fmt.Errorf("failed to parse AI enhancement: %w", err)
	}

	// Apply enhancements to user
	user.OverallReport.Summary = enhancement.OverallReport.Summary
	user.OverallReport.Conclusion = enhancement.OverallReport.Conclusion
	user.OverallReport.TotalDiscrepancies = enhancement.OverallReport.TotalDiscrepancies
	user.OverallReport.CriticalDiscrepancies = enhancement.OverallReport.CriticalDiscrepancies

	// Update daily reports
	for i := range user.DailyReports {
		daily := &user.DailyReports[i]
		
		// Find matching enhancement
		for _, enh := range enhancement.DailyReports {
			if enh.Date == daily.Date {
				daily.Summary = enh.Summary
				daily.NotableDiscrepancies = enh.NotableDiscrepancies
				break
			}
		}
	}

	return nil
}

// EnhanceWeeklyUserReportWithAI enhances a single user's weekly report with AI-generated text fields
func (s *AIService) EnhanceWeeklyUserReportWithAI(
	apiKey string,
	user *models.User,
	rawDataContext string,
	request models.GenerateWeeklyReportRequest,
	userName string,
	startDate, endDate string,
) error {
	// Create a temporary report structure with just this user for AI processing
	tempReport := &models.Report{
		Organizations: []models.Organization{
			{
				OrganizationName: request.Org,
				Users:            []models.User{*user},
			},
		},
		GeneratedAt: time.Now().UTC().Format(time.RFC3339),
		PeriodAnalyzed: models.Period{
			StartDate: startDate,
			EndDate:   endDate,
		},
	}

	// Convert report to JSON for context
	reportJSON, err := json.MarshalIndent(tempReport, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal report: %w", err)
	}

	// Extract app usage summary for this user
	appUsageSummary := s.extractAppUsageSummary(tempReport)

	// Build prompt for AI to fill in text fields - weekly report version
	prompt := fmt.Sprintf(`You are a productivity analyst detecting discrepancies and unproductive activity patterns in work monitoring data for WEEKLY ORGANIZATIONAL REPORTS.

THIS IS A WEEKLY REPORT that will be sent to organization management via email. Focus on weekly patterns, trends, and organizational insights.

EXISTING REPORT STRUCTURE (all numeric data is already calculated):
%s

APP USAGE SUMMARY (analyze this carefully for discrepancies):
%s

RAW DATA CONTEXT (for additional context):
%s

YOUR TASK - WEEKLY DISCREPANCY DETECTION:
Carefully analyze the app usage data above, paying special attention to WINDOW TITLES to detect unproductive use of productive apps. Focus on weekly patterns and trends.

CRITICAL: Create ONE discrepancy entry per unproductive activity that includes:
- ALL time periods where that activity occurred (list all periods from the data)
- TOTAL cumulative duration across all periods
- Analyze WINDOW TITLES to detect unproductive browsing in browsers (Chrome, Firefox, Edge, Safari)

SEVERITY JUDGMENT - TWO FACTORS:
1. TOTAL weekly usage: Judge based on total cumulative duration across all periods in the week
2. CONCENTRATED usage: Check if any single hour has > 30 minutes of unproductive activity
   - If an unproductive app takes up > 30 minutes in a single hour, this is CRITICAL severity
   - If an unproductive app takes up > 20 minutes in a single hour, this is HIGH severity
   - Concentrated usage is more problematic than spread-out usage
   - Example: 40 minutes of Spotify in one hour = CRITICAL, even if total weekly is only 40 minutes

Look for:

1. SOCIAL_MEDIA: 
   - Direct apps: Facebook, Twitter/X, Instagram, TikTok, LinkedIn (personal), Reddit, Discord, Slack (non-work), Snapchat, WhatsApp (personal)
   - Browser usage: ONLY flag Chrome/Firefox/Edge/Safari if window titles contain:
     * facebook.com, twitter.com, x.com, instagram.com, tiktok.com, reddit.com, linkedin.com (personal), discord.com, snapchat.com
     * Look for URLs, page titles mentioning social media
   - IMPORTANT: Check window titles for productive context:
     * Reddit: Work-related subreddits (r/programming, r/webdev, r/sysadmin, r/learnprogramming) = potentially productive
     * Twitter/X: Professional networking, industry news = potentially productive
     * LinkedIn: Generally productive unless clearly personal use
     * Discord/Slack: Work-related channels = productive
   - ALWAYS flag social media usage as a discrepancy
   - If productive context found: Flag with LOW severity and note in description/context about potential productive use
   - Aggregate ALL occurrences (both direct apps and browser usage) across the entire week
   - List ALL time periods with durations
   - Calculate TOTAL duration across all periods in the week
   - Check for CONCENTRATED usage: If any single hour has > 30 min of social media = CRITICAL
   - Base severity on BOTH total weekly usage AND concentrated usage:
     * Concentrated (> 30 min in one hour) = CRITICAL
     * Concentrated (> 20 min in one hour) = HIGH
     * Total weekly: < 60 min = low, 60-180 min = medium, 180-360 min = high, > 360 min = critical
   - Use the HIGHER severity from concentrated vs total weekly
   - Adjust severity down if productive context is evident (but still flag concentrated usage)

2. MEDIA_CONSUMPTION: 
   - Direct apps: YouTube, Netflix, Hulu, Spotify, gaming platforms (Steam, Epic Games, etc.), Twitch
   - Browser usage: ONLY flag Chrome/Firefox/Edge/Safari if window titles contain:
     * youtube.com, netflix.com, hulu.com, spotify.com, twitch.tv, steam, epic games
     * Look for video/streaming content URLs
   - IMPORTANT: Check window titles for productive context:
     * YouTube: Educational content (tutorials, courses, tech talks, professional development, documentation) = potentially productive
     * Spotify: Background music during work = generally acceptable (may skip if clearly background)
     * Twitch: Tech streams, coding streams = potentially productive
   - ALWAYS flag media consumption as a discrepancy (except possibly background music)
   - If productive context found: Flag with LOW severity and note in description/context about potential productive use
   - Same aggregation approach - one entry per activity type with all periods across the week
   - Check for CONCENTRATED usage: If any single hour has > 30 min of media consumption = CRITICAL
   - Base severity on BOTH total weekly usage AND concentrated usage:
     * Concentrated (> 30 min in one hour) = CRITICAL
     * Concentrated (> 20 min in one hour) = HIGH
     * Total weekly: < 60 min = low, 60-180 min = medium, 180-360 min = high, > 360 min = critical
   - Use the HIGHER severity from concentrated vs total weekly
   - Adjust for productive context (but still flag concentrated usage)

3. LOW_PRODUCTIVITY_APPS: 
   - Direct apps: Games, entertainment apps, shopping sites (Amazon, eBay for non-work), dating apps
   - Browser usage: ONLY flag Chrome/Firefox/Edge/Safari if window titles contain:
     * amazon.com, ebay.com, shopping sites, dating sites, gaming sites
     * Entertainment/news sites used during work hours
   - DO NOT flag browsers without unproductive window titles - browsers are productive tools
   - Aggregate all occurrences per activity type across the week
   - Consider total weekly usage when setting severity

4. EXTENDED_AFK: Already calculated in the report - flag periods > 30 min during work hours

5. SUSPICIOUS_PATTERN: 
   - Minimal app usage suggesting minimal actual work
   - Only activity at start/end of day
   - Patterns inconsistent with productive work
   - Weekly patterns (e.g., consistently low activity on certain days)

WINDOW TITLE ANALYSIS:
- When you see browsers (Chrome, Firefox, Edge, Safari) in the app list, ALWAYS check the window titles
- Window titles often contain URLs or page titles that reveal the actual content being viewed
- Examples: "Reddit - Dive into anything" indicates Reddit usage, "YouTube" indicates YouTube, "Facebook" indicates Facebook
- Look for domain names, site names, or content indicators in window titles
- CRITICAL: Browsers (Chrome, Firefox, Edge, Safari) should NOT be flagged as unproductive UNLESS window titles show unproductive content
- Browsers are productive tools when used for work - only flag them if window titles indicate social media, entertainment, shopping, etc.
- If window titles are missing or unclear, do NOT flag browser usage as unproductive
- If window titles show unproductive content, create a discrepancy for that specific content (e.g., "Chrome (Reddit usage detected via window titles)")

PRODUCTIVE USE CONTEXT:
- Some platforms can be used productively. Check window titles for context:
  * Reddit: Look for work-related subreddits (r/programming, r/webdev, r/sysadmin, r/learnprogramming, etc.)
  * YouTube: Look for educational content (tutorials, courses, tech talks, professional development)
  * LinkedIn: Generally productive for professional networking
  * Discord/Slack: Could be work-related channels/teams
  * Twitter/X: Could be professional networking or industry news
- CRITICAL: ALWAYS flag potentially unproductive activities (social media, games, media consumption) as discrepancies
- If window titles suggest productive use:
  * STILL CREATE THE DISCREPANCY (don't skip it)
  * Use LOWER severity (e.g., if would be "medium", make it "low" if productive context found)
  * Note in description: "Potentially productive use - window titles suggest [educational/professional] content"
  * Note in context: "Window titles indicate work-related content, but usage should still be monitored"
- The goal is transparency - flag everything, but provide context about potential productive use

For each discrepancy:
- startTime: First occurrence start time
- endTime: Last occurrence end time  
- durationMinutes: TOTAL cumulative duration across ALL occurrences in the week
- description: Include app name (or "Chrome/Firefox/Edge" if detected via window titles), total duration, list all time periods with individual durations (e.g., "09:00-10:00 (40 min), 14:00-15:00 (5 min)"), and mention window titles if relevant. If productive context is evident, mention it (e.g., "Reddit usage detected, but window titles suggest work-related subreddits (r/programming, r/webdev)"). If concentrated usage is detected, highlight it (e.g., "40 minutes of Spotify in a single hour (09:00-10:00) - concentrated usage")
- context: Additional context about the usage pattern. If productive use is possible, explicitly state: "Potentially productive use - window titles suggest [educational/professional] content" or "Could be productive - window titles indicate [specific productive context]". If concentrated usage detected, note: "Concentrated usage detected - [X] minutes in a single hour indicates significant distraction"
- severity: Based on BOTH total weekly usage AND concentrated usage. Use the HIGHER severity:
  * If > 30 min in one hour = CRITICAL (regardless of total weekly)
  * If > 20 min in one hour = HIGH (unless total weekly would be critical)
  * Otherwise base on total weekly: < 60 min = low, 60-180 min = medium, 180-360 min = high, > 360 min = critical
  * Reduce severity if productive context is evident, but concentrated usage should still be flagged appropriately

YOUR TASK - WEEKLY TEXT GENERATION:
1. Generate a "summary" string for the overallReport (3-5 sentences summarizing the WEEK, focusing on weekly patterns, trends, and day-of-week patterns)
2. Generate a "conclusion" string for the overallReport (2-3 sentences with weekly assessment - MUST be critical if weekly working time is insufficient. Typical full-time expectation: 30-40 hours active time per week)
3. Generate a "summary" string for each dailyReport (1-2 sentences per day, noting day of week)
4. Count totalDiscrepancies and criticalDiscrepancies in overallReport across the entire week

RESPOND WITH ONLY JSON in this exact format:
{
  "overallReport": {
    "summary": "string here (3-5 sentences focusing on weekly patterns)",
    "conclusion": "string here (2-3 sentences with weekly assessment)",
    "totalDiscrepancies": 0,
    "criticalDiscrepancies": 0
  },
  "dailyReports": [
    {
      "date": "YYYY-MM-DD",
      "summary": "string here (1-2 sentences, note day of week)",
      "notableDiscrepancies": [
        {
          "type": "extended_afk|social_media|media_consumption|excessive_idle|low_productivity_apps|suspicious_pattern",
          "severity": "low|medium|high|critical",
          "startTime": "HH:MM",
          "endTime": "HH:MM",
          "durationMinutes": 0,
          "description": "App name, total duration, and list all time periods (e.g., 'Reddit used for 18.2 minutes total across periods: 09:00-10:00 (2.2 min), 10:00-11:00 (11 min), 14:00-15:00 (5 min)')",
          "context": "Additional context about usage pattern (e.g., 'Used across 3 separate periods during core work hours')"
        }
      ]
    }
  ]
}

IMPORTANT: 
- Only include the fields above. Do not modify numeric data or structure.
- Be thorough in discrepancy detection - analyze ALL app usage data across the entire week
- ALWAYS flag potentially unproductive activities (social media, games, media consumption) - never skip them
- If productive context is evident, still flag it but with lower severity and note the productive context
- Include specific app names in discrepancy descriptions
- Use exact times from the hourly breakdown data
- Focus on WEEKLY patterns and trends suitable for organizational email reports`, string(reportJSON), appUsageSummary, rawDataContext)

	// Call Gemini API
	model := s.config.Model
	if model == "" || strings.HasPrefix(model, "gpt-") {
		model = defaultGeminiModel
	}
	
	systemPrompt := "You are a productivity analyst generating weekly organizational reports. Generate concise, professional summaries and identify discrepancies in work activity data for weekly email delivery to organization management."
	responseJSON, err := callGeminiAPI(apiKey, model, systemPrompt, prompt, s.config.Temperature, 4000)
	if err != nil {
		return fmt.Errorf("Gemini API error: %w", err)
	}
	responseJSON = strings.TrimSpace(responseJSON)
	if strings.HasPrefix(responseJSON, "```json") {
		responseJSON = strings.TrimPrefix(responseJSON, "```json")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	} else if strings.HasPrefix(responseJSON, "```") {
		responseJSON = strings.TrimPrefix(responseJSON, "```")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	}

	var enhancement struct {
		OverallReport struct {
			Summary              string `json:"summary"`
			Conclusion           string `json:"conclusion"`
			TotalDiscrepancies   int    `json:"totalDiscrepancies"`
			CriticalDiscrepancies int   `json:"criticalDiscrepancies"`
		} `json:"overallReport"`
		DailyReports []struct {
			Date                string            `json:"date"`
			Summary             string            `json:"summary"`
			NotableDiscrepancies []models.Discrepancy `json:"notableDiscrepancies"`
		} `json:"dailyReports"`
	}

	if err := json.Unmarshal([]byte(responseJSON), &enhancement); err != nil {
		return fmt.Errorf("failed to parse AI enhancement: %w", err)
	}

	// Apply enhancements to user
	user.OverallReport.Summary = enhancement.OverallReport.Summary
	user.OverallReport.Conclusion = enhancement.OverallReport.Conclusion
	user.OverallReport.TotalDiscrepancies = enhancement.OverallReport.TotalDiscrepancies
	user.OverallReport.CriticalDiscrepancies = enhancement.OverallReport.CriticalDiscrepancies

	// Update daily reports
	for i := range user.DailyReports {
		daily := &user.DailyReports[i]
		
		// Find matching enhancement
		for _, enh := range enhancement.DailyReports {
			if enh.Date == daily.Date {
				daily.Summary = enh.Summary
				daily.NotableDiscrepancies = enh.NotableDiscrepancies
				break
			}
		}
	}

	return nil
}

// EnhanceWeeklyOrganizationReportWithAI enhances the weekly report with organization-level summaries and conclusions
func (s *AIService) EnhanceWeeklyOrganizationReportWithAI(
	apiKey string,
	report *models.Report,
	rawDataContext string,
	request models.GenerateWeeklyReportRequest,
	startDate, endDate string,
) error {
	if report == nil || len(report.Organizations) == 0 {
		return fmt.Errorf("invalid report structure")
	}

	org := &report.Organizations[0]

	// Convert report to JSON for context
	reportJSON, err := json.MarshalIndent(report, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal report: %w", err)
	}

	// Extract organization-level app usage summary
	appUsageSummary := s.extractAppUsageSummary(report)

	// Calculate organization-level aggregates
	var totalActiveHours, totalAfkHours float64
	var totalDiscrepancies, criticalDiscrepancies int
	var userCount int

	for _, user := range org.Users {
		totalActiveHours += user.OverallReport.TotalActiveHours
		totalAfkHours += user.OverallReport.TotalAfkHours
		totalDiscrepancies += user.OverallReport.TotalDiscrepancies
		criticalDiscrepancies += user.OverallReport.CriticalDiscrepancies
		userCount++
	}

	avgActiveHoursPerUser := totalActiveHours / float64(userCount)
	avgDailyActiveHoursPerUser := avgActiveHoursPerUser / 7.0 // 7 days in a week

	// Build prompt for AI to generate organization-level summaries
	prompt := fmt.Sprintf(`You are a productivity analyst generating WEEKLY ORGANIZATIONAL REPORTS for email delivery to organization management.

THIS IS AN ORGANIZATION-LEVEL WEEKLY REPORT that aggregates data across all users in the organization.

EXISTING REPORT STRUCTURE (all numeric data is already calculated):
%s

ORGANIZATION-LEVEL AGGREGATES:
- Total Active Hours (across all users): %.2f hours
- Total AFK Hours (across all users): %.2f hours
- Average Active Hours per User: %.2f hours
- Average Daily Active Hours per User: %.2f hours
- Total Discrepancies (across all users): %d
- Total Critical Discrepancies (across all users): %d
- Number of Users: %d

APP USAGE SUMMARY (analyze this carefully for organization-level patterns):
%s

RAW DATA CONTEXT (aggregated across all users):
%s

YOUR TASK - ORGANIZATION-LEVEL WEEKLY ANALYSIS:

Generate organization-level summaries and conclusions for the WEEKLY REPORT. This report will be emailed to organization management, so focus on:

1. ORGANIZATION-LEVEL SUMMARY (4-6 sentences):
   - Overall weekly productivity trends across the organization
   - Aggregate activity patterns and work habits
   - Day-of-week patterns (e.g., Monday vs Friday performance)
   - Comparison to expected weekly working hours (typically 30-40 hours per user per week)
   - Notable organizational patterns or concerns
   - High-level insights suitable for management review

2. ORGANIZATION-LEVEL CONCLUSION (2-3 sentences):
   - MUST be critical if organization-wide working time is insufficient
   - Evaluate if organization meets expected productivity requirements
   - Typical full-time work expectation: 30-40 hours active time per user per week
   - If average daily active hours per user < 6 hours, conclusion MUST be critical
   - If total weekly active hours per user < 30 hours, conclusion MUST be critical
   - If significant discrepancies detected across the organization, conclusion should reflect severity
   - Provide clear assessment of organizational productivity and working time adequacy
   - Make it actionable for organizational oversight

3. UPDATE USER SUMMARIES (for each user, generate 2-3 sentences):
   - Focus on their contribution to the organization's weekly performance
   - Note any patterns or concerns specific to that user
   - Keep it concise but informative

4. UPDATE USER CONCLUSIONS (for each user, 1-2 sentences):
   - Brief assessment of their weekly performance
   - Note if they meet expected working hours
   - Keep it concise

IMPORTANT:
- This is an ORGANIZATION-LEVEL report, so emphasize organizational patterns and trends
- Focus on aggregate metrics and organizational health
- Make insights suitable for email delivery to management
- Be professional and actionable
- Still include user-level data but with organization-focused context

RESPOND WITH ONLY JSON in this exact format:
{
  "organizationSummary": "Organization-level weekly summary (4-6 sentences focusing on organizational patterns and trends)",
  "organizationConclusion": "Organization-level weekly conclusion (2-3 sentences with organizational assessment)",
  "users": [
    {
      "userName": "user1",
      "summary": "User-level summary (2-3 sentences focusing on their contribution to org performance)",
      "conclusion": "User-level conclusion (1-2 sentences with brief assessment)"
    },
    {
      "userName": "user2",
      "summary": "User-level summary (2-3 sentences focusing on their contribution to org performance)",
      "conclusion": "User-level conclusion (1-2 sentences with brief assessment)"
    }
  ]
}

IMPORTANT: 
- Only include the fields above. Do not modify numeric data or structure.
- Focus on ORGANIZATION-LEVEL insights suitable for management email delivery
- Be thorough in organizational pattern analysis
- Make conclusions actionable for organizational oversight`, 
		string(reportJSON),
		totalActiveHours,
		totalAfkHours,
		avgActiveHoursPerUser,
		avgDailyActiveHoursPerUser,
		totalDiscrepancies,
		criticalDiscrepancies,
		userCount,
		appUsageSummary,
		rawDataContext)

	// Call Gemini API
	model := s.config.Model
	if model == "" || strings.HasPrefix(model, "gpt-") {
		model = defaultGeminiModel
	}
	
	systemPrompt := "You are a productivity analyst generating weekly organizational reports for email delivery to organization management. Generate concise, professional organization-level summaries and insights."
	responseJSON, err := callGeminiAPI(apiKey, model, systemPrompt, prompt, s.config.Temperature, 4000)
	if err != nil {
		return fmt.Errorf("Gemini API error: %w", err)
	}
	responseJSON = strings.TrimSpace(responseJSON)
	if strings.HasPrefix(responseJSON, "```json") {
		responseJSON = strings.TrimPrefix(responseJSON, "```json")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	} else if strings.HasPrefix(responseJSON, "```") {
		responseJSON = strings.TrimPrefix(responseJSON, "```")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	}

	var enhancement struct {
		OrganizationSummary   string `json:"organizationSummary"`
		OrganizationConclusion string `json:"organizationConclusion"`
		Users                 []struct {
			UserName   string `json:"userName"`
			Summary    string `json:"summary"`
			Conclusion string `json:"conclusion"`
		} `json:"users"`
	}

	if err := json.Unmarshal([]byte(responseJSON), &enhancement); err != nil {
		return fmt.Errorf("failed to parse AI enhancement: %w", err)
	}

	// Update individual user summaries and conclusions from AI response
	userMap := make(map[string]*models.User)
	for i := range org.Users {
		userMap[org.Users[i].UserName] = &org.Users[i]
	}

	for _, enh := range enhancement.Users {
		if user, ok := userMap[enh.UserName]; ok {
			user.OverallReport.Summary = enh.Summary
			user.OverallReport.Conclusion = enh.Conclusion
		}
	}

	// Store organization-level summary in UserRanking summary (if rankings exist)
	// This makes it accessible at the organization level
	if org.UserRanking != nil {
		if enhancement.OrganizationSummary != "" {
			org.UserRanking.Summary = enhancement.OrganizationSummary
		}
	} else if len(org.Users) > 0 {
		// If no rankings, store org summary in first user's summary as fallback
		// Note: This is a workaround - ideally we'd add org-level fields to Organization model
		org.Users[0].OverallReport.Summary = enhancement.OrganizationSummary
		org.Users[0].OverallReport.Conclusion = enhancement.OrganizationConclusion
	}

	return nil
}

// EnhanceWeeklyReportSummaries enhances weekly report with organization summary and user summaries
func (s *AIService) EnhanceWeeklyReportSummaries(
	apiKey string,
	report *models.Report,
	weeklySummary *models.WeeklyOrganizationSummary,
	userSummaries []models.WeeklyUserSummary,
	rawDataContext string,
	request models.GenerateWeeklyReportRequest,
	startDate, endDate string,
) error {
	if report == nil || len(report.Organizations) == 0 {
		return fmt.Errorf("invalid report structure")
	}

	// Extract app usage summary for organization-level analysis
	appUsageSummary := s.extractAppUsageSummary(report)

	// Build context with user summaries
	var userContext strings.Builder
	userContext.WriteString("USER SUMMARIES:\n\n")
	for _, summary := range userSummaries {
		userContext.WriteString(fmt.Sprintf("User: %s\n", summary.UserName))
		userContext.WriteString(fmt.Sprintf("  Active Hours: %.2f\n", summary.ActiveHours))
		userContext.WriteString(fmt.Sprintf("  Activity Ratio: %.2f%%\n", summary.ActivityRatio))
		userContext.WriteString(fmt.Sprintf("  Total Discrepancies: %d\n", summary.TotalDiscrepancies))
		userContext.WriteString(fmt.Sprintf("  Critical Discrepancies: %d\n", summary.CriticalDiscrepancies))
		userContext.WriteString(fmt.Sprintf("  Distracted Time: %.2f hours\n", summary.DistractedTimeHours))
		userContext.WriteString("\n")
	}

	// Build prompt for AI with dynamic top/bottom labels
	topCount := len(weeklySummary.Top5Employees)
	bottomCount := len(weeklySummary.Bottom5Employees)
	
	prompt := fmt.Sprintf(`You are a productivity analyst generating WEEKLY ORGANIZATIONAL REPORTS for email delivery to organization management.

THIS IS AN ORGANIZATION-LEVEL WEEKLY REPORT that aggregates data across all users in the organization.

PERIOD: %s to %s
ORGANIZATION: %s (ID: %d)

USER SUMMARIES:
%s

TOP %d EMPLOYEES:
%s

BOTTOM %d EMPLOYEES:
%s

APP USAGE SUMMARY (analyze this carefully for unproductive activities):
%s

RAW DATA CONTEXT (for additional context):
%s

YOUR TASK:

1. ORGANIZATION PRODUCTIVITY SUMMARY (3-4 sentences):
   - Overall weekly productivity trends across the organization
   - Aggregate activity patterns and work habits
   - Day-of-week patterns if notable
   - Comparison to expected weekly working hours (typically 30-40 hours per user per week)
   - High-level insights suitable for management review
   - Keep it concise and actionable

2. USER PRODUCTIVITY SUMMARIES (for each user, 1-2 sentences MAX):
   - Very short summary highlighting the TOP 3 unproductive things/apps used
   - Focus on the most significant distractions
   - Be specific: mention app names or activity types
   - Example: "Top distractions: Reddit (45 min), YouTube (30 min), Facebook (20 min)"
   - If no significant distractions, note that productivity was good
   - Keep it extremely concise - organizations can be very large

IMPORTANT:
- This is an ORGANIZATION-LEVEL report for email delivery to management
- User summaries must be VERY SHORT (1-2 sentences max)
- Focus on actionable insights
- Be specific about unproductive apps/activities
- Highlight top 3 distractions per user where possible

RESPOND WITH ONLY JSON in this exact format:
{
  "organizationProductivitySummary": "Organization-level weekly productivity summary (3-4 sentences)",
  "userSummaries": [
    {
      "userName": "user1",
      "productivitySummary": "Very short summary (1-2 sentences) highlighting top 3 unproductive things"
    },
    {
      "userName": "user2",
      "productivitySummary": "Very short summary (1-2 sentences) highlighting top 3 unproductive things"
    }
  ]
}`, 
		startDate,
		endDate,
		request.Org,
		request.OrgID,
		userContext.String(),
		topCount,
		s.formatWeeklyRanks(weeklySummary.Top5Employees),
		bottomCount,
		s.formatWeeklyRanks(weeklySummary.Bottom5Employees),
		appUsageSummary,
		rawDataContext)

	// Call Gemini API
	model := s.config.Model
	if model == "" || strings.HasPrefix(model, "gpt-") {
		model = defaultGeminiModel
	}
	
	systemPrompt := "You are a productivity analyst generating weekly organizational reports for email delivery to organization management. Generate concise, professional summaries focusing on actionable insights."
	responseJSON, err := callGeminiAPI(apiKey, model, systemPrompt, prompt, s.config.Temperature, 3000)
	if err != nil {
		return fmt.Errorf("Gemini API error: %w", err)
	}
	responseJSON = strings.TrimSpace(responseJSON)
	if strings.HasPrefix(responseJSON, "```json") {
		responseJSON = strings.TrimPrefix(responseJSON, "```json")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	} else if strings.HasPrefix(responseJSON, "```") {
		responseJSON = strings.TrimPrefix(responseJSON, "```")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	}

	var enhancement struct {
		OrganizationProductivitySummary string `json:"organizationProductivitySummary"`
		UserSummaries                   []struct {
			UserName            string `json:"userName"`
			ProductivitySummary  string `json:"productivitySummary"`
		} `json:"userSummaries"`
	}

	if err := json.Unmarshal([]byte(responseJSON), &enhancement); err != nil {
		return fmt.Errorf("failed to parse AI enhancement: %w", err)
	}

	// Apply enhancements
	weeklySummary.ProductivitySummary = enhancement.OrganizationProductivitySummary

	// Map user summaries
	userSummaryMap := make(map[string]string)
	for _, us := range enhancement.UserSummaries {
		userSummaryMap[us.UserName] = us.ProductivitySummary
	}

	// Update user summaries (they're passed by reference, so this will update the original)
	for i := range userSummaries {
		if summary, ok := userSummaryMap[userSummaries[i].UserName]; ok {
			userSummaries[i].ProductivitySummary = summary
		}
	}

	// Also update the report's organization weekly user summaries
	if len(report.Organizations) > 0 && len(report.Organizations[0].WeeklyUserSummaries) > 0 {
		for i := range report.Organizations[0].WeeklyUserSummaries {
			if summary, ok := userSummaryMap[report.Organizations[0].WeeklyUserSummaries[i].UserName]; ok {
				report.Organizations[0].WeeklyUserSummaries[i].ProductivitySummary = summary
			}
		}
	}

	return nil
}

// formatWeeklyRanks formats weekly ranks for display in prompt
func (s *AIService) formatWeeklyRanks(ranks []models.WeeklyUserRank) string {
	if len(ranks) == 0 {
		return "None"
	}
	
	var builder strings.Builder
	for i, rank := range ranks {
		builder.WriteString(fmt.Sprintf("%d. %s - Activity Ratio: %.2f%%, Active Hours: %.2f, Discrepancies: %d\n",
			i+1, rank.UserName, rank.ActivityRatio, rank.ActiveHours, rank.TotalDiscrepancies))
	}
	return builder.String()
}

// extractAppUsageSummary extracts app usage data from the report for discrepancy analysis
func (s *AIService) extractAppUsageSummary(report *models.Report) string {
	if report == nil || len(report.Organizations) == 0 {
		return "No app usage data available."
	}

	var summary strings.Builder
	summary.WriteString("APP USAGE ANALYSIS DATA:\n\n")

	for _, org := range report.Organizations {
		for _, user := range org.Users {
			// Format summary - group by app with all time periods and window titles
			for _, daily := range user.DailyReports {
				date := daily.Date
				summary.WriteString(fmt.Sprintf("DATE: %s\n", date))

			// Group apps with all their time periods and window titles
			type appUsageDetail struct {
				totalMinutes float64
				periods      []string // Format: "HH:MM-HH:MM (X.X min)"
				windowTitles map[string]bool // Track unique window titles
			}
			appDetails := make(map[string]*appUsageDetail)

			// Collect all periods and window titles for each app
			for _, hourly := range daily.HourlyBreakdown {
				hour := hourly.Hour
				startTime, endTime := utils.GenerateHourRange(hour)
				
				for _, app := range hourly.AppUsage {
					if app.AppName != "" && app.DurationMinutes > 0 {
						if appDetails[app.AppName] == nil {
							appDetails[app.AppName] = &appUsageDetail{
								totalMinutes: 0,
								periods:      []string{},
								windowTitles: make(map[string]bool),
							}
						}
						appDetails[app.AppName].totalMinutes += app.DurationMinutes
						appDetails[app.AppName].periods = append(appDetails[app.AppName].periods, 
							fmt.Sprintf("%s-%s (%.1f min)", startTime, endTime, app.DurationMinutes))
						
						// Collect window titles
						for _, title := range app.WindowTitles {
							if title != "" {
								appDetails[app.AppName].windowTitles[title] = true
							}
						}
					}
				}
			}

				// Sort apps by total duration
				type appSummary struct {
					name        string
					total       float64
					periods     []string
					windowTitles []string
				}
				var sortedApps []appSummary
				for name, details := range appDetails {
					// Convert window titles map to sorted slice
					titles := make([]string, 0, len(details.windowTitles))
					for title := range details.windowTitles {
						titles = append(titles, title)
					}
					// Simple sort titles
					for i := 0; i < len(titles)-1; i++ {
						for j := i + 1; j < len(titles); j++ {
							if titles[i] > titles[j] {
								titles[i], titles[j] = titles[j], titles[i]
							}
						}
					}
					
					sortedApps = append(sortedApps, appSummary{
						name:        name,
						total:       details.totalMinutes,
						periods:     details.periods,
						windowTitles: titles,
					})
				}
				// Simple sort by total duration
				for i := 0; i < len(sortedApps)-1; i++ {
					for j := i + 1; j < len(sortedApps); j++ {
						if sortedApps[i].total < sortedApps[j].total {
							sortedApps[i], sortedApps[j] = sortedApps[j], sortedApps[i]
						}
					}
				}

				// Format output
				summary.WriteString("  App Usage (grouped by app with all time periods and window titles):\n")
				for _, app := range sortedApps {
					summary.WriteString(fmt.Sprintf("    %s:\n", app.name))
					summary.WriteString(fmt.Sprintf("      Total: %.2f minutes (%.2f hours)\n", app.total, app.total/60))
					summary.WriteString(fmt.Sprintf("      Time Periods: %s\n", strings.Join(app.periods, ", ")))
					if len(app.windowTitles) > 0 {
						summary.WriteString(fmt.Sprintf("      Window Titles: %s\n", strings.Join(app.windowTitles, ", ")))
					}
				}
				summary.WriteString("\n")
			}
		}
	}

	return summary.String()
}

// EnhanceRankingsWithAI enhances user rankings with AI-generated insights and summary
func (s *AIService) EnhanceRankingsWithAI(
	apiKey string,
	ranking *models.UserRanking,
	users []models.User,
	request models.GenerateReportRequest,
) error {
	if ranking == nil || len(ranking.Rankings) == 0 {
		return nil
	}

	// Build context with all user data
	var userContext strings.Builder
	userContext.WriteString("USER RANKINGS AND METRICS:\n\n")
	for _, rank := range ranking.Rankings {
		// Find the full user data
		var fullUser *models.User
		for i := range users {
			if users[i].UserName == rank.UserName {
				fullUser = &users[i]
				break
			}
		}

		if fullUser != nil {
			userContext.WriteString(fmt.Sprintf("User: %s (Rank #%d)\n", rank.UserName, rank.Rank))
			userContext.WriteString(fmt.Sprintf("  Total Active Hours: %.2f\n", rank.TotalActiveHours))
			userContext.WriteString(fmt.Sprintf("  Average Daily Active Hours: %.2f\n", rank.AverageDailyActiveHours))
			userContext.WriteString(fmt.Sprintf("  Total AFK Hours: %.2f\n", rank.TotalAfkHours))
			userContext.WriteString(fmt.Sprintf("  Active Percentage: %.2f%%\n", rank.ActivePercentage))
			userContext.WriteString(fmt.Sprintf("  Total Discrepancies: %d\n", rank.TotalDiscrepancies))
			userContext.WriteString(fmt.Sprintf("  Critical Discrepancies: %d\n", rank.CriticalDiscrepancies))
			userContext.WriteString(fmt.Sprintf("  Summary: %s\n", fullUser.OverallReport.Summary))
			userContext.WriteString("\n")
		}
	}

	// Build prompt for AI
	prompt := fmt.Sprintf(`You are a productivity analyst providing comparative insights on user rankings.

RANKING DATA:
%s

PERIOD ANALYZED: %s to %s

YOUR TASK:
1. Generate a comprehensive summary (2-3 paragraphs) comparing all users' performance, highlighting:
   - Overall productivity trends
   - Key differences between top and bottom performers
   - Notable patterns or insights
   - Areas for improvement

2. For each user in the rankings, provide brief insights (1-2 sentences) explaining:
   - Why they achieved their rank
   - Key strengths or weaknesses
   - Notable patterns in their data

Return ONLY a JSON object with this structure:
{
  "summary": "Overall comparison summary (2-3 paragraphs)",
  "rankings": [
    {
      "userName": "user1",
      "insights": "Brief insights about this user's ranking (1-2 sentences)"
    },
    {
      "userName": "user2",
      "insights": "Brief insights about this user's ranking (1-2 sentences)"
    }
  ]
}

IMPORTANT:
- Be constructive and professional
- Focus on actionable insights
- Compare users fairly
- Highlight both strengths and areas for improvement`, userContext.String(), request.StartDate, request.EndDate)

	// Call OpenAI
	// Call Gemini API
	model := s.config.Model
	if model == "" || strings.HasPrefix(model, "gpt-") {
		model = defaultGeminiModel
	}
	
	systemPrompt := "You are a productivity analyst providing comparative insights on user performance rankings. Always return valid JSON."
	responseJSON, err := callGeminiAPI(apiKey, model, systemPrompt, prompt, s.config.Temperature, 2000)
	if err != nil {
		return fmt.Errorf("Gemini API error: %w", err)
	}

	// Clean up the JSON - remove markdown code fences if present
	responseJSON = strings.TrimSpace(responseJSON)
	if strings.HasPrefix(responseJSON, "```json") {
		responseJSON = strings.TrimPrefix(responseJSON, "```json")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	} else if strings.HasPrefix(responseJSON, "```") {
		responseJSON = strings.TrimPrefix(responseJSON, "```")
		responseJSON = strings.TrimSuffix(responseJSON, "```")
		responseJSON = strings.TrimSpace(responseJSON)
	}

	// Parse JSON response
	var aiResponse struct {
		Summary  string `json:"summary"`
		Rankings []struct {
			UserName string `json:"userName"`
			Insights string `json:"insights"`
		} `json:"rankings"`
	}

	if err := json.Unmarshal([]byte(responseJSON), &aiResponse); err != nil {
		return fmt.Errorf("failed to parse AI response: %w", err)
	}

	// Update ranking with AI-generated content
	ranking.Summary = aiResponse.Summary

	// Map insights to ranking entries
	insightsMap := make(map[string]string)
	for _, r := range aiResponse.Rankings {
		insightsMap[r.UserName] = r.Insights
	}

	for i := range ranking.Rankings {
		if insights, ok := insightsMap[ranking.Rankings[i].UserName]; ok {
			ranking.Rankings[i].Insights = insights
		}
	}

	return nil
}

func (s *AIService) buildSystemPrompt() (string, error) {
	promptData, err := os.ReadFile("prompts/system_prompt.txt")
	if err != nil {
		return "", fmt.Errorf("failed to read system prompt: %w", err)
	}
	return string(promptData), nil
}

// buildWeeklySystemPrompt builds the weekly system prompt
func (s *AIService) buildWeeklySystemPrompt() (string, error) {
	promptData, err := os.ReadFile("prompts/weekly_system_prompt.txt")
	if err != nil {
		return "", fmt.Errorf("failed to read weekly system prompt: %w", err)
	}
	return string(promptData), nil
}

// buildUserPrompt builds the user prompt with data context
func (s *AIService) buildUserPrompt(dataContext string, request models.GenerateReportRequest) string {
	// Format users as a readable string
	userStrings := make([]string, len(request.Users))
	for i, user := range request.Users {
		userStrings[i] = fmt.Sprintf("%s (ID: %d)", user.Name, user.ID)
	}
	usersStr := fmt.Sprintf("[%s]", fmt.Sprintf("%v", userStrings))
	
	// Get first user name for example
	firstUserName := ""
	if len(request.Users) > 0 {
		firstUserName = request.Users[0].Name
	}
	
	return fmt.Sprintf(`Generate a productivity report for the following request:

ORGANIZATION: %s (ID: %d)
USERS: %s
PERIOD: %s to %s

%s

CRITICAL REQUIREMENTS - YOU MUST USE THESE EXACT FIELD NAMES:
1. The "organizations" array MUST contain at least one organization object
2. Each organization MUST contain a "users" array with at least one user object
3. Each user MUST have an "overallReport" object with these EXACT fields:
   - periodStart (string, YYYY-MM-DD)
   - periodEnd (string, YYYY-MM-DD)
   - totalActiveHours (number)
   - totalActiveMinutes (number)
   - totalAfkHours (number)
   - totalAfkMinutes (number)
   - averageDailyActiveHours (number)
   - averageDailyActiveMinutes (number)
   - totalDiscrepancies (integer)
   - criticalDiscrepancies (integer)
   - summary (string)
   - conclusion (string) - MUST be a string, not an object
4. Each user MUST have a "dailyReports" array with objects containing these EXACT fields:
   - date (string, YYYY-MM-DD)
   - hourlyBreakdown (array of 24 hour objects)
   - totalActiveMinutes (number) - NOT "dailyTotals.activeMinutes"
   - totalActiveHours (number) - NOT "dailyTotals.activeHours"
   - totalAfkMinutes (number) - NOT "dailyTotals.afkMinutes"
   - totalAfkHours (number) - NOT "dailyTotals.afkHours"
   - notableDiscrepancies (array) - NOT "discrepancies"
   - summary (string)
5. Each hourlyBreakdown object MUST have:
   - hour (integer 0-23)
   - startTime (string HH:MM)
   - endTime (string HH:MM)
   - activeMinutes (number)
   - afkMinutes (number)
   - appUsage (array)
   - totalMinutes (integer, always 60)
6. Every hour (0-23) MUST be represented for each day in the period
7. Respond with ONLY valid JSON matching the required structure - no markdown, no code fences, no explanatory text

EXAMPLE STRUCTURE (use this exact format):
{
  "organizations": [
    {
      "organizationName": "%s",
      "users": [
        {
          "userName": "%s",
          "overallReport": {
            "periodStart": "%s",
            "periodEnd": "%s",
            "totalActiveHours": 0,
            "totalActiveMinutes": 0,
            "totalAfkHours": 0,
            "totalAfkMinutes": 0,
            "averageDailyActiveHours": 0,
            "averageDailyActiveMinutes": 0,
            "totalDiscrepancies": 0,
            "criticalDiscrepancies": 0,
            "summary": "string here",
            "conclusion": "string here"
          },
          "dailyReports": [
            {
              "date": "%s",
              "hourlyBreakdown": [
                {
                  "hour": 0,
                  "startTime": "00:00",
                  "endTime": "01:00",
                  "activeMinutes": 0,
                  "afkMinutes": 0,
                  "appUsage": [],
                  "totalMinutes": 60
                }
              ],
              "totalActiveMinutes": 0,
              "totalActiveHours": 0,
              "totalAfkMinutes": 0,
              "totalAfkHours": 0,
              "notableDiscrepancies": [],
              "summary": "string here"
            }
          ]
        }
      ]
    }
  ],
  "generatedAt": "2025-11-25T00:00:00Z",
  "periodAnalyzed": {
    "startDate": "%s",
    "endDate": "%s"
  }
}`, 
		request.Org,
		request.OrgID,
		usersStr,
		request.StartDate, 
		request.EndDate,
		dataContext,
		request.Org,
		firstUserName,
		request.StartDate,
		request.EndDate,
		request.StartDate,
		request.StartDate,
		request.EndDate)
}

