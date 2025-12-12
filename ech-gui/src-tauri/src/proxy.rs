//! System proxy control for ECH Workers
//! Supports macOS (networksetup) and Windows (registry)

/// Set system SOCKS proxy
pub fn set_system_proxy(enabled: bool, listen_addr: &str) -> Result<String, String> {
    if cfg!(target_os = "macos") {
        set_macos_proxy(enabled, listen_addr)
    } else if cfg!(target_os = "windows") {
        set_windows_proxy(enabled, listen_addr)
    } else {
        Err("Linux 暂不支持自动设置系统代理".to_string())
    }
}

/// Get current proxy status
pub fn get_proxy_status() -> bool {
    if cfg!(target_os = "macos") {
        get_macos_proxy_status()
    } else if cfg!(target_os = "windows") {
        get_windows_proxy_status()
    } else {
        false
    }
}

// ============ macOS Implementation ============

#[cfg(target_os = "macos")]
fn set_macos_proxy(enabled: bool, listen_addr: &str) -> Result<String, String> {
    use std::process::Command;
    // Parse address
    let (host, port) = parse_listen_addr(listen_addr)?;
    
    // Get network services
    let output = Command::new("networksetup")
        .arg("-listallnetworkservices")
        .output()
        .map_err(|e| format!("获取网络服务列表失败: {}", e))?;
    
    let services_output = String::from_utf8_lossy(&output.stdout);
    let services: Vec<&str> = services_output
        .lines()
        .skip(1) // Skip header
        .filter(|s| !s.starts_with('*') && !s.is_empty())
        .collect();
    
    let bypass_domains = vec![
        "localhost", "127.*", "10.*", 
        "172.16.*", "172.17.*", "172.18.*", "172.19.*",
        "172.20.*", "172.21.*", "172.22.*", "172.23.*",
        "172.24.*", "172.25.*", "172.26.*", "172.27.*",
        "172.28.*", "172.29.*", "172.30.*", "172.31.*",
        "192.168.*", "*.local", "169.254.*"
    ];
    
    for service in &services {
        if enabled {
            // Set SOCKS proxy
            let _ = Command::new("networksetup")
                .args(["-setsocksfirewallproxy", service, &host, &port])
                .output();
            
            // Set bypass domains
            let mut args = vec!["-setsocksfirewallproxybypassdomains", service];
            args.extend(bypass_domains.iter().copied());
            let _ = Command::new("networksetup")
                .args(&args)
                .output();
            
            // Enable proxy
            let _ = Command::new("networksetup")
                .args(["-setsocksfirewallproxystate", service, "on"])
                .output();
        } else {
            // Disable proxy
            let _ = Command::new("networksetup")
                .args(["-setsocksfirewallproxystate", service, "off"])
                .output();
        }
    }
    
    Ok(if enabled {
        format!("已设置系统代理: {}:{}", host, port)
    } else {
        "已关闭系统代理".to_string()
    })
}

#[cfg(target_os = "macos")]
fn get_macos_proxy_status() -> bool {
    use std::process::Command;

    // Check Wi-Fi service (most common)
    let output = Command::new("networksetup")
        .args(["-getsocksfirewallproxy", "Wi-Fi"])
        .output();
    
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.contains("Enabled: Yes")
    } else {
        false
    }
}

#[cfg(not(target_os = "macos"))]
fn set_macos_proxy(_enabled: bool, _listen_addr: &str) -> Result<String, String> {
    Err("Not macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
fn get_macos_proxy_status() -> bool {
    false
}

// ============ Windows Implementation ============

#[cfg(target_os = "windows")]
fn set_windows_proxy(enabled: bool, listen_addr: &str) -> Result<String, String> {
    use winreg::enums::*;
    use winreg::RegKey;
    
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            KEY_SET_VALUE,
        )
        .map_err(|e| format!("打开注册表失败: {}", e))?;
    
    if enabled {
        let (host, port) = parse_listen_addr(listen_addr)?;
        let proxy_server = format!("{}:{}", host, port);
        
        key.set_value("ProxyServer", &proxy_server)
            .map_err(|e| format!("设置代理服务器失败: {}", e))?;
        key.set_value("ProxyEnable", &1u32)
            .map_err(|e| format!("启用代理失败: {}", e))?;
        
        // Set bypass list
        let bypass = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;<local>";
        key.set_value("ProxyOverride", &bypass)
            .map_err(|e| format!("设置绕过列表失败: {}", e))?;
        
        // Notify system of changes
        notify_windows_proxy_change();
        
        Ok(format!("已设置系统代理: {}", proxy_server))
    } else {
        key.set_value("ProxyEnable", &0u32)
            .map_err(|e| format!("禁用代理失败: {}", e))?;
        
        notify_windows_proxy_change();
        
        Ok("已关闭系统代理".to_string())
    }
}

#[cfg(target_os = "windows")]
fn notify_windows_proxy_change() {
    #[cfg(target_arch = "x86_64")]
    {
        use std::ptr::null_mut;
        use winapi::um::wininet::{InternetSetOptionW, INTERNET_OPTION_SETTINGS_CHANGED, INTERNET_OPTION_REFRESH};

        unsafe {
            InternetSetOptionW(null_mut(), INTERNET_OPTION_SETTINGS_CHANGED, null_mut(), 0);
            InternetSetOptionW(null_mut(), INTERNET_OPTION_REFRESH, null_mut(), 0);
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        use windows::Win32::Networking::WinInet::*;

        unsafe {
            let _ = InternetSetOptionW(None, INTERNET_OPTION_SETTINGS_CHANGED, None, 0);
            let _ = InternetSetOptionW(None, INTERNET_OPTION_REFRESH, None, 0);
        }
    }
}

#[cfg(target_os = "windows")]
fn get_windows_proxy_status() -> bool {
    use winreg::enums::*;
    use winreg::RegKey;
    
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings") {
        if let Ok(enabled) = key.get_value::<u32, _>("ProxyEnable") {
            return enabled == 1;
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn set_windows_proxy(_enabled: bool, _listen_addr: &str) -> Result<String, String> {
    Err("Not Windows".to_string())
}

#[cfg(not(target_os = "windows"))]
fn get_windows_proxy_status() -> bool {
    false
}

// ============ Helpers ============

fn parse_listen_addr(addr: &str) -> Result<(String, String), String> {
    if let Some(idx) = addr.rfind(':') {
        let host = addr[..idx].to_string();
        let port = addr[idx + 1..].to_string();
        Ok((host, port))
    } else {
        Ok(("127.0.0.1".to_string(), addr.to_string()))
    }
}
