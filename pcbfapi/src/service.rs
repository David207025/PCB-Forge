use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

/// Service identifier across macOS launchd and Linux systemd
pub const SERVICE_NAME: &str = "com.pcb-forge.pcbfapi";

/// Ensures that the daemon service is registered on the system so it runs on boot.
pub fn ensure_service_installed() {
  if !is_service_installed() {
    println!("⚙️  First run detected: Registering pcbfapi as a persistent background service...");
    match install_service() {
      Ok(msg) => println!("✓ {}", msg),
      Err(err) => eprintln!("⚠️ Could not auto-register service: {}", err),
    }
  }
}

/// Checks whether the background service configuration exists on this machine.
pub fn is_service_installed() -> bool {
  #[cfg(target_os = "macos")]
  {
    if let Some(home) = dirs::home_dir() {
      let plist_path = home.join("Library/LaunchAgents").join(format!("{}.plist", SERVICE_NAME));
      return plist_path.exists();
    }
    false
  }
  #[cfg(target_os = "linux")]
  {
    if let Some(home) = dirs::home_dir() {
      let service_path = home.join(".config/systemd/user/pcbfapi.service");
      return service_path.exists();
    }
    false
  }
  #[cfg(target_os = "windows")]
  {
    // Check HKCU Run key
    let output = Command::new("reg")
      .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "pcbfapi"])
      .output();
    matches!(output, Ok(o) if o.status.success())
  }
  #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
  {
    false
  }
}

/// Installs and activates pcbfapi as a background service/daemon that runs on boot.
pub fn install_service() -> Result<String, String> {
  let exe_path = std::env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;
  let exe_path_str = exe_path.to_string_lossy().to_string();

  let home_dir = dirs::home_dir().ok_or_else(|| "Could not locate user home directory".to_string())?;
  let logs_dir = home_dir.join(".pcb-forge").join("logs");
  let _ = fs::create_dir_all(&logs_dir);

  #[cfg(target_os = "macos")]
  {
    let launch_agents_dir = home_dir.join("Library/LaunchAgents");
    let _ = fs::create_dir_all(&launch_agents_dir);

    let plist_path = launch_agents_dir.join(format!("{}.plist", SERVICE_NAME));
    let stdout_log = logs_dir.join("pcbfapi.log");
    let stderr_log = logs_dir.join("pcbfapi.err.log");

    let plist_content = format!(
      r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
    </dict>
</dict>
</plist>
"#,
      SERVICE_NAME,
      exe_path_str,
      stdout_log.to_string_lossy(),
      stderr_log.to_string_lossy()
    );

    fs::write(&plist_path, plist_content)
      .map_err(|e| format!("Failed to write LaunchAgent plist to {}: {}", plist_path.display(), e))?;

    // Unload any existing instance first
    let _ = Command::new("launchctl")
      .args(["unload", "-w", plist_path.to_str().unwrap()])
      .output();

    // Load and enable the LaunchAgent
    let load_res = Command::new("launchctl")
      .args(["load", "-w", plist_path.to_str().unwrap()])
      .output()
      .map_err(|e| format!("Failed to invoke launchctl load: {}", e))?;

    if !load_res.status.success() {
      let stderr = String::from_utf8_lossy(&load_res.stderr);
      return Err(format!("launchctl load failed: {}", stderr));
    }

    Ok(format!(
      "macOS LaunchAgent registered at {} and loaded. pcbfapi will run in the background on boot.",
      plist_path.display()
    ))
  }

  #[cfg(target_os = "linux")]
  {
    let systemd_dir = home_dir.join(".config/systemd/user");
    let _ = fs::create_dir_all(&systemd_dir);

    let service_path = systemd_dir.join("pcbfapi.service");
    let service_content = format!(
      r#"[Unit]
Description=PCB Forge API Background Daemon
After=network.target

[Service]
Type=simple
ExecStart={} run
Restart=always
RestartSec=3
Environment="PATH=/usr/local/bin:/usr/bin:/bin"
StandardOutput=append:{}/pcbfapi.log
StandardError=append:{}/pcbfapi.err.log

[Install]
WantedBy=default.target
"#,
      exe_path_str,
      logs_dir.to_string_lossy(),
      logs_dir.to_string_lossy()
    );

    fs::write(&service_path, service_content)
      .map_err(|e| format!("Failed to write systemd service file: {}", e))?;

    let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
    let enable_res = Command::new("systemctl")
      .args(["--user", "enable", "--now", "pcbfapi.service"])
      .output()
      .map_err(|e| format!("Failed to enable systemd service: {}", e))?;

    if !enable_res.status.success() {
      return Err(format!("systemctl enable failed: {}", String::from_utf8_lossy(&enable_res.stderr)));
    }

    Ok("systemd user service registered and started. pcbfapi will run in the background on boot.".to_string())
  }

  #[cfg(target_os = "windows")]
  {
    let cmd = format!("\"{}\" run", exe_path_str);
    let reg_res = Command::new("reg")
      .args([
        "add",
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        "/v",
        "pcbfapi",
        "/t",
        "REG_SZ",
        "/d",
        &cmd,
        "/f",
      ])
      .output()
      .map_err(|e| format!("Failed to write Windows registry run key: {}", e))?;

    if !reg_res.status.success() {
      return Err(format!("reg add failed: {}", String::from_utf8_lossy(&reg_res.stderr)));
    }

    Ok("Windows startup registry entry configured. pcbfapi will launch on boot.".to_string())
  }

  #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
  {
    Err("Background service registration is not supported on this operating system.".to_string())
  }
}

