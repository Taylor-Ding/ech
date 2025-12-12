//! Tauri commands exposed to the frontend
//! These are callable from JavaScript via invoke()

use crate::config::{ConfigManager, Server};
use crate::process::ProcessManager;
use crate::proxy;
use once_cell::sync::Lazy;
use tauri::AppHandle;

// Global managers
static CONFIG_MANAGER: Lazy<ConfigManager> = Lazy::new(ConfigManager::new);
static PROCESS_MANAGER: Lazy<ProcessManager> = Lazy::new(ProcessManager::new);

// ============ Server Commands ============

#[tauri::command]
pub fn get_servers() -> Vec<Server> {
    CONFIG_MANAGER.get_servers()
}

#[tauri::command]
pub fn get_current_server() -> Option<Server> {
    CONFIG_MANAGER.get_current_server()
}

#[tauri::command]
pub fn get_current_server_id() -> Option<String> {
    CONFIG_MANAGER.get_current_server_id()
}

#[tauri::command]
pub fn set_current_server(id: String) -> Result<(), String> {
    CONFIG_MANAGER.set_current_server(&id);
    CONFIG_MANAGER.save()
}

#[tauri::command]
pub fn add_server(name: String) -> Result<Server, String> {
    let mut server = Server::default();
    server.name = name;
    server.id = String::new(); // Will be generated
    
    let id = CONFIG_MANAGER.add_server(server);
    CONFIG_MANAGER.save()?;
    
    CONFIG_MANAGER
        .get_servers()
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "添加服务器失败".to_string())
}

#[tauri::command]
pub fn update_server(server: Server) -> Result<(), String> {
    if CONFIG_MANAGER.update_server(server) {
        CONFIG_MANAGER.save()
    } else {
        Err("服务器不存在".to_string())
    }
}

#[tauri::command]
pub fn delete_server(id: String) -> Result<(), String> {
    // Don't allow deleting if only one server
    if CONFIG_MANAGER.get_servers().len() <= 1 {
        return Err("至少需要保留一个服务器配置".to_string());
    }
    
    if CONFIG_MANAGER.delete_server(&id) {
        CONFIG_MANAGER.save()
    } else {
        Err("服务器不存在".to_string())
    }
}

#[tauri::command]
pub fn rename_server(id: String, new_name: String) -> Result<(), String> {
    if CONFIG_MANAGER.rename_server(&id, &new_name) {
        CONFIG_MANAGER.save()
    } else {
        Err("服务器不存在".to_string())
    }
}

// ============ Process Commands ============

#[tauri::command]
pub fn start_process(app_handle: AppHandle) -> Result<String, String> {
    let server = CONFIG_MANAGER
        .get_current_server()
        .ok_or_else(|| "没有选择服务器".to_string())?;
    
    if server.server.is_empty() {
        return Err("请输入服务地址".to_string());
    }
    if server.listen.is_empty() {
        return Err("请输入监听地址".to_string());
    }
    
    PROCESS_MANAGER.start(&server, app_handle)?;
    
    Ok(format!("已启动服务器: {}", server.name))
}

#[tauri::command]
pub fn stop_process(app_handle: AppHandle) -> Result<String, String> {
    PROCESS_MANAGER.stop(&app_handle)?;
    Ok("进程已停止".to_string())
}

#[tauri::command]
pub fn is_process_running() -> bool {
    PROCESS_MANAGER.is_running()
}

// ============ Proxy Commands ============

#[tauri::command]
pub fn set_system_proxy(enabled: bool) -> Result<String, String> {
    let listen = CONFIG_MANAGER
        .get_current_server()
        .map(|s| s.listen)
        .unwrap_or_else(|| "127.0.0.1:30000".to_string());
    
    proxy::set_system_proxy(enabled, &listen)
}

#[tauri::command]
pub fn get_proxy_status() -> bool {
    proxy::get_proxy_status()
}

// ============ Utility Commands ============

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
