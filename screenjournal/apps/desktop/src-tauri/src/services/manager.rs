use crate::services::types::{AllServicesStatus, ServiceProgress, ServiceStatus};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;


// Global state for service processes (Go binaries)
type ServiceProcesses = Arc<Mutex<Vec<Child>>>;

// Global state for Python processes (managed separately)
type PythonProcesses = Arc<Mutex<Vec<tokio::process::Child>>>;

static SERVICE_PROCESSES: once_cell::sync::Lazy<ServiceProcesses> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

static PYTHON_PROCESSES: once_cell::sync::Lazy<PythonProcesses> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

/// Get the resource directory path for bundled resources
fn get_resource_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let resource_dir = app_handle
        .path()
        .resource_dir()
        .map_err(|e| format!("Failed to get resource directory: {}", e))?;
    
    log::info!("Resource directory resolved to: {:?}", resource_dir);
    
    // Log directory contents for debugging
    if let Ok(entries) = std::fs::read_dir(&resource_dir) {
        let mut subdirs = Vec::new();
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    subdirs.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
        log::info!("Resource directory contains: {:?}", subdirs);
    }
    
    Ok(resource_dir)
}

/// Get the app data directory for storing service data
fn get_app_data_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?;
    
    // Create subdirectories
    std::fs::create_dir_all(&app_data_dir.join("storage")).map_err(|e| {
        format!("Failed to create storage directory: {}", e)
    })?;
    std::fs::create_dir_all(&app_data_dir.join("data")).map_err(|e| {
        format!("Failed to create data directory: {}", e)
    })?;
    std::fs::create_dir_all(&app_data_dir.join("mongodb").join("data")).map_err(|e| {
        format!("Failed to create MongoDB data directory: {}", e)
    })?;
    std::fs::create_dir_all(&app_data_dir.join("influxdb").join("data")).map_err(|e| {
        format!("Failed to create InfluxDB data directory: {}", e)
    })?;
    
    Ok(app_data_dir)
}

/// Get platform and architecture for database binaries
fn get_platform_arch() -> (String, String) {
    let platform = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };
    
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        "unknown"
    };
    
    (platform.to_string(), arch.to_string())
}

/// Start MongoDB database
pub async fn start_mongodb(app_handle: AppHandle) -> Result<(), String> {
    let resource_dir = get_resource_dir(&app_handle)?;
    let app_data_dir = get_app_data_dir(&app_handle)?;
    let (platform, arch) = get_platform_arch();
    
    log::info!("Resource directory: {:?}", resource_dir);
    log::info!("Platform: {}/{}", platform, arch);
    
    let mongod_path = resource_dir
        .join("databases")
        .join("mongodb")
        .join(&platform)
        .join(&arch)
        .join(if cfg!(target_os = "windows") { "mongod.exe" } else { "mongod" });
    
    log::info!("MongoDB binary path: {:?}", mongod_path);
    
    if !mongod_path.exists() {
        return Err(format!("MongoDB binary not found at: {:?}", mongod_path));
    }
    
    // Create data directory
    let db_path = app_data_dir.join("mongodb").join("data");
    log::info!("MongoDB data directory: {:?}", db_path);
    std::fs::create_dir_all(&db_path)
        .map_err(|e| format!("Failed to create MongoDB data directory: {}", e))?;
    
    // Start MongoDB with embedded-friendly settings
    // Note: --nojournal, --smallfiles, and --noprealloc were removed in MongoDB 7.0+
    let mut cmd = Command::new(&mongod_path);
    cmd.arg("--dbpath").arg(&db_path);
    cmd.arg("--port").arg("27017");
    cmd.arg("--bind_ip").arg("127.0.0.1");
    // Use storage engine settings compatible with MongoDB 7.0+
    cmd.arg("--wiredTigerCacheSizeGB").arg("0.5"); // Limit cache for embedded use
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    let mut child = cmd.spawn().map_err(|e| {
        format!("Failed to start MongoDB: {}", e)
    })?;
    
    // Spawn task to read and log stderr
    let stderr = child.stderr.take();
    if let Some(stderr) = stderr {
        tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                let lines = reader.lines();
                for line in lines {
                    match line {
                        Ok(line) => {
                            if !line.trim().is_empty() {
                                log::error!("MongoDB stderr: {}", line);
                            }
                        }
                        Err(e) => {
                            log::error!("Error reading MongoDB stderr: {}", e);
                            break;
                        }
                    }
                }
            })
            .await
            .ok();
        });
    }
    
    {
        let mut processes = SERVICE_PROCESSES.lock().unwrap();
        processes.push(child);
    }
    
    // Wait a moment and check if process is still running
    sleep(Duration::from_millis(500)).await;
    {
        let mut processes = SERVICE_PROCESSES.lock().unwrap();
        if let Some(last_process) = processes.last_mut() {
            if let Ok(Some(status)) = last_process.try_wait() {
                log::error!("MongoDB process exited immediately with status: {:?}", status);
                return Err("MongoDB process exited immediately".to_string());
            }
        }
    }
    
    log::info!("MongoDB process spawned and running");
    Ok(())
}

