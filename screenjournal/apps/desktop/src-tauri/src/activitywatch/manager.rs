use crate::activitywatch::types::{ServerInfo, ServerStatus, WatcherStatus};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::Duration,
};
use sysinfo::{System, Pid};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use tauri::{AppHandle, Manager};

static AW_CHILD: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));
static AW_WATCHERS: Lazy<Mutex<HashMap<String, Child>>> = Lazy::new(|| Mutex::new(HashMap::new()));
pub static AW_BASE_URL: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
static AW_PORT: Lazy<Mutex<Option<u16>>> = Lazy::new(|| Mutex::new(None));

fn app_data_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app_data_dir available")
        .join("activitywatch")
}

fn platform_bin_name() -> (&'static str, &'static str) {
    #[cfg(target_os = "windows")]
    {
        ("windows/x86_64", "aw-server.exe")
    }
    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        {
            ("darwin/aarch64", "aw-server")
        }
        #[cfg(target_arch = "x86_64")]
        {
            ("darwin/x86_64", "aw-server")
        }
    }
    #[cfg(target_os = "linux")]
    {
        ("linux/x86_64", "aw-server")
    }
}

fn platform_watcher_names() -> Vec<&'static str> {
    vec!["aw-watcher-window", "aw-watcher-afk", "aw-watcher-input"]
}

fn bundled_aw_server_path(app: &AppHandle) -> PathBuf {
    let (platform_subdir, bin) = platform_bin_name();

    // New structure: aw-server is in aw-server/aw-server subdirectory
    let base_path = |root: PathBuf| -> PathBuf {
        // Try new structure first: platform/aw-server/aw-server
        let new_path = root.join(platform_subdir).join("aw-server").join(&bin);
        if new_path.exists() {
            return new_path;
        }
        // Fallback to old structure: platform/aw-server (direct)
        let old_path = root.join(platform_subdir).join(&bin);
        if old_path.exists() {
            return old_path;
        }
        new_path // return new path as default
    };

    // DEV: use repo resource path if present
    if cfg!(debug_assertions) {
        // 1) explicit src-tauri/resources path from project root
        let candidate1 = base_path(
            PathBuf::from("src-tauri")
                .join("resources")
                .join("activitywatch"),
        );
        if candidate1.exists() {
            return candidate1;
        }
        // 2) resources path when cwd is src-tauri
        let candidate2 = base_path(PathBuf::from("resources").join("activitywatch"));
        if candidate2.exists() {
            return candidate2;
        }
        // 3) try resolving via BaseDirectory::Resource
        if let Ok(p) = app.path().resolve(
            format!(
                "resources/activitywatch/{}/aw-server/{}",
                platform_subdir, bin
            ),
            tauri::path::BaseDirectory::Resource,
        ) {
            if p.exists() {
                return p;
            }
        }
    }

    // PROD: use packaged resource dir (new structure)
    base_path(
        app.path()
            .resource_dir()
            .expect("resource_dir available")
            .join("activitywatch"),
    )
}

