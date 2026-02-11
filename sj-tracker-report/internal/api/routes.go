package api

import (
	"github.com/gin-gonic/gin"
)

// SetupRoutes configures all API routes
func SetupRoutes(handlers *Handlers) *gin.Engine {
	router := gin.Default()

	// Add CORS middleware
	router.Use(corsMiddleware())

	// API routes
	api := router.Group("/api")
	{
		// Chat agent endpoints (no auth required for local use)
		chat := api.Group("/chat")
		{
			chat.GET("/tools", handlers.ListToolsHandler)
			chat.POST("/tools/execute", handlers.ExecuteToolHandler)
		}

		// Report routes (no auth required for open-source local version)
		reports := api.Group("/reports")
		{
			reports.POST("/generate", handlers.GenerateReportHandler)
			reports.POST("/generate-sync", handlers.GenerateReportSyncHandler)
			reports.POST("/generate-weekly", handlers.GenerateWeeklyReportHandler)
			reports.POST("/generate-weekly-sync", handlers.GenerateWeeklyReportSyncHandler)
			reports.GET("/status/:taskId", handlers.GetTaskStatusHandler)
			
			// Weekly email opt-in/opt-out
			weekly := reports.Group("/weekly")
			{
				weekly.POST("/opt-in", handlers.OptInWeeklyReportsHandler)
				weekly.POST("/opt-out", handlers.OptOutWeeklyReportsHandler)
				weekly.POST("/send-email", handlers.SendWeeklyReportEmailHandler)
				weekly.GET("/opted-in/:accountId", handlers.GetOptedInAccountsHandler)
			}
		}

		// Timeline routes (no auth required for open-source local version)
		timeline := api.Group("/timeline")
		{
			timeline.GET("", handlers.GetTimelineHandler)
		}

		// Audio transcript routes - non-admins can access but only their own data (checked in handler)
		audioTranscripts := api.Group("/audio-transcripts")
		{
			audioTranscripts.GET("", handlers.GetAudioTranscriptsHandler)
		}

		// Audio file serving route - serves local audio files
		api.GET("/audio-file", handlers.ServeAudioFileHandler)
	}

	// Health check endpoint
	router.GET("/health", func(c *gin.Context) {
		c.JSON(200, gin.H{"status": "ok"})
	})

	return router
}

// corsMiddleware adds CORS headers
func corsMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Writer.Header().Set("Access-Control-Allow-Origin", "*")
		c.Writer.Header().Set("Access-Control-Allow-Credentials", "true")
		c.Writer.Header().Set("Access-Control-Allow-Headers", "Content-Type, Content-Length, Accept-Encoding, X-CSRF-Token, Authorization, accept, origin, Cache-Control, X-Requested-With")
		c.Writer.Header().Set("Access-Control-Allow-Methods", "POST, OPTIONS, GET, PUT, DELETE")

		if c.Request.Method == "OPTIONS" {
			c.AbortWithStatus(204)
			return
		}

		c.Next()
	}
}

