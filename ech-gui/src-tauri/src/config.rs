//! Configuration management for ECH Workers GUI
//! Handles server configs, persistence, and cross-platform config paths

use dirs;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Single server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub listen: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub dns: String,
    #[serde(default)]
    pub ech: String,
    #[serde(default = "default_routing_mode")]
    pub routing_mode: String,
}

fn default_routing_mode() -> String {
    "bypass_cn".to_string()
}

impl Default for Server {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "默认服务器".to_string(),
            server: "example.com:443".to_string(),
            listen: "127.0.0.1:30000".to_string(),
            token: String::new(),
            ip: "saas.sin.fan".to_string(),
            dns: "dns.alidns.com/dns-query".to_string(),
            ech: "cloudflare-ech.com".to_string(),
            routing_mode: "bypass_cn".to_string(),
        }
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub servers: Vec<Server>,
    pub current_server_id: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_server = Server::default();
        Self {
            current_server_id: Some(default_server.id.clone()),
            servers: vec![default_server],
        }
    }
}

/// Configuration manager with thread-safe access
pub struct ConfigManager {
    config: RwLock<AppConfig>,
    config_path: PathBuf,
}

impl ConfigManager {
    /// Create a new ConfigManager and load existing config
    pub fn new() -> Self {
        let config_dir = Self::get_config_dir();
        fs::create_dir_all(&config_dir).ok();
        
        let config_path = config_dir.join("config.json");
        let config = Self::load_from_path(&config_path).unwrap_or_default();
        
        Self {
            config: RwLock::new(config),
            config_path,
        }
    }
    
    /// Get platform-specific config directory
    fn get_config_dir() -> PathBuf {
        if cfg!(target_os = "windows") {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("ECHWorkersClient")
        } else if cfg!(target_os = "macos") {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Library")
                .join("Application Support")
                .join("ECHWorkersClient")
        } else {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("ECHWorkersClient")
        }
    }
    
    /// Load config from file path
    fn load_from_path(path: &PathBuf) -> Option<AppConfig> {
        if path.exists() {
            let content = fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }
    
    /// Save current config to file
    pub fn save(&self) -> Result<(), String> {
        let config = self.config.read();
        let json = serde_json::to_string_pretty(&*config)
            .map_err(|e| format!("序列化配置失败: {}", e))?;
        fs::write(&self.config_path, json)
            .map_err(|e| format!("保存配置失败: {}", e))?;
        Ok(())
    }
    
    /// Get all servers
    pub fn get_servers(&self) -> Vec<Server> {
        self.config.read().servers.clone()
    }
    
    /// Get current server
    pub fn get_current_server(&self) -> Option<Server> {
        let config = self.config.read();
        if let Some(id) = &config.current_server_id {
            config.servers.iter().find(|s| &s.id == id).cloned()
        } else {
            config.servers.first().cloned()
        }
    }
    
    /// Get current server ID
    pub fn get_current_server_id(&self) -> Option<String> {
        self.config.read().current_server_id.clone()
    }
    
    /// Set current server by ID
    pub fn set_current_server(&self, id: &str) {
        let mut config = self.config.write();
        if config.servers.iter().any(|s| s.id == id) {
            config.current_server_id = Some(id.to_string());
        }
    }
    
    /// Add a new server
    pub fn add_server(&self, mut server: Server) -> String {
        let mut config = self.config.write();
        if server.id.is_empty() {
            server.id = Uuid::new_v4().to_string();
        }
        let id = server.id.clone();
        config.servers.push(server);
        config.current_server_id = Some(id.clone());
        id
    }
    
    /// Update existing server
    pub fn update_server(&self, server: Server) -> bool {
        let mut config = self.config.write();
        if let Some(existing) = config.servers.iter_mut().find(|s| s.id == server.id) {
            *existing = server;
            true
        } else {
            false
        }
    }
    
    /// Delete server by ID
    pub fn delete_server(&self, id: &str) -> bool {
        let mut config = self.config.write();
        let initial_len = config.servers.len();
        config.servers.retain(|s| s.id != id);
        
        if config.servers.is_empty() {
            // Always keep at least one server
            config.servers.push(Server::default());
        }
        
        // Update current server if deleted
        if config.current_server_id.as_deref() == Some(id) {
            config.current_server_id = config.servers.first().map(|s| s.id.clone());
        }
        
        config.servers.len() < initial_len
    }
    
    /// Rename server
    pub fn rename_server(&self, id: &str, new_name: &str) -> bool {
        let mut config = self.config.write();
        if let Some(server) = config.servers.iter_mut().find(|s| s.id == id) {
            server.name = new_name.to_string();
            true
        } else {
            false
        }
    }
}