/// Start InfluxDB database
pub async fn start_influxdb(app_handle: AppHandle) -> Result<(), String> {
    let resource_dir = get_resource_dir(&app_handle)?;
    let app_data_dir = get_app_data_dir(&app_handle)?;
    let (platform, arch) = get_platform_arch();
    
    log::info!("Resource directory: {:?}", resource_dir);
    log::info!("Platform: {}/{}", platform, arch);
    
    let influxd_path = resource_dir
        .join("databases")
        .join("influxdb")
        .join(&platform)
        .join(&arch)
        .join(if cfg!(target_os = "windows") { "influxd.exe" } else { "influxd" });
    
    log::info!("InfluxDB binary path: {:?}", influxd_path);
    
    if !influxd_path.exists() {
        // List directory contents for debugging
        let parent_dir = influxd_path.parent().unwrap();
        if parent_dir.exists() {
            let entries: Vec<String> = std::fs::read_dir(parent_dir)
                .unwrap_or_else(|_| {
                    log::error!("Failed to read directory: {:?}", parent_dir);
                    return std::fs::read_dir(std::path::Path::new(".")).unwrap();
                })
                .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
                .collect();
            log::error!("Directory contents: {:?}", entries);
        }
        return Err(format!("InfluxDB binary not found at: {:?}", influxd_path));
    }
    
    // Create data directory
    let db_path = app_data_dir.join("influxdb").join("data");
    log::info!("InfluxDB data directory: {:?}", db_path);
    
    // Check if InfluxDB needs setup (first run)
    let bolt_path = db_path.join("influxdb.bolt");
    let needs_setup = !bolt_path.exists();
    
    // Start InfluxDB with embedded-friendly settings
    let mut cmd = Command::new(&influxd_path);
    cmd.env("INFLUXD_DATA_DIR", &db_path);
    cmd.env("INFLUXD_BOLT_PATH", &bolt_path);
    cmd.env("INFLUXD_ENGINE_PATH", db_path.join("engine"));
    
    // Set default organization and bucket for embedded use
    if needs_setup {
        cmd.env("DOCKER_INFLUXDB_INIT_MODE", "setup");
        cmd.env("DOCKER_INFLUXDB_INIT_USERNAME", "admin");
        cmd.env("DOCKER_INFLUXDB_INIT_PASSWORD", "admin123");
        cmd.env("DOCKER_INFLUXDB_INIT_ORG", "screenjournal-org");
        cmd.env("DOCKER_INFLUXDB_INIT_BUCKET", "screenjournal-metrics");
        cmd.env("DOCKER_INFLUXDB_INIT_ADMIN_TOKEN", "screenjournal-admin-token-change-in-production");
    }
    
    cmd.arg("--http-bind-address").arg("127.0.0.1:8086");
    cmd.arg("--log-level").arg("error"); // Less logging
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    let mut child = cmd.spawn().map_err(|e| {
        format!("Failed to start InfluxDB: {}", e)
    })?;
    
    // Spawn task to read and log stderr
    let stderr = child.stderr.take();
    if let Some(stderr) = stderr {
        tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                let lines = reader.lines();
                for line in lines {
                    match line {
                        Ok(line) => {
                            if !line.trim().is_empty() {
                                log::error!("InfluxDB stderr: {}", line);
                            }
                        }
                        Err(e) => {
                            log::error!("Error reading InfluxDB stderr: {}", e);
                            break;
                        }
                    }
                }
            })
            .await
            .ok();
        });
    }
    
    {
        let mut processes = SERVICE_PROCESSES.lock().unwrap();
        processes.push(child);
    }
    
    log::info!("InfluxDB process spawned");
    Ok(())
}

