package main

import (
	"log"
	"sj-tracker-report/internal/api"
	"sj-tracker-report/internal/config"
	"sj-tracker-report/internal/database"
	"sj-tracker-report/internal/services"
)

func main() {
	// Load configuration
	cfg, err := config.LoadConfig()
	if err != nil {
		log.Fatalf("Failed to load config: %v", err)
	}

	// Initialize InfluxDB client
	// Log configuration (without exposing full token)
	tokenPreview := ""
	if len(cfg.InfluxDB.Token) > 0 {
		if len(cfg.InfluxDB.Token) > 8 {
			tokenPreview = cfg.InfluxDB.Token[:4] + "..." + cfg.InfluxDB.Token[len(cfg.InfluxDB.Token)-4:]
		} else {
			tokenPreview = "***"
		}
	} else {
		tokenPreview = "(empty)"
	}
	log.Printf("Initializing InfluxDB connection - URL: %s, Org: %s, Bucket: %s, Token: %s", 
		cfg.InfluxDB.URL, cfg.InfluxDB.Org, cfg.InfluxDB.Bucket, tokenPreview)
	
	influxClient, err := database.NewInfluxDBClient(
		cfg.InfluxDB.URL,
		cfg.InfluxDB.Token,
		cfg.InfluxDB.Org,
		cfg.InfluxDB.Bucket,
	)
	if err != nil {
		log.Fatalf("Failed to connect to InfluxDB: %v", err)
	}
	defer influxClient.Close()
	log.Printf("Successfully initialized InfluxDB client")

	// Initialize MongoDB client (optional - for report caching)
	var mongoClient *database.MongoDBClient
	// Try to connect if URI is provided OR if Host is set
	if cfg.MongoDB.URI != "" || cfg.MongoDB.Host != "" {
		log.Printf("Initializing MongoDB connection (Host: %s, Port: %s, Database: %s)",
			cfg.MongoDB.Host, cfg.MongoDB.Port, cfg.MongoDB.Database)
		mongoClient, err = database.NewMongoDBClient(cfg.MongoDB)
		if err != nil {
			log.Printf("WARNING: Failed to connect to MongoDB (caching disabled): %v", err)
			mongoClient = nil
		} else {
			log.Printf("Successfully connected to MongoDB for report caching")
			defer mongoClient.Close()
		}
	} else {
		log.Printf("MongoDB not configured (Host and URI are empty), report caching disabled")
	}

	// Initialize services
	dataService := services.NewDataService(influxClient)
	aiService := services.NewAIService(
		cfg.OpenAI, // Still using OpenAI config struct for model/temperature settings
		"schemas/report_schema.json",
	)
	reportService := services.NewReportService(dataService, aiService, mongoClient)
	taskService := services.NewTaskService()

	// Initialize email and PDF services (for weekly reports)
	var emailService *services.EmailService
	var pdfService *services.PDFService
	var weeklyEmailService *services.WeeklyEmailService

	if cfg.Email.APIKey != "" {
		emailService = services.NewEmailService(cfg.Email)
		pdfService = services.NewPDFService()
		weeklyEmailService = services.NewWeeklyEmailService(reportService, emailService, pdfService, mongoClient)

		// Start the cron scheduler
		weeklyEmailService.Start()

		// Load and schedule all opted-in accounts
		if mongoClient != nil {
			err = weeklyEmailService.LoadAndScheduleOptedAccounts()
			if err != nil {
				log.Printf("WARNING: Failed to load opted accounts: %v", err)
			}
		} else {
			log.Printf("WARNING: MongoDB not available, cannot load opted accounts for weekly reports")
		}

		// Ensure cron is stopped on exit
		defer weeklyEmailService.Stop()
	} else {
		log.Printf("SendGrid API key not configured, weekly email reports disabled")
	}

	// Initialize chat tools
	chatTools := services.NewChatTools(influxClient, reportService)

	// Initialize handlers
	handlers := api.NewHandlers(reportService, taskService, weeklyEmailService, mongoClient, chatTools, influxClient)

	// Setup routes
	router := api.SetupRoutes(handlers)

	// Start server
	addr := cfg.Server.Host + ":" + cfg.Server.Port
	log.Printf("Server starting on %s", addr)
	if err := router.Run(addr); err != nil {
		log.Fatalf("Failed to start server: %v", err)
	}
}

