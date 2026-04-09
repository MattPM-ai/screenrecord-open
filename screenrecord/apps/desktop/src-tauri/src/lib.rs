use tauri::Manager;
use tauri_plugin_log::{Target, TargetKind};

pub mod activitywatch;
pub mod recording;
pub mod collector;
pub mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            // ActivityWatch commands
            crate::activitywatch::manager::start_server,
            crate::activitywatch::manager::get_server_status,
            crate::activitywatch::manager::get_server_info,
            crate::activitywatch::manager::stop_server,
            crate::activitywatch::manager::start_watcher,
            crate::activitywatch::manager::stop_watcher,
            crate::activitywatch::manager::get_watchers_status,
            crate::activitywatch::manager::get_buckets,
            crate::activitywatch::manager::get_bucket_events,
            crate::activitywatch::manager::get_current_status,
            crate::activitywatch::manager::get_events_by_date_range,
            crate::activitywatch::manager::get_daily_metrics,
            crate::activitywatch::manager::get_app_usage_breakdown,
            // Recording commands (multi-display MP4 capture)
            crate::recording::manager::start_recording,
            crate::recording::manager::stop_recording,
            crate::recording::manager::get_recording_status,
            crate::recording::manager::get_recording_config,
            crate::recording::manager::update_recording_config,
            crate::recording::manager::get_display_count,
            crate::recording::manager::get_recordings_by_date_range,
            // Gemini AI integration commands
            crate::recording::manager::has_gemini_api_key,
            crate::recording::manager::get_gemini_config,
            crate::recording::manager::update_gemini_config,
            crate::recording::manager::get_gemini_queue_status,
            crate::recording::manager::set_gemini_api_key,
            crate::recording::manager::get_gemini_api_key_status,
            crate::recording::manager::delete_gemini_api_key,
            // Audio feature and transcription commands
            crate::recording::manager::get_audio_feature_config,
            crate::recording::manager::update_audio_feature_config,
            crate::recording::manager::is_whisper_available,
            crate::recording::manager::get_transcription_queue_status,
            // Collector commands
            crate::collector::manager::start_collector,
            crate::collector::manager::stop_collector,
            crate::collector::manager::get_collector_status,
            crate::collector::manager::update_collector_config,
            crate::collector::manager::get_collector_config,
            crate::collector::manager::test_collector_connection,
            crate::collector::manager::update_collector_app_jwt_token,
            crate::collector::manager::trigger_daily_collection,
            // Backend services management
            crate::services::manager::get_all_services_status,
        ])
        .setup(|app| {
            // Enable logging in both debug and release modes
            // In release mode, use Info level for services to help diagnose startup issues
            let log_level = if cfg!(debug_assertions) {
                log::LevelFilter::Warn
            } else {
                log::LevelFilter::Info
            };

            // Write logs to app data dir so users can read them (e.g. desktop.log alongside report.log, chat-agent.log)
            let log_targets: Vec<tauri_plugin_log::Target> = match app.path().app_data_dir() {
                Ok(dir) => {
                    let _ = std::fs::create_dir_all(&dir);
                    vec![
                        Target::new(TargetKind::Stderr),
                        Target::new(TargetKind::Folder {
                            path: dir,
                            file_name: Some("desktop.log".to_string()),
                        }),
                    ]
                }
                Err(_) => {
                    vec![
                        Target::new(TargetKind::Stderr),
                        Target::new(TargetKind::LogDir {
                            file_name: Some("desktop.log".to_string()),
                        }),
                    ]
                }
            };

            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .targets(log_targets)
                    .level(log_level)
                    // Set specific levels for modules
                    .level_for("app_lib::services", log::LevelFilter::Info) // Always show service logs
                    .level_for("app_lib::collector", log::LevelFilter::Warn)
                    .level_for("app_lib::activitywatch", log::LevelFilter::Warn)
                    .level_for("app_lib::recording", log::LevelFilter::Warn)
                    .build(),
            )?;
            
            // Initialize recording system
            // 1. Initialize FFmpeg path (must be done before any recording operations)
            crate::recording::capture::init_ffmpeg_path(&app.handle());
            
            // 2. Load recording config on startup
            let recording_config = crate::recording::config::load_config(&app.handle())
                .unwrap_or_default();
            crate::recording::manager::init_config(recording_config);
            
            // 3. Initialize audio feature config
            if let Err(e) = crate::recording::manager::init_audio_config(&app.handle()) {
                log::warn!("Failed to initialize audio feature config: {}", e);
            }
            
            // 4. Initialize Gemini queue
            let gemini_config = crate::recording::config::load_gemini_config(&app.handle())
                .unwrap_or_default();
            crate::recording::gemini::init_queue(&app.handle(), gemini_config);
            
            // 5. Initialize Whisper model for transcription
            if let Some(whisper_model_path) =
                crate::recording::transcription::whisper::resolve_whisper_model_path(&app.handle())
            {
                if let Err(e) =
                    crate::recording::transcription::whisper::init_whisper(&whisper_model_path)
                {
                    log::warn!(
                        "Failed to initialize Whisper model: {} - transcription will be disabled",
                        e
                    );
                } else {
                    log::info!("Whisper model initialized successfully");
                }
            } else {
                log::warn!(
                    "Whisper model not found - transcription will be disabled"
                );
            }
            
            // 6. Initialize transcription queue with settings from audio config
            let audio_config = crate::recording::config::load_audio_feature_config(&app.handle())
                .unwrap_or_default();
            let transcription_config = crate::recording::transcription::TranscriptionConfig {
                enabled: audio_config.transcription_enabled,
                model: audio_config.transcription_model,
                max_retries: audio_config.transcription_max_retries,
                retry_delay_seconds: audio_config.transcription_retry_delay_seconds,
                processing_delay_seconds: audio_config.transcription_processing_delay_seconds,
            };
            crate::recording::transcription::init_queue(&app.handle(), transcription_config);
            
            // Start all backend services on app launch
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                log::info!("Starting bundled backend services...");
                if let Err(e) = crate::services::manager::start_all_services(app_handle.clone()).await {
                    log::error!("Failed to start backend services: {}", e);
                } else {
                    log::info!("All backend services started successfully");
                }
            });
            
            // Load collector config and auto-start if enabled
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match crate::collector::config::load_config(&app_handle) {
                    Ok(collector_config) => {
                        if collector_config.enabled {
                            log::info!("Collector is enabled in config, starting automatically...");
                            if let Err(e) = crate::collector::manager::start_collector(
                                app_handle,
                                collector_config,
                            ).await {
                                log::error!("Failed to auto-start collector: {}", e);
                            } else {
                                log::info!("Collector auto-started successfully");
                            }
                        } else {
                            log::info!("Collector is disabled in config, not auto-starting");
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to load collector config on startup: {}", e);
                    }
                }
            });
            
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Only trigger graceful shutdown when the MAIN window is closed
                // Other windows (like settings) should close without affecting the app
                let window_label = window.label();
                
                if window_label != "main" {
                    log::info!("Window '{}' closed, app continues running", window_label);
                    return;
                }
                
                // Handle graceful shutdown when main window is closing
                log::info!("Main window close requested, performing graceful shutdown...");
                
                // Stop server and watchers
                tauri::async_runtime::block_on(async {
                    if let Err(e) = crate::activitywatch::manager::stop_server().await {
                        log::error!("Failed to stop server during shutdown: {}", e);
                    } else {
                        log::info!("✓ Server stopped gracefully");
                    }
                });
                
                // Stop recording
                let app_handle = window.app_handle().clone();
                tauri::async_runtime::block_on(async {
                    if let Err(e) = crate::recording::manager::stop_recording(app_handle).await {
                        log::error!("Failed to stop recording during shutdown: {}", e);
                    } else {
                        log::info!("✓ Recording stopped gracefully");
                    }
                });
                
                // Stop collector
                tauri::async_runtime::block_on(async {
                    if let Err(e) = crate::collector::manager::stop_collector().await {
                        log::error!("Failed to stop collector during shutdown: {}", e);
                    } else {
                        log::info!("✓ Collector stopped gracefully");
                    }
                });
                
                // Stop Gemini queue
                tauri::async_runtime::block_on(async {
                    crate::recording::gemini::shutdown_queue().await;
                    log::info!("✓ Gemini queue stopped gracefully");
                });
                
                // Stop all backend services
                let app_handle = window.app_handle().clone();
                tauri::async_runtime::block_on(async {
                    if let Err(e) = crate::services::manager::stop_all_services(app_handle).await {
                        log::error!("Failed to stop backend services: {}", e);
                    } else {
                        log::info!("✓ Backend services stopped gracefully");
                    }
                });
                
                log::info!("Graceful shutdown complete");
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