/// Wait for MongoDB to be ready
async fn wait_for_mongodb() -> Result<(), String> {
    log::info!("Waiting for MongoDB to be ready...");
    for i in 0..30 {
        if check_mongodb_connection().await {
            log::info!("MongoDB is ready");
            return Ok(());
        }
        if i < 29 {
            sleep(Duration::from_secs(1)).await;
        }
    }
    Err("MongoDB failed to start within 30 seconds".to_string())
}

/// Check if MongoDB is accepting connections
async fn check_mongodb_connection() -> bool {
    // Simple async TCP connection check
    use tokio::net::TcpStream;
    match TcpStream::connect("127.0.0.1:27017").await {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Wait for InfluxDB to be ready
async fn wait_for_influxdb() -> Result<(), String> {
    log::info!("Waiting for InfluxDB to be ready...");
    for i in 0..30 {
        if check_influxdb_connection().await {
            log::info!("InfluxDB is ready");
            return Ok(());
        }
        if i < 29 {
            sleep(Duration::from_secs(1)).await;
        }
    }
    Err("InfluxDB failed to start within 30 seconds".to_string())
}

/// Check if InfluxDB is accepting connections
async fn check_influxdb_connection() -> bool {
    let client = reqwest::Client::new();
    match client
        .get("http://127.0.0.1:8086/health")
        .timeout(Duration::from_secs(1))
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

/// Start the sj-collector Go backend
pub async fn start_collector(app_handle: AppHandle) -> Result<(), String> {
    let resource_dir = get_resource_dir(&app_handle)?;
    let app_data_dir = get_app_data_dir(&app_handle)?;
    
    let collector_binary = resource_dir.join("binaries").join("sj-collector");
    
    // Check if binary exists
    if !collector_binary.exists() {
        // Log directory contents for debugging
        let binaries_dir = resource_dir.join("binaries");
        if binaries_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&binaries_dir) {
                let files: Vec<String> = entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect();
                log::error!("Binaries directory contains: {:?}", files);
            }
        } else {
            log::error!("Binaries directory does not exist: {:?}", binaries_dir);
        }
        return Err(format!("Collector binary not found at: {:?}", collector_binary));
    }
    
    // Check if binary is executable (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&collector_binary) {
            let permissions = metadata.permissions();
            let mode = permissions.mode();
            let is_executable = (mode & 0o111) != 0;
            log::info!("Collector binary permissions: {:o}, executable: {}", mode, is_executable);
            if !is_executable {
                log::warn!("Collector binary is not executable, attempting to fix...");
                if let Err(e) = std::fs::set_permissions(&collector_binary, std::fs::Permissions::from_mode(0o755)) {
                    log::error!("Failed to set executable permissions: {}", e);
                } else {
                    log::info!("Successfully set executable permissions on collector binary");
                }
            }
        }
    }
    
    // Create storage directory
    let storage_path = app_data_dir.join("storage");
    std::fs::create_dir_all(&storage_path)
        .map_err(|e| format!("Failed to create storage directory: {}", e))?;
    
    // Start the collector process with environment variables
    log::info!("Starting collector from: {:?}", collector_binary);
    log::info!("Collector working directory: {:?}", app_data_dir);
    let mut cmd = Command::new(&collector_binary);
    cmd.current_dir(app_data_dir.clone());
    cmd.env("SERVER_HOST", "0.0.0.0");
    cmd.env("SERVER_PORT", "8080");
    cmd.env("JWT_SECRET", "screenjournal-bundled-secret-key");
    cmd.env("INFLUXDB2_URL", "http://localhost:8086");
    cmd.env("INFLUXDB2_TOKEN", "screenjournal-admin-token-change-in-production");
    cmd.env("INFLUXDB2_ORG", "screenjournal-org");
    cmd.env("INFLUXDB2_BUCKET", "screenjournal-metrics");
    cmd.env("STORAGE_BASE_PATH", storage_path.to_string_lossy().as_ref());
    cmd.env("STORAGE_BASE_URL", "http://localhost:8080/storage");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    let mut child = cmd.spawn().map_err(|e| {
        format!("Failed to start collector: {}", e)
    })?;
    
    // Spawn task to read and log stderr
    let stderr = child.stderr.take();
    if let Some(stderr) = stderr {
        tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                let lines = reader.lines();
                for line in lines {
                    match line {
                        Ok(line) => {
                            if !line.trim().is_empty() {
                                log::error!("Collector stderr: {}", line);
                            }
                        }
                        Err(e) => {
                            log::error!("Error reading Collector stderr: {}", e);
                            break;
                        }
                    }
                }
            })
            .await
            .ok();
        });
    }
    
    {
        let mut processes = SERVICE_PROCESSES.lock().unwrap();
        processes.push(child);
    }
    
    // Wait a bit for the service to start
    sleep(Duration::from_secs(2)).await;
    
    // Check if process is still running
    {
        let mut processes = SERVICE_PROCESSES.lock().unwrap();
        if let Some(last_process) = processes.last_mut() {
            if let Ok(Some(status)) = last_process.try_wait() {
                log::error!("Collector process exited immediately with status: {:?}", status);
                return Err("Collector process exited immediately".to_string());
            }
        }
    }
    
    log::info!("Collector service started");
    Ok(())
}

