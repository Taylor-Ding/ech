//! Process management for ECH Workers
//! Handles spawning, monitoring, and terminating the ech-workers executable

use parking_lot::Mutex;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tauri::{AppHandle, Emitter};

use crate::config::Server;

/// Process manager state
pub struct ProcessManager {
    child: Mutex<Option<Child>>,
    is_running: AtomicBool,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            is_running: AtomicBool::new(false),
        }
    }
    
    /// Check if process is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
    
    /// Find the ech-workers executable
    fn find_executable() -> Option<PathBuf> {
        let exe_name = if cfg!(target_os = "windows") {
            "ech-workers.exe"
        } else {
            "ech-workers"
        };
        
        // Get the directory where the app is located
        let app_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        
        // Possible locations to search
        let mut search_paths = Vec::new();
        
        // 1. App bundle directory (for packaged apps)
        if let Some(dir) = &app_dir {
            search_paths.push(dir.join(exe_name));
            // On macOS, check Resources folder
            if cfg!(target_os = "macos") {
                search_paths.push(dir.join("../Resources").join(exe_name));
            }
        }
        
        // 2. Parent directory (for development - ech-gui is inside ech-wk)
        if let Some(dir) = &app_dir {
            search_paths.push(dir.join("../../..").join(exe_name));
            search_paths.push(dir.join("../../../..").join(exe_name));
        }
        
        // 3. Current working directory
        search_paths.push(PathBuf::from(exe_name));
        
        // 4. Parent of current directory
        search_paths.push(PathBuf::from("..").join(exe_name));
        
        for path in search_paths {
            if let Ok(canonical) = path.canonicalize() {
                if canonical.exists() {
                    // On Unix, check if executable
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(meta) = std::fs::metadata(&canonical) {
                            if meta.permissions().mode() & 0o111 != 0 {
                                return Some(canonical);
                            }
                        }
                    }
                    // On Windows, just check existence
                    #[cfg(windows)]
                    {
                        return Some(canonical);
                    }
                }
            }
        }
        
        // 5. Try PATH
        if let Ok(path) = which::which(exe_name) {
            return Some(path);
        }
        
        None
    }
    
    /// Start the ech-workers process
    pub fn start(&self, server: &Server, app_handle: AppHandle) -> Result<(), String> {
        if self.is_running() {
            return Err("进程已在运行".to_string());
        }
        
        let exe_path = Self::find_executable()
            .ok_or_else(|| "找不到 ech-workers 可执行文件".to_string())?;
        
        // Build command arguments
        let mut cmd = Command::new(&exe_path);
        
        if !server.server.is_empty() {
            cmd.args(["-f", &server.server]);
        }
        if !server.listen.is_empty() {
            cmd.args(["-l", &server.listen]);
        }
        if !server.token.is_empty() {
            cmd.args(["-token", &server.token]);
        }
        if !server.ip.is_empty() {
            cmd.args(["-ip", &server.ip]);
        }
        if !server.dns.is_empty() && server.dns != "dns.alidns.com/dns-query" {
            cmd.args(["-dns", &server.dns]);
        }
        if !server.ech.is_empty() && server.ech != "cloudflare-ech.com" {
            cmd.args(["-ech", &server.ech]);
        }
        if !server.routing_mode.is_empty() {
            cmd.args(["-routing", &server.routing_mode]);
        }
        
        // Configure process
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        
        // On Windows, hide console window
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("启动进程失败: {}", e))?;
        
        self.is_running.store(true, Ordering::SeqCst);
        
        // Stream stdout to frontend
        if let Some(stdout) = child.stdout.take() {
            let app_handle_clone = app_handle.clone();
            let is_running = Arc::new(AtomicBool::new(true));
            let is_running_clone = is_running.clone();
            
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if !is_running_clone.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(line) = line {
                        let _ = app_handle_clone.emit("log-output", line);
                    }
                }
            });
        }
        
        // Store child process
        *self.child.lock() = Some(child);
        
        // Emit start event
        let _ = app_handle.emit("process-started", ());
        
        Ok(())
    }
    
    /// Stop the running process
    pub fn stop(&self, app_handle: &AppHandle) -> Result<(), String> {
        self.is_running.store(false, Ordering::SeqCst);
        
        let mut child_guard = self.child.lock();
        if let Some(mut child) = child_guard.take() {
            // Try graceful termination first
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGTERM);
                }
            }
            
            #[cfg(windows)]
            {
                let _ = child.kill();
            }
            
            // Wait a bit, then force kill if needed
            match child.try_wait() {
                Ok(Some(_)) => {}
                _ => {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
        
        let _ = app_handle.emit("process-stopped", ());
        Ok(())
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(mut child) = self.child.lock().take() {
            let _ = child.kill();
        }
    }
}