fn bundled_watcher_path(app: &AppHandle, watcher_name: &str) -> Option<PathBuf> {
    let (platform_subdir, _) = platform_bin_name();

    // New structure: watchers are in watcher-name/watcher-name subdirectories
    // Windows binaries require .exe extension
    #[cfg(target_os = "windows")]
    let bin_name = format!("{}.exe", watcher_name);
    #[cfg(not(target_os = "windows"))]
    let bin_name = watcher_name.to_string();

    let base_path = |root: PathBuf| -> PathBuf {
        root.join(platform_subdir)
            .join(watcher_name)
            .join(&bin_name)
    };

    // DEV: use repo resource path if present
    if cfg!(debug_assertions) {
        let candidate1 = base_path(
            PathBuf::from("src-tauri")
                .join("resources")
                .join("activitywatch"),
        );
        if candidate1.exists() {
            return Some(candidate1);
        }
        let candidate2 = base_path(PathBuf::from("resources").join("activitywatch"));
        if candidate2.exists() {
            return Some(candidate2);
        }
    }

    // PROD: use packaged resource dir
    let path = base_path(
        app.path()
            .resource_dir()
            .expect("resource_dir available")
            .join("activitywatch"),
    );
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

// Detect and cleanup orphaned ActivityWatch processes from previous sessions
// Returns a list of process names that were terminated
fn detect_and_cleanup_orphaned_processes() -> Result<Vec<String>, String> {
    log::info!("Checking for orphaned ActivityWatch processes...");
    
    let mut system = System::new_all();
    system.refresh_all();
    
    let target_names = vec![
        "aw-server",
        "aw-watcher-window",
        "aw-watcher-afk",
        "aw-watcher-input",
    ];
    
    let mut terminated = Vec::new();
    
    // Collect PIDs to kill first to avoid borrow issues
    let mut pids_to_kill: Vec<(Pid, String)> = Vec::new();
    
    for (pid, process) in system.processes() {
        let process_name = process.name().to_string();
        
        // Check if this process matches any of our target names
        let matches = target_names.iter().any(|name| {
            // Handle both exact matches and executable names with extensions
            process_name == *name || 
            process_name.starts_with(name) ||
            process_name == format!("{}.exe", name)
        });
        
        if matches {
            log::warn!(
                "Found orphaned process: {} (PID: {})",
                process_name,
                pid
            );
            pids_to_kill.push((*pid, process_name));
        }
    }
    
    // Now kill the processes
    for (pid, process_name) in pids_to_kill {
        // Attempt to kill the process
        #[cfg(target_os = "windows")]
        {
            let output = std::process::Command::new("taskkill")
                .args(&["/F", "/PID", &pid.as_u32().to_string()])
                .output();
                
            match output {
                Ok(_) => {
                    log::info!("Successfully terminated {} (PID: {})", process_name, pid);
                    terminated.push(process_name);
                }
                Err(e) => {
                    log::error!("Failed to terminate {} (PID: {}): {}", process_name, pid, e);
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // On Unix-like systems, use kill command
            let output = std::process::Command::new("kill")
                .arg(pid.as_u32().to_string())
                .output();
                
            match output {
                Ok(_) => {
                    log::info!("Successfully terminated {} (PID: {})", process_name, pid);
                    terminated.push(process_name);
                    
                    // Give it a moment, then force kill if still alive
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    system.refresh_all();
                    
                    if system.process(pid).is_some() {
                        log::warn!("Process {} still alive, sending SIGKILL", pid);
                        let _ = std::process::Command::new("kill")
                            .args(&["-9", &pid.as_u32().to_string()])
                            .output();
                    }
                }
                Err(e) => {
                    log::error!("Failed to terminate {} (PID: {}): {}", process_name, pid, e);
                }
            }
        }
    }
    
    if terminated.is_empty() {
        log::info!("No orphaned processes found");
    } else {
        log::info!("Terminated {} orphaned process(es): {:?}", terminated.len(), terminated);
    }
    
    Ok(terminated)
}

// Ensure all required watchers are running and tracked
// Verifies existing watchers are alive and starts any that are missing
fn ensure_watchers_running(app: &AppHandle, port: u16) -> Result<Vec<String>, String> {
    log::info!("Ensuring all watchers are running...");
    
    let mut system = System::new_all();
    system.refresh_all();
    
    let mut watchers = AW_WATCHERS.lock().unwrap();
    let mut started_watchers = Vec::new();
    
    for watcher_name in platform_watcher_names() {
        let mut needs_start = false;
        
        // Check if watcher is in our HashMap
        if let Some(child) = watchers.get_mut(watcher_name) {
            // Verify the process is still alive
            let pid = child.id();
            system.refresh_all();
            let sysinfo_pid = Pid::from_u32(pid);
            
            if system.process(sysinfo_pid).is_none() {
                log::warn!("Watcher {} (PID: {}) is dead, will restart", watcher_name, pid);
                needs_start = true;
                watchers.remove(watcher_name);
            } else {
                log::debug!("Watcher {} is already running (PID: {})", watcher_name, pid);
            }
        } else {
            log::info!("Watcher {} not tracked, will start", watcher_name);
            needs_start = true;
        }
        
        // Start the watcher if needed
        if needs_start {
            let watcher_path = bundled_watcher_path(app, watcher_name);
            if let Some(watcher_path) = watcher_path {
                let watcher_path = std::fs::canonicalize(&watcher_path).unwrap_or(watcher_path);
                
                let mut cmd = Command::new(&watcher_path);
                if let Some(bin_dir) = watcher_path.parent() {
                    cmd.current_dir(bin_dir);
                }
                cmd.arg("--host")
                    .arg("127.0.0.1")
                    .arg("--port")
                    .arg(port.to_string())
                    .arg("--testing")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                #[cfg(target_os = "windows")]
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                match cmd.spawn() {
                    Ok(child) => {
                        log::info!("Started watcher {} (PID: {:?})", watcher_name, child.id());
                        started_watchers.push(watcher_name.to_string());
                        watchers.insert(watcher_name.to_string(), child);
                    }
                    Err(e) => {
                        log::error!("Failed to start watcher {}: {}", watcher_name, e);
                        return Err(format!("Failed to start watcher {}: {}", watcher_name, e));
                    }
                }
            } else {
                log::error!("Watcher {} binary not found", watcher_name);
                return Err(format!("Watcher {} binary not found", watcher_name));
            }
        }
    }
    
    log::info!("Watcher check complete. Started {} watcher(s)", started_watchers.len());
    Ok(started_watchers)
}

// Verify that watchers are actually creating data by checking for buckets
// Waits up to 10 seconds for required bucket types to appear
async fn verify_watcher_health(base_url: &str) -> Result<bool, String> {
    log::info!("Verifying watcher health by checking for buckets...");
    
    let required_bucket_types = vec!["currentwindow", "afkstatus"];
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    
    while std::time::Instant::now() < deadline {
        // Try to fetch buckets from the server
        match crate::activitywatch::client::fetch_buckets(base_url).await {
            Ok(buckets) => {
                let mut found_types = std::collections::HashSet::new();
                
                for bucket in &buckets {
                    found_types.insert(bucket.bucket_type.as_str());
                }
                
                // Check if we have all required bucket types
                let all_found = required_bucket_types.iter()
                    .all(|req_type| found_types.contains(req_type));
                
                if all_found {
                    log::info!("All required buckets found: {:?}", required_bucket_types);
                    return Ok(true);
                } else {
                    let missing: Vec<&str> = required_bucket_types.iter()
                        .filter(|req_type| !found_types.contains(**req_type))
                        .copied()
                        .collect();
                    log::debug!("Still waiting for buckets: {:?}", missing);
                }
            }
            Err(e) => {
                log::debug!("Failed to fetch buckets (retrying): {}", e);
            }
        }
        
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    
    log::warn!("Watcher health verification timed out - not all buckets created");
    // Return Ok(false) instead of error to allow app to continue
    // Watchers might just need more time
    Ok(false)
}

pub async fn wait_healthy(base_url: &str, timeout: Duration) -> bool {
    let start = tokio::time::Instant::now();
    let deadline = start + timeout;
    let mut attempt = 0;
    while tokio::time::Instant::now() < deadline {
        attempt += 1;
        if let Ok(resp) = reqwest::Client::new()
            .get(format!("{}/api/0/info", base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            if resp.status().is_success() {
                if attempt > 1 {
                    let elapsed = start.elapsed();
                    log::info!("aw-server health check succeeded after {} attempts ({:.1}s)", attempt, elapsed.as_secs_f64());
                }
                return true;
            }
        }
        // Log progress every 5 seconds (approximately every 16 attempts at 300ms intervals)
        if attempt % 16 == 0 {
            let elapsed = start.elapsed();
            log::debug!("aw-server health check attempt {} (elapsed: {:.1}s)", attempt, elapsed.as_secs_f64());
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    let elapsed = start.elapsed();
    log::warn!("aw-server health check timed out after {} attempts ({:.1}s)", attempt, elapsed.as_secs_f64());
    false
}

fn pick_port() -> u16 {
    std::env::var("AW_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(5600)
}

fn use_system_aw() -> bool {
    std::env::var("USE_SYSTEM_AW")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn system_aw_base_url() -> String {
    format!("http://127.0.0.1:{}", pick_port())
}

// Removed ensure_server_running - use start_server instead

fn start_server_internal(
    data_dir: PathBuf,
    bin_path: PathBuf,
    app: AppHandle,
) -> impl std::future::Future<Output = Result<ServerInfo, String>> + Send {
    async move {
        // Clean up any orphaned processes from previous sessions (force quit, crashes, etc.)
        match detect_and_cleanup_orphaned_processes() {
            Ok(terminated) => {
                if !terminated.is_empty() {
                    log::info!("Cleaned up {} orphaned process(es), waiting for ports to release...", terminated.len());
                    // Give the OS time to release the ports
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
            Err(e) => {
                log::warn!("Failed to cleanup orphaned processes: {}", e);
                // Continue anyway - this is not fatal
            }
        }
        
        // Invalidate any stale cached client from previous session
        crate::activitywatch::client::invalidate_client();
        
        // If already running, return info - drop lock before await
        let url_opt = { AW_BASE_URL.lock().unwrap().clone() };
        if let Some(url) = url_opt {
            let port = { AW_PORT.lock().unwrap().unwrap_or(0) };
            let ok = wait_healthy(&url, Duration::from_secs(1)).await;
            if ok {
                return Ok(ServerInfo {
                    base_url: url,
                    port,
                    version: None,
                });
            }
        }

        // Optional dev fallback to system instance
        if use_system_aw() {
            let url = system_aw_base_url();
            if wait_healthy(&url, Duration::from_secs(2)).await {
                *AW_BASE_URL.lock().unwrap() = Some(url.clone());
                *AW_PORT.lock().unwrap() = Some(pick_port());
                return Ok(ServerInfo {
                    base_url: url,
                    port: pick_port(),
                    version: None,
                });
            }
        }

        std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;

        let port = pick_port();
        let base_url = format!("http://127.0.0.1:{}", port);

        // Check if server is already healthy (from previous session or orphaned process)
        let server_already_running = wait_healthy(&base_url, Duration::from_secs(2)).await;
        
        if server_already_running {
            log::info!("Found existing healthy server at {}, will ensure watchers are running", base_url);
            *AW_BASE_URL.lock().unwrap() = Some(base_url.clone());
            *AW_PORT.lock().unwrap() = Some(port);
            // DO NOT return early - continue to watcher verification below
        } else {
            // Server not running, need to start it
            let bin_path = std::fs::canonicalize(&bin_path).unwrap_or(bin_path);
        if !bin_path.exists() {
            log::error!("aw-server missing at {}", bin_path.display());
            return Err(format!(
                "ActivityWatch server not found at: {}",
                bin_path.display()
            ));
        }

        let mut cmd = Command::new(&bin_path);
        if let Some(bin_dir) = bin_path.parent() {
            cmd.current_dir(bin_dir);
        }
        cmd.arg("--port")
            .arg(port.to_string())
            .arg("--host")
            .arg("127.0.0.1")
            // Activity watch debug logs argument
            // .arg("--verbose")
            .stdin(Stdio::null());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        log::info!("spawning aw-server at {}", bin_path.display());
        let child = cmd.spawn().map_err(|e| {
            log::error!("failed to spawn aw-server at {}: {}", bin_path.display(), e);
            e.to_string()
        })?;
        *AW_CHILD.lock().unwrap() = Some(child);

        // Give the process a moment to start before checking health
        log::info!("Waiting for aw-server to start...");
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Wait until healthy (increased timeout to 30 seconds for slow starts)
        log::info!("Checking aw-server health at {} (timeout: 30s)...", base_url);
        if !wait_healthy(&base_url, Duration::from_secs(30)).await {
            log::error!("aw-server failed to become healthy at {} after 30 seconds", base_url);
            return Err(format!(
                "ActivityWatch server failed to become healthy at {} (timeout: 30s)",
                base_url
            ));
        }
        log::info!("aw-server is healthy at {}", base_url);

            *AW_BASE_URL.lock().unwrap() = Some(base_url.clone());
            *AW_PORT.lock().unwrap() = Some(port);
        }
        
        // At this point, server is guaranteed to be healthy (either was running or just started)
        // Now ensure all watchers are running, regardless of how we got here
        log::info!("Server healthy at {}, ensuring watchers are running...", base_url);
        
        match ensure_watchers_running(&app, port) {
            Ok(started) => {
                if !started.is_empty() {
                    log::info!("Started {} watcher(s): {:?}", started.len(), started);
                }
            }
            Err(e) => {
                log::error!("Failed to ensure watchers running: {}", e);
                // Don't fail the entire startup - server is still usable
            }
        }
        
        // Verify watchers are creating data
        match verify_watcher_health(&base_url).await {
            Ok(true) => {
                log::info!("✓ Watcher health verification passed");
            }
            Ok(false) => {
                log::warn!("⚠ Watcher health verification incomplete - buckets may still be initializing");
            }
            Err(e) => {
                log::error!("✗ Watcher health verification failed: {}", e);
            }
        }

        Ok(ServerInfo {
            base_url,
            port,
            version: None,
        })
    }
}

#[tauri::command]
pub async fn start_server(app: AppHandle) -> Result<ServerInfo, String> {
    // Extract needed paths synchronously BEFORE any async operations
    let data_dir = app_data_dir(&app);
    let bin_path: PathBuf = if use_system_aw() {
        PathBuf::from("aw-server")
    } else {
        bundled_aw_server_path(&app)
    };
    let app_clone = app.clone();
    // Drop original app reference before async
    drop(app);

    start_server_internal(data_dir, bin_path, app_clone).await
}

#[tauri::command]
pub async fn get_server_status() -> Result<ServerStatus, String> {
    let url_opt = { AW_BASE_URL.lock().unwrap().clone() };
    if let Some(url) = url_opt {
        let ok = wait_healthy(&url, Duration::from_secs(1)).await;
        if ok {
            // Check watcher status as well
            let watcher_count = {
                let watchers = AW_WATCHERS.lock().unwrap();
                watchers.len()
            };
            
            if watcher_count == 0 {
                log::warn!("Server is healthy but no watchers are tracked (count: {})", watcher_count);
            }
            
            return Ok(ServerStatus {
                healthy: true,
                message: None,
            });
        }
        // Server was running but is no longer healthy
        // Invalidate cached client before clearing server state
        crate::activitywatch::client::invalidate_client();
        
        *AW_BASE_URL.lock().unwrap() = None;
        *AW_PORT.lock().unwrap() = None;
    }
    Ok(ServerStatus {
        healthy: false,
        message: Some("Server offline".into()),
    })
}

#[tauri::command]
pub async fn get_server_info() -> Result<Option<ServerInfo>, String> {
    let url_opt = { AW_BASE_URL.lock().unwrap().clone() };
    let url = match url_opt {
        Some(url) => url,
        None => return Ok(None),
    };
    let port = AW_PORT.lock().unwrap().unwrap_or(0);
    Ok(Some(ServerInfo {
        base_url: url,
        port,
        version: None,
    }))
}

#[tauri::command]
pub async fn start_watcher(app: AppHandle, watcher_name: String) -> Result<(), String> {
    let mut watchers = AW_WATCHERS.lock().unwrap();
    if watchers.contains_key(&watcher_name) {
        return Err(format!("Watcher {} is already running", watcher_name));
    }

    let port = AW_PORT
        .lock()
        .unwrap()
        .ok_or_else(|| "Server not running".to_string())?;
    let watcher_path = bundled_watcher_path(&app, &watcher_name)
        .ok_or_else(|| format!("Watcher {} not found", watcher_name))?;
    let watcher_path = std::fs::canonicalize(&watcher_path).unwrap_or(watcher_path);

    let mut cmd = Command::new(&watcher_path);
    if let Some(bin_dir) = watcher_path.parent() {
        cmd.current_dir(bin_dir);
    }
    cmd.arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--testing") // TODO: remove this for production
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    match cmd.spawn() {
        Ok(child) => {
            log::info!("Started watcher {}", watcher_name);
            watchers.insert(watcher_name.clone(), child);
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to start watcher {}: {}", watcher_name, e);
            Err(format!("Failed to start watcher: {}", e))
        }
    }
}

#[tauri::command]
pub async fn stop_watcher(watcher_name: String) -> Result<(), String> {
    let mut watchers = AW_WATCHERS.lock().unwrap();
    if let Some(mut child) = watchers.remove(&watcher_name) {
        child.kill().ok();
        log::info!("Stopped watcher {}", watcher_name);
        Ok(())
    } else {
        Err(format!("Watcher {} is not running", watcher_name))
    }
}

#[tauri::command]
pub async fn get_watchers_status() -> Result<Vec<WatcherStatus>, String> {
    let mut system = System::new_all();
    system.refresh_all();
    
    let mut watchers = AW_WATCHERS.lock().unwrap();
    let mut statuses = Vec::new();
    let mut dead_watchers = Vec::new();
    
    for name in platform_watcher_names() {
        let mut running = false;
        
        // Check if watcher is in HashMap
        if let Some(child) = watchers.get(name) {
            // Verify PID is actually alive
            let pid = child.id();
            let sysinfo_pid = Pid::from_u32(pid);
            if system.process(sysinfo_pid).is_some() {
                running = true;
            } else {
                log::warn!("Watcher {} (PID: {}) is dead, marking for removal", name, pid);
                dead_watchers.push(name.to_string());
            }
        }
        
        statuses.push(WatcherStatus {
            name: name.to_string(),
            running,
        });
    }
    
    // Remove dead watchers from the HashMap
    for dead_watcher in dead_watchers {
        watchers.remove(&dead_watcher);
        log::info!("Removed dead watcher {} from tracking", dead_watcher);
    }
    
    Ok(statuses)
}

#[tauri::command]
pub async fn stop_server() -> Result<(), String> {
    if let Some(mut child) = AW_CHILD.lock().unwrap().take() {
        child.kill().ok();
    }
    let mut watchers = AW_WATCHERS.lock().unwrap();
    for (name, mut watcher) in watchers.drain() {
        watcher.kill().ok();
        log::info!("Stopped watcher {}", name);
    }
    
    // Invalidate cached AwClient when server stops
    crate::activitywatch::client::invalidate_client();
    
    *AW_BASE_URL.lock().unwrap() = None;
    *AW_PORT.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn get_buckets() -> Result<Vec<crate::activitywatch::types::BucketInfo>, String> {
    // Get the base URL from global state
    let base_url = {
        let url_opt = AW_BASE_URL.lock().unwrap().clone();
        url_opt.ok_or_else(|| "Server not running".to_string())?
    };
    
    // Check if server is healthy before attempting to fetch buckets
    let healthy = wait_healthy(&base_url, Duration::from_secs(1)).await;
    if !healthy {
        return Err("Server is not responding".to_string());
    }
    
    crate::activitywatch::client::fetch_buckets(&base_url).await
}

#[tauri::command]
pub async fn get_bucket_events(
    bucket_id: String,
    limit: Option<u64>,
) -> Result<crate::activitywatch::types::BucketEventsResponse, String> {
    // Get the base URL from global state
    let base_url = {
        let url_opt = AW_BASE_URL.lock().unwrap().clone();
        url_opt.ok_or_else(|| "Server not running".to_string())?
    };
    
    // Check if server is healthy before attempting to fetch events
    let healthy = wait_healthy(&base_url, Duration::from_secs(1)).await;
    if !healthy {
        return Err("Server is not responding".to_string());
    }
    
    crate::activitywatch::client::fetch_bucket_events(&base_url, &bucket_id, limit).await
}

#[tauri::command]
pub async fn get_current_status() -> Result<crate::activitywatch::types::CurrentStatus, String> {
    // Get the base URL from global state
    let base_url = {
        let url_opt = AW_BASE_URL.lock().unwrap().clone();
        url_opt.ok_or_else(|| "Server not running".to_string())?
    };
    
    // Check if server is healthy before attempting to fetch current status
    let healthy = wait_healthy(&base_url, Duration::from_secs(1)).await;
    if !healthy {
        return Err("Server is not responding".to_string());
    }
    
    crate::activitywatch::client::fetch_current_status(&base_url).await
}

#[tauri::command]
pub async fn get_events_by_date_range(
    app: tauri::AppHandle,
    start_time: String,
    end_time: String,
) -> Result<crate::activitywatch::types::DateRangeEventsResponse, String> {
    // Get the base URL from global state
    let base_url = {
        let url_opt = AW_BASE_URL.lock().unwrap().clone();
        url_opt.ok_or_else(|| "Server not running".to_string())?
    };
    
    // Check if server is healthy
    let healthy = wait_healthy(&base_url, Duration::from_secs(1)).await;
    if !healthy {
        return Err("Server is not responding".to_string());
    }
    
    // Parse ISO 8601 timestamps
    let start = chrono::DateTime::parse_from_rfc3339(&start_time)
        .map_err(|e| format!("Invalid start_time format: {}", e))?
        .with_timezone(&chrono::Utc);
    
    let end = chrono::DateTime::parse_from_rfc3339(&end_time)
        .map_err(|e| format!("Invalid end_time format: {}", e))?
        .with_timezone(&chrono::Utc);
    
    let events = crate::activitywatch::client::fetch_events_by_range(&base_url, start, end).await?;
    
    // Send events to collector if enabled
    for event in &events.window_events {
        let _ = crate::collector::bridge::collect_window_event(&app, event);
    }
    for event in &events.afk_events {
        if let Err(e) = crate::collector::bridge::collect_afk_event(&app, event) {
            log::warn!("Failed to collect AFK event for collector: {}", e);
        }
    }
    
    Ok(events)
}

#[tauri::command]
pub async fn get_daily_metrics(
    app: tauri::AppHandle,
    date: String,
) -> Result<crate::activitywatch::types::DailyMetrics, String> {
    // Get the base URL from global state
    let base_url = {
        let url_opt = AW_BASE_URL.lock().unwrap().clone();
        url_opt.ok_or_else(|| "Server not running".to_string())?
    };
    
    // Check if server is healthy
    let healthy = wait_healthy(&base_url, Duration::from_secs(1)).await;
    if !healthy {
        return Err("Server is not responding".to_string());
    }
    
    // Parse date string (YYYY-MM-DD format)
    let naive_date = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date format (expected YYYY-MM-DD): {}", e))?;
    
    let metrics = crate::activitywatch::client::calculate_daily_metrics(&base_url, naive_date).await?;
    
    // Send metrics to collector if enabled
    if let Err(e) = crate::collector::bridge::collect_daily_metrics(&app, &metrics) {
        log::warn!("Failed to collect daily metrics for collector: {}", e);
    }
    
    Ok(metrics)
}

#[tauri::command]
pub async fn get_app_usage_breakdown(
    app: tauri::AppHandle,
    start_time: String,
    end_time: String,
) -> Result<Vec<crate::activitywatch::types::AppUsage>, String> {
    // Get the base URL from global state
    let base_url = {
        let url_opt = AW_BASE_URL.lock().unwrap().clone();
        url_opt.ok_or_else(|| "Server not running".to_string())?
    };
    
    // Check if server is healthy
    let healthy = wait_healthy(&base_url, Duration::from_secs(1)).await;
    if !healthy {
        return Err("Server is not responding".to_string());
    }
    
    // Parse ISO 8601 timestamps
    let start = chrono::DateTime::parse_from_rfc3339(&start_time)
        .map_err(|e| format!("Invalid start_time format: {}", e))?
        .with_timezone(&chrono::Utc);
    
    let end = chrono::DateTime::parse_from_rfc3339(&end_time)
        .map_err(|e| format!("Invalid end_time format: {}", e))?
        .with_timezone(&chrono::Utc);
    
    let app_usage = crate::activitywatch::client::aggregate_app_usage(&base_url, start, end).await?;
    
    // Send app usage to collector if enabled
    if let Err(e) = crate::collector::bridge::collect_app_usage(&app, &app_usage, &end_time) {
        log::warn!("Failed to collect app usage for collector: {}", e);
    }
    
    Ok(app_usage)
}