/// Start the sj-tracker-report Go backend
pub async fn start_report_service(app_handle: AppHandle) -> Result<(), String> {
    let resource_dir = get_resource_dir(&app_handle)?;
    let app_data_dir = get_app_data_dir(&app_handle)?;
    
    let report_binary = resource_dir.join("binaries").join("sj-tracker-report");
    
    // Check if binary exists
    if !report_binary.exists() {
        return Err(format!("Report service binary not found at: {:?}", report_binary));
    }
    
    // Check if binary is executable (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&report_binary) {
            let permissions = metadata.permissions();
            let mode = permissions.mode();
            let is_executable = (mode & 0o111) != 0;
            log::info!("Report service binary permissions: {:o}, executable: {}", mode, is_executable);
            if !is_executable {
                log::warn!("Report service binary is not executable, attempting to fix...");
                if let Err(e) = std::fs::set_permissions(&report_binary, std::fs::Permissions::from_mode(0o755)) {
                    log::error!("Failed to set executable permissions: {}", e);
                } else {
                    log::info!("Successfully set executable permissions on report service binary");
                }
            }
        }
    }
    
    // Start the report service process with environment variables
    log::info!("Starting report service from: {:?}", report_binary);
    log::info!("Report service working directory: {:?}", app_data_dir);
    let mut cmd = Command::new(&report_binary);
    cmd.current_dir(app_data_dir.clone());
    cmd.env("PORT", "8085");
    cmd.env("HOST", "0.0.0.0");
    cmd.env("INFLUXDB2_URL", "http://localhost:8086");
    cmd.env("INFLUXDB2_TOKEN", "screenjournal-admin-token-change-in-production");
    cmd.env("INFLUXDB2_ORG", "screenjournal-org");
    cmd.env("INFLUXDB2_BUCKET", "screenjournal-metrics");
    cmd.env("MONGODB_HOST", "localhost");
    cmd.env("MONGODB_PORT", "27017");
    cmd.env("MONGODB_DATABASE", "reports");
    cmd.env("MONGODB_USERNAME", "admin");
    cmd.env("MONGODB_PASSWORD", "admin123");
    cmd.env("MONGODB_AUTH_SOURCE", "admin");
    cmd.env("OPENAI_API_KEY", "");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    let mut child = cmd.spawn().map_err(|e| {
        format!("Failed to start report service: {}", e)
    })?;
    
    // Spawn task to read and log stderr
    let stderr = child.stderr.take();
    if let Some(stderr) = stderr {
        tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                let lines = reader.lines();
                for line in lines {
                    match line {
                        Ok(line) => {
                            if !line.trim().is_empty() {
                                log::error!("Report service stderr: {}", line);
                            }
                        }
                        Err(e) => {
                            log::error!("Error reading Report service stderr: {}", e);
                            break;
                        }
                    }
                }
            })
            .await
            .ok();
        });
    }
    
    {
        let mut processes = SERVICE_PROCESSES.lock().unwrap();
        processes.push(child);
    }
    
    // Wait a bit for the service to start
    sleep(Duration::from_secs(2)).await;
    
    // Check if process is still running
    {
        let mut processes = SERVICE_PROCESSES.lock().unwrap();
        if let Some(last_process) = processes.last_mut() {
            if let Ok(Some(status)) = last_process.try_wait() {
                log::error!("Report service process exited immediately with status: {:?}", status);
                return Err("Report service process exited immediately".to_string());
            }
        }
    }
    
    log::info!("Report service started");
    Ok(())
}