/// Uninstalls and disables the background service.
pub fn uninstall_service() -> Result<String, String> {
  let home_dir = dirs::home_dir().ok_or_else(|| "Could not locate user home directory".to_string())?;

  #[cfg(target_os = "macos")]
  {
    let plist_path = home_dir.join("Library/LaunchAgents").join(format!("{}.plist", SERVICE_NAME));
    if plist_path.exists() {
      let _ = Command::new("launchctl")
        .args(["unload", "-w", plist_path.to_str().unwrap()])
        .output();
      let _ = fs::remove_file(&plist_path);
      Ok(format!("Uninstalled LaunchAgent: {}", plist_path.display()))
    } else {
      Ok("LaunchAgent plist does not exist.".to_string())
    }
  }

  #[cfg(target_os = "linux")]
  {
    let service_path = home_dir.join(".config/systemd/user/pcbfapi.service");
    let _ = Command::new("systemctl").args(["--user", "disable", "--now", "pcbfapi.service"]).output();
    if service_path.exists() {
      let _ = fs::remove_file(&service_path);
    }
    let _ = Command::new("systemctl").args(["--user", "daemon-reload"]).output();
    Ok("Uninstalled systemd user service.".to_string())
  }

  #[cfg(target_os = "windows")]
  {
    let _ = Command::new("reg")
      .args(["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "pcbfapi", "/f"])
      .output();
    Ok("Removed Windows startup registry entry.".to_string())
  }

  #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
  {
    Err("Unsupported OS".to_string())
  }
}

/// Starts the background service if installed.
pub fn start_service() -> Result<String, String> {
  if !is_service_installed() {
    return install_service();
  }

  #[cfg(target_os = "macos")]
  {
    let home = dirs::home_dir().unwrap();
    let plist_path = home.join("Library/LaunchAgents").join(format!("{}.plist", SERVICE_NAME));
    let _ = Command::new("launchctl")
      .args(["load", "-w", plist_path.to_str().unwrap()])
      .output();
    let _ = Command::new("launchctl")
      .args(["start", SERVICE_NAME])
      .output();
    Ok("Started LaunchAgent service com.pcb-forge.pcbfapi".to_string())
  }

  #[cfg(target_os = "linux")]
  {
    let out = Command::new("systemctl")
      .args(["--user", "start", "pcbfapi.service"])
      .output()
      .map_err(|e| format!("Failed to start systemd service: {}", e))?;
    if out.status.success() {
      Ok("Started pcbfapi systemd service.".to_string())
    } else {
      Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
  }

  #[cfg(target_os = "windows")]
  {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    Command::new(exe_path)
      .arg("run")
      .spawn()
      .map_err(|e| format!("Failed to spawn pcbfapi: {}", e))?;
    Ok("Spawned pcbfapi background process.".to_string())
  }

  #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
  {
    Err("Unsupported OS".to_string())
  }
}

/// Stops the background service.
pub fn stop_service() -> Result<String, String> {
  #[cfg(target_os = "macos")]
  {
    if let Some(home) = dirs::home_dir() {
      let plist_path = home.join("Library/LaunchAgents").join(format!("{}.plist", SERVICE_NAME));
      if plist_path.exists() {
        let _ = Command::new("launchctl")
          .args(["unload", "-w", plist_path.to_str().unwrap()])
          .output();
      }
    }
    let _ = Command::new("launchctl").args(["stop", SERVICE_NAME]).output();
    Ok("Stopped LaunchAgent service com.pcb-forge.pcbfapi".to_string())
  }

  #[cfg(target_os = "linux")]
  {
    let out = Command::new("systemctl")
      .args(["--user", "stop", "pcbfapi.service"])
      .output()
      .map_err(|e| format!("Failed to stop systemd service: {}", e))?;
    if out.status.success() {
      Ok("Stopped systemd service.".to_string())
    } else {
      Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
  }

  #[cfg(target_os = "windows")]
  {
    let _ = Command::new("taskkill").args(["/IM", "pcbfapi.exe", "/F"]).output();
    Ok("Stopped pcbfapi process on Windows.".to_string())
  }

  #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
  {
    Err("Unsupported OS".to_string())
  }
}

/// Checks the status of the service (both service manager status and HTTP responsiveness).
pub fn status_service() -> String {
  let is_installed = is_service_installed();
  let addr: SocketAddr = "127.0.0.1:47210".parse().unwrap();
  let http_running = TcpStream::connect_timeout(&addr, Duration::from_millis(600)).is_ok();

  format!(
    "Service Status:\n  Installed: {}\n  Daemon Running (127.0.0.1:47210): {}",
    if is_installed { "YES (auto-starts on boot)" } else { "NO" },
    if http_running { "ACTIVE (Healthy)" } else { "STOPPED / UNREACHABLE" }
  )
}