/// Start the Python chat agent (using PyInstaller standalone executable)
pub async fn start_chat_agent(app_handle: AppHandle) -> Result<(), String> {
    let resource_dir = get_resource_dir(&app_handle)?;
    let app_data_dir = get_app_data_dir(&app_handle)?;
    
    // Use PyInstaller standalone executable (bundled app approach)
    let chat_agent_exe = if cfg!(target_os = "windows") {
        resource_dir.join("python").join("sj-tracker-chat-agent").join("sj-chat-agent.exe")
    } else {
        resource_dir.join("python").join("sj-tracker-chat-agent").join("sj-chat-agent")
    };
    
    // Check if standalone executable exists
    if !chat_agent_exe.exists() {
        // Fallback: try Python venv approach (for development)
    let venv_dir = resource_dir.join("python").join("sj-tracker-chat-agent-venv");
    let server_script = resource_dir.join("python").join("sj-tracker-chat-agent").join("server.py");
    
        if venv_dir.exists() && server_script.exists() {
            log::info!("Using Python venv approach (development mode)");
    let python_exe = if cfg!(target_os = "windows") {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python3")
    };
    
    if !python_exe.exists() {
        return Err(format!("Python executable not found at: {:?}", python_exe));
    }
    
    let mut cmd = TokioCommand::new(&python_exe);
    cmd.arg(&server_script);
    cmd.current_dir(app_data_dir.clone());
    cmd.env("BACKEND_URL", "http://localhost:8085");
    cmd.env("CHAT_AGENT_PORT", "8087");
    cmd.env("HOST", "0.0.0.0");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    let child = cmd.spawn().map_err(|e| {
        format!("Failed to start chat agent: {}", e)
    })?;
    
    {
        let mut python_processes = PYTHON_PROCESSES.lock().unwrap();
        python_processes.push(child);
    }
    
    sleep(Duration::from_secs(2)).await;
            log::info!("Chat agent started (venv mode)");
            return Ok(());
        }
        
        return Err(format!("Chat agent executable not found at: {:?}", chat_agent_exe));
    }
    
    log::info!("Starting chat agent using standalone executable: {:?}", chat_agent_exe);
    
    // Start the chat agent using standalone executable
    let mut cmd = TokioCommand::new(&chat_agent_exe);
    cmd.current_dir(app_data_dir.clone());
    cmd.env("BACKEND_URL", "http://localhost:8085");
    cmd.env("CHAT_AGENT_PORT", "8087");
    cmd.env("HOST", "0.0.0.0");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    let mut child = cmd.spawn().map_err(|e| {
        format!("Failed to start chat agent: {}", e)
    })?;
    
    // Spawn task to read and log stderr for debugging
    let stderr = child.stderr.take();
    if let Some(stderr) = stderr {
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.trim().is_empty() {
                    log::error!("Chat agent stderr: {}", line);
                }
            }
        });
    }
    
    // Spawn task to read and log stdout for debugging
    let stdout = child.stdout.take();
    if let Some(stdout) = stdout {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.trim().is_empty() {
                    log::info!("Chat agent stdout: {}", line);
                }
            }
        });
    }
    
    // Store Python process separately
    {
        let mut python_processes = PYTHON_PROCESSES.lock().unwrap();
        python_processes.push(child);
    }
    
    // Wait a bit for the service to start (PyInstaller executables may take longer)
    sleep(Duration::from_secs(3)).await;
    
    // Check if process is still running
    {
        let mut python_processes = PYTHON_PROCESSES.lock().unwrap();
        if let Some(last_process) = python_processes.last_mut() {
            if let Ok(Some(status)) = last_process.try_wait() {
                log::error!("Chat agent process exited immediately with status: {:?}", status);
                return Err("Chat agent process exited immediately".to_string());
            }
        }
    }
    
    log::info!("Chat agent started (standalone executable)");
    Ok(())
}

/// Start the report frontend (Next.js app on port 3030)
pub async fn start_report_frontend(app_handle: AppHandle) -> Result<(), String> {
    let resource_dir = get_resource_dir(&app_handle)?;
    
    let frontend_dir = resource_dir.join("frontend").join("sj-tracker-frontend");
    
    // Check if frontend directory exists
    if !frontend_dir.exists() {
        log::warn!("Report frontend not found at: {:?}. Skipping frontend startup.", frontend_dir);
        log::warn!("Note: Report frontend requires Node.js to be installed on the system.");
        return Err(format!("Report frontend not found at: {:?}", frontend_dir));
    }
    
    // Check for Node.js - try "node" command (OS will find it in PATH)
    let node_exe = if cfg!(target_os = "windows") {
        "node.exe"
    } else {
        "node"
    };
    
    log::info!("Starting report frontend from: {:?}", frontend_dir);
    log::info!("Using Node.js command: {}", node_exe);
    
    // Check if node_modules/.bin/next exists
    let next_bin = frontend_dir.join("node_modules").join(".bin").join(if cfg!(target_os = "windows") { "next.cmd" } else { "next" });
    if !next_bin.exists() {
        return Err(format!("Next.js binary not found at: {:?}. Frontend needs to be built with 'npm install' and 'npm run build'.", next_bin));
    }
    
    // Start the Next.js frontend
    // Note: This requires Node.js to be installed on the system
    let mut cmd = TokioCommand::new(node_exe);
    cmd.arg(&next_bin);
    cmd.arg("start");
    cmd.arg("-p");
    cmd.arg("3030");
    cmd.current_dir(frontend_dir.clone());
    cmd.env("NODE_ENV", "production");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    
    let child = cmd.spawn().map_err(|e| {
        format!("Failed to start report frontend: {}", e)
    })?;
    
    // Store as Python process (we can create a separate type later if needed)
    {
        let mut python_processes = PYTHON_PROCESSES.lock().unwrap();
        python_processes.push(child);
    }
    
    // Wait a bit for the service to start
    sleep(Duration::from_secs(3)).await;
    
    log::info!("Report frontend started");
    Ok(())
}

/// Start all backend services using the bundled startup script
pub async fn start_all_services(app_handle: AppHandle) -> Result<(), String> {
    log::info!("Starting all backend services using bundled script...");
    
    let resource_dir = get_resource_dir(&app_handle)?;
    let app_data_dir = get_app_data_dir(&app_handle)?;
    
    // Determine script path based on platform
    let (script_path, shell_cmd) = if cfg!(target_os = "windows") {
        let script = resource_dir.join("start-bundled.bat");
        (script, "cmd.exe")
    } else {
        let script = resource_dir.join("start-bundled.sh");
        (script, "bash")
    };
    
    if !script_path.exists() {
        log::error!(
            "Startup script not found at: {:?} (resource_dir: {:?})",
            script_path,
            resource_dir
        );
        return Err(format!("Startup script not found at: {:?}", script_path));
    }
    
    // Use string paths for env vars so Windows gets a clear UTF-8 value (PathBuf/OsStr can be tricky in child env)
    #[cfg(target_os = "windows")]
    let (resource_dir_str, app_data_dir_str) = {
        let mut r = resource_dir.to_string_lossy().to_string();
        let mut a = app_data_dir.to_string_lossy().to_string();
        for s in [&mut r, &mut a] {
            if s.starts_with(r"\\?\") {
                *s = s[r"\\?\".len()..].to_string();
            }
        }
        (r, a)
    };
    #[cfg(not(target_os = "windows"))]
    let (resource_dir_str, app_data_dir_str) = (
        resource_dir.to_string_lossy().to_string(),
        app_data_dir.to_string_lossy().to_string(),
    );

    log::info!(
        "Executing startup script: {:?} with RESOURCE_DIR={:?} APP_DATA_DIR={:?}",
        script_path,
        resource_dir_str,
        app_data_dir_str
    );

    let mut cmd = TokioCommand::new(shell_cmd);
    if cfg!(target_os = "windows") {
        let mut script_path_str = script_path.to_string_lossy().to_string();
        if script_path_str.starts_with(r"\\?\") {
            script_path_str = script_path_str[r"\\?\".len()..].to_string();
        }
        cmd.arg("/c").arg(&script_path_str);
    } else {
        cmd.arg(&script_path);
    }
    cmd.env("RESOURCE_DIR", &resource_dir_str);
    cmd.env("APP_DATA_DIR", &app_data_dir_str);
    cmd.current_dir(&app_data_dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| {
        format!("Failed to execute startup script: {}", e)
    })?;
    
    // Read stdout line by line and parse progress; signal when script reports "all:ready"
    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let mut reader = BufReader::new(stdout).lines();
    let (all_ready_tx, all_ready_rx) = oneshot::channel::<()>();
    
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        while let Ok(Some(line)) = reader.next_line().await {
            let line = line.trim();
            
            if line.starts_with("[PROGRESS]") {
                let parts: Vec<&str> = line[10..].trim().split(':').collect();
                if parts.len() == 2 {
                    let service = parts[0].to_string();
                    let status = parts[1].to_string();
                    
                    let progress = ServiceProgress {
                        service: service.clone(),
                        status: status.clone(),
                        message: None,
                    };
                    let _ = app_handle_clone.emit("service-progress", &progress);
                    log::info!("Service progress: {} -> {}", service, status);
                    
                    if service == "all" && status == "ready" {
                        let _ = all_ready_tx.send(());
                        break;
                    }
                    // On Windows, CMD pipe buffering often prevents "all:ready" from being read.
                    // When we see the last service (frontend) ready/skipped, emit synthetic all:ready
                    // so the frontend can advance without waiting for the script's echoed line.
                    if service == "frontend" && (status == "ready" || status == "skipped") {
                        let all_progress = ServiceProgress {
                            service: "all".to_string(),
                            status: "ready".to_string(),
                            message: None,
                        };
                        let _ = app_handle_clone.emit("service-progress", &all_progress);
                        let _ = all_ready_tx.send(());
                        break;
                    }
                }
            } else if line.starts_with("[STEP]") {
                log::info!("{}", &line[7..]);
            } else if line.starts_with("[SUCCESS]") {
                log::info!("{}", &line[9..]);
            } else if line.starts_with("[ERROR]") {
                log::error!("{}", &line[7..]);
            }
        }
    });
    
    // Keep the script process alive (it runs :keep_alive forever); don't block on wait()
    let mut child_handle = child;
    tokio::spawn(async move {
        let _ = child_handle.wait().await;
    });
    
    // Return once we see "all:ready" or after timeout; frontend can also advance via status polling
    const ALL_READY_TIMEOUT_SECS: u64 = 120;
    match tokio::time::timeout(Duration::from_secs(ALL_READY_TIMEOUT_SECS), all_ready_rx).await {
        Ok(Ok(())) => {
            log::info!("All backend services started successfully via script (all:ready received)");
        }
        Ok(Err(_)) => {
            log::warn!("Startup script output channel closed before all:ready");
        }
        Err(_) => {
            log::warn!(
                "No all:ready from script within {}s; frontend may advance via status polling",
                ALL_READY_TIMEOUT_SECS
            );
        }
    };
    Ok(())
}

/// Stop all backend services
pub async fn stop_all_services(app_handle: AppHandle) -> Result<(), String> {
    log::info!("Stopping all backend services...");
    
    let app_data_dir = get_app_data_dir(&app_handle)?;
    let pids_file = app_data_dir.join("service_pids.txt");
    
    // Try to read PIDs from file (created by startup script)
    if pids_file.exists() {
        if let Ok(contents) = std::fs::read_to_string(&pids_file) {
            for line in contents.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    if let Ok(pid) = value.trim().parse::<u32>() {
                        log::info!("Stopping process {} ({})", pid, key);
                        // Try to kill the process
                        #[cfg(unix)]
                        {
                            use std::process::Command;
                            let _ = Command::new("kill").arg(pid.to_string()).output();
                        }
                        #[cfg(windows)]
                        {
                            use std::process::Command;
                            let _ = Command::new("taskkill")
                                .args(&["/F", "/PID", &pid.to_string()])
                                .output();
                        }
                    }
                }
            }
            // Remove the PIDs file
            let _ = std::fs::remove_file(&pids_file);
        }
    }
    
    // Also stop processes managed directly by Rust
    let mut processes = SERVICE_PROCESSES.lock().unwrap();
    for mut child in processes.drain(..) {
        if let Err(e) = child.kill() {
            log::warn!("Failed to kill Go process: {}", e);
        } else {
            log::info!("Go service process stopped");
        }
    }
    
    // Stop Python processes
    let mut python_processes = PYTHON_PROCESSES.lock().unwrap();
    for mut child in python_processes.drain(..) {
        if let Err(e) = child.kill().await {
            log::warn!("Failed to kill Python process: {}", e);
        } else {
            log::info!("Python service process stopped");
        }
    }
    
    log::info!("All backend services stopped");
    Ok(())
}

/// Get status of all services
#[tauri::command]
pub async fn get_all_services_status(_app_handle: AppHandle) -> Result<AllServicesStatus, String> {
    // Check if services are running by attempting to connect to their ports
    let mongodb_status = check_database_status("mongodb", 27017).await;
    let influxdb_status = check_service_status("influxdb", 8086).await;
    let collector_status = check_service_status("collector", 8080).await;
    let report_status = check_service_status("report", 8085).await;
    let chat_agent_status = check_service_status("chat_agent", 8087).await;
    
    // Report frontend runs on port 3030
    let frontend_status = check_service_status("frontend", 3030).await;
    
    Ok(AllServicesStatus {
        mongodb: mongodb_status,
        influxdb: influxdb_status,
        collector: collector_status,
        report: report_status,
        chat_agent: chat_agent_status,
        frontend: frontend_status,
    })
}

async fn check_database_status(name: &str, port: u16) -> ServiceStatus {
    // For MongoDB, use async TCP connection check
    use tokio::net::TcpStream;
    match TcpStream::connect(format!("127.0.0.1:{}", port)).await {
        Ok(_) => ServiceStatus {
            name: name.to_string(),
            running: true,
            pid: None,
            port: Some(port),
            error: None,
        },
        Err(_) => ServiceStatus {
            name: name.to_string(),
            running: false,
            pid: None,
            port: Some(port),
            error: Some("Database not responding".to_string()),
        },
    }
}

async fn check_service_status(name: &str, port: u16) -> ServiceStatus {
    // Try to connect to the service
    // For Next.js frontend (port 3030), check root URL
    // For other services, check /health endpoint
    let client = reqwest::Client::new();
    let url = if port == 3030 {
        format!("http://localhost:{}", port)
    } else {
        format!("http://localhost:{}/health", port)
    };
    
    match client.get(&url).timeout(Duration::from_secs(1)).send().await {
        Ok(response) => {
            if response.status().is_success() {
                ServiceStatus {
                    name: name.to_string(),
                    running: true,
                    pid: None,
                    port: Some(port),
                    error: None,
                }
            } else {
                ServiceStatus {
                    name: name.to_string(),
                    running: false,
                    pid: None,
                    port: Some(port),
                    error: Some(format!("Service returned status {}", response.status())),
                }
            }
        }
        Err(e) => ServiceStatus {
            name: name.to_string(),
            running: false,
            pid: None,
            port: Some(port),
            error: Some(format!("Connection failed: {}", e)),
        },
    }
}

