use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KiCadAccessReport {
  pub has_access: bool,
  pub kicad_cli_found: bool,
  pub kicad_cli_path: Option<String>,
  pub kicad_version: Option<String>,
  pub app_bundle_found: bool,
  pub app_bundle_path: Option<String>,
  pub preferences_accessible: bool,
  pub preferences_path: Option<String>,
  pub config_home: String,
  pub has_quarantine: bool,
  pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PcbForgeConfig {
  kicad_cli_path: Option<String>,
}

fn get_config_file_path() -> PathBuf {
  let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
  home.join(".pcb-forge").join("config.json")
}

fn load_saved_kicad_cli_path() -> Option<PathBuf> {
  let config_path = get_config_file_path();
  if let Ok(content) = fs::read_to_string(&config_path) {
    if let Ok(cfg) = serde_json::from_str::<PcbForgeConfig>(&content) {
      if let Some(p) = cfg.kicad_cli_path {
        let path = PathBuf::from(p);
        if path.exists() {
          return Some(path);
        }
      }
    }
  }
  None
}

pub fn save_kicad_cli_path(path: &Path) {
  let config_path = get_config_file_path();
  if let Some(parent) = config_path.parent() {
    let _ = fs::create_dir_all(parent);
  }

  let mut cfg = if let Ok(content) = fs::read_to_string(&config_path) {
    serde_json::from_str::<PcbForgeConfig>(&content).unwrap_or_default()
  } else {
    PcbForgeConfig::default()
  };

  cfg.kicad_cli_path = Some(path.to_string_lossy().to_string());
  if let Ok(json) = serde_json::to_string_pretty(&cfg) {
    let _ = fs::write(&config_path, json);
  }
}

/// Returns the dedicated, secure KiCad config directory for PCB Forge.
/// This prevents permission conflicts and TCC sandbox denials on ~/Library/Preferences/kicad.
pub fn get_kicad_config_home() -> PathBuf {
  let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
  let dir = home.join(".pcb-forge").join("kicad-config");
  if !dir.exists() {
    let _ = fs::create_dir_all(&dir);
  }
  sync_existing_kicad_tables(&dir);
  dir
}

/// If user has existing footprint or symbol tables in their default OS KiCad directory,
/// ensure they are accessible in the dedicated config directory.
fn sync_existing_kicad_tables(target_dir: &Path) {
  let source_dirs: Vec<PathBuf> = if cfg!(target_os = "macos") {
    let home = dirs::home_dir().unwrap_or_default();
    vec![
      home.join("Library/Preferences/kicad/10.0"),
      home.join("Library/Preferences/kicad/9.0"),
      home.join("Library/Preferences/kicad/8.0"),
      home.join("Library/Preferences/kicad/7.0"),
    ]
  } else if cfg!(target_os = "windows") {
    let appdata = dirs::config_dir().unwrap_or_default();
    vec![
      appdata.join("kicad/8.0"),
      appdata.join("kicad/7.0"),
    ]
  } else {
    let home = dirs::home_dir().unwrap_or_default();
    vec![
      home.join(".config/kicad/8.0"),
      home.join(".config/kicad/7.0"),
    ]
  };

  for src in source_dirs {
    if src.is_dir() {
      for file_name in &["fp-lib-table", "sym-lib-table", "kicad_common.json"] {
        let src_file = src.join(file_name);
        let dest_file = target_dir.join(file_name);
        if src_file.exists() && !dest_file.exists() {
          let _ = fs::copy(&src_file, &dest_file);
        }
      }
    }
  }
}

/// Detects the kicad-cli binary executable.
pub fn detect_kicad_cli() -> Option<PathBuf> {
  // 1. Check explicitly saved user path
  if let Some(saved) = load_saved_kicad_cli_path() {
    return Some(saved);
  }

  // 2. Check PATH
  if let Ok(path) = which::which("kicad-cli") {
    return Some(path);
  }

  // 3. Fall back to standard OS install paths
  let candidate_paths: Vec<PathBuf> = if cfg!(target_os = "macos") {
    vec![
      PathBuf::from("/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli"),
      PathBuf::from("/Applications/KiCad.app/Contents/MacOS/kicad-cli"),
      PathBuf::from("/usr/local/bin/kicad-cli"),
      PathBuf::from("/opt/homebrew/bin/kicad-cli"),
    ]
  } else if cfg!(target_os = "windows") {
    vec![
      PathBuf::from(r"C:\Program Files\KiCad\9.0\bin\kicad-cli.exe"),
      PathBuf::from(r"C:\Program Files\KiCad\8.0\bin\kicad-cli.exe"),
      PathBuf::from(r"C:\Program Files\KiCad\7.0\bin\kicad-cli.exe"),
      PathBuf::from(r"C:\Program Files\KiCad\bin\kicad-cli.exe"),
      PathBuf::from(r"C:\Program Files (x86)\KiCad\8.0\bin\kicad-cli.exe"),
    ]
  } else {
    vec![
      PathBuf::from("/usr/bin/kicad-cli"),
      PathBuf::from("/usr/local/bin/kicad-cli"),
      PathBuf::from("/var/lib/flatpak/exports/bin/org.kicad.KiCad"),
    ]
  };

  for path in candidate_paths {
    if path.exists() {
      return Some(path);
    }
  }

  None
}

/// Detects the KiCad application bundle on macOS.
pub fn detect_kicad_app() -> Option<PathBuf> {
  if cfg!(target_os = "macos") {
    let paths = [
      PathBuf::from("/Applications/KiCad/KiCad.app"),
      PathBuf::from("/Applications/KiCad.app"),
    ];
    for p in &paths {
      if p.exists() {
        return Some(p.clone());
      }
    }
  }
  None
}

/// Configures environment variables on a Command before running kicad-cli.
pub fn configure_kicad_command(cmd: &mut Command) {
  let config_home = get_kicad_config_home();
  cmd.env("KICAD_CONFIG_HOME", &config_home);

  // Extend PATH so all standard binaries and homebrew tools are in PATH
  let current_path = std::env::var("PATH").unwrap_or_default();
  let extra_paths = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";
  let combined_path = format!("{}:{}", extra_paths, current_path);
  cmd.env("PATH", combined_path);

  #[cfg(target_os = "macos")]
  {
    if let Some(app) = detect_kicad_app() {
      let shared_support = app.join("Contents/SharedSupport");
      if shared_support.exists() {
        cmd.env("KICAD_SYMBOL_DIR", shared_support.join("symbols"));
        cmd.env("KICAD_FOOTPRINT_DIR", shared_support.join("footprints"));
        cmd.env("KICAD_TEMPLATE_DIR", shared_support.join("template"));
      }
    }
  }
}

/// Checks whether macOS quarantine extended attribute is present on the path.
pub fn check_quarantine(path: &Path) -> bool {
  #[cfg(target_os = "macos")]
  {
    let out = Command::new("xattr")
      .args(["-p", "com.apple.quarantine", path.to_str().unwrap_or_default()])
      .output();
    matches!(out, Ok(o) if o.status.success() && !o.stdout.is_empty())
  }
  #[cfg(not(target_os = "macos"))]
  {
    let _ = path;
    false
  }
}

/// Clears macOS quarantine attribute on the given path if present.
pub fn remove_quarantine(path: &Path) -> bool {
  #[cfg(target_os = "macos")]
  {
    let out = Command::new("xattr")
      .args(["-d", "com.apple.quarantine", path.to_str().unwrap_or_default()])
      .output();
    matches!(out, Ok(o) if o.status.success())
  }
  #[cfg(not(target_os = "macos"))]
  {
    let _ = path;
    false
  }
}

/// Performs a comprehensive check of KiCad availability, permissions, and accessibility.
pub fn check_kicad_access() -> KiCadAccessReport {
  let cli_path_opt = detect_kicad_cli();
  let app_bundle_opt = detect_kicad_app();
  let config_home = get_kicad_config_home();

  let has_quarantine = if let Some(ref app) = app_bundle_opt {
    check_quarantine(app)
  } else {
    false
  };

  let preferences_path = if cfg!(target_os = "macos") {
    dirs::home_dir().map(|h| h.join("Library/Preferences/kicad"))
  } else {
    None
  };

  let preferences_accessible = if let Some(ref p) = preferences_path {
    p.is_dir() && fs::read_dir(p).is_ok()
  } else {
    true
  };

  let mut report = KiCadAccessReport {
    has_access: false,
    kicad_cli_found: cli_path_opt.is_some(),
    kicad_cli_path: cli_path_opt.as_ref().map(|p| p.to_string_lossy().to_string()),
    kicad_version: None,
    app_bundle_found: app_bundle_opt.is_some(),
    app_bundle_path: app_bundle_opt.as_ref().map(|p| p.to_string_lossy().to_string()),
    preferences_accessible,
    preferences_path: preferences_path.map(|p| p.to_string_lossy().to_string()),
    config_home: config_home.to_string_lossy().to_string(),
    has_quarantine,
    message: String::new(),
  };

  if let Some(ref cli_path) = cli_path_opt {
    let mut cmd = Command::new(cli_path);
    cmd.arg("--version");
    configure_kicad_command(&mut cmd);

    match cmd.output() {
      Ok(output) => {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout.trim().to_string();

        if output.status.success() && !version.is_empty() {
          report.kicad_version = Some(version.clone());
          if stderr.contains("Operation not permitted") {
            report.has_access = true; // Still working via isolated config home
            report.message = format!(
              "KiCad {} is operational using isolated configuration directory.",
              version
            );
          } else {
            report.has_access = true;
            report.message = format!("KiCad {} is fully operational with full access.", version);
          }
        } else {
          report.has_access = false;
          report.message = format!("kicad-cli returned error: {}", stderr.trim());
        }
      }
      Err(e) => {
        report.has_access = false;
        report.message = format!("Failed to execute kicad-cli: {}", e);
      }
    }
  } else {
    report.has_access = false;
    report.message = "kicad-cli was not found on PATH or standard installation directories.".to_string();
  }

  report
}

/// Asks the user for access to the KiCad application files via native dialogs when permissions are missing.
pub fn prompt_and_request_access() -> KiCadAccessReport {
  let initial = check_kicad_access();

  #[cfg(target_os = "macos")]
  {
    // 1. If quarantine is set, remove it
    if let Some(ref app) = initial.app_bundle_path {
      if initial.has_quarantine {
        remove_quarantine(Path::new(app));
      }
    }

    let current = check_kicad_access();
    if current.has_access && !current.has_quarantine {
      return current;
    }

    // 2. If kicad-cli was not found, prompt user to select KiCad.app
    if !initial.kicad_cli_found {
      let script = r#"
        try
          set selectedApp to POSIX path of (choose application with prompt "PCB Forge needs access to KiCad. Please select your KiCad application:")
          return selectedApp
        on error
          return ""
        end try
      "#;

      if let Ok(output) = Command::new("osascript").args(["-e", script]).output() {
        let chosen = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !chosen.is_empty() {
          let app_path = PathBuf::from(&chosen);
          let cli_in_app = app_path.join("Contents/MacOS/kicad-cli");
          if cli_in_app.exists() {
            save_kicad_cli_path(&cli_in_app);
            remove_quarantine(&app_path);
          }
        }
      }
    }

    // 3. If KiCad still cannot be accessed or preferences are restricted, show permission request dialog
    let updated = check_kicad_access();
    if !updated.has_access {
      let dialog_script = r#"
        display dialog "PCB Forge requires permission to access KiCad application files to render PCB and schematic previews." & return & return & "Click 'Grant Access' to locate your KiCad application, or 'Open Settings' to configure Full Disk Access." buttons {"Cancel", "Open Settings", "Grant Access"} default button "Grant Access" with title "PCB Forge - KiCad Access" with icon caution
      "#;

      if let Ok(output) = Command::new("osascript").args(["-e", dialog_script]).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("Grant Access") {
          let folder_script = r#"
            try
              set chosen to POSIX path of (choose folder with prompt "Grant PCB Forge access to KiCad folder (/Applications/KiCad or KiCad Preferences):")
              return chosen
            on error
              return ""
            end try
          "#;
          let _ = Command::new("osascript").args(["-e", folder_script]).output();
        } else if stdout.contains("Open Settings") {
          let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles")
            .output();
        }
      }
    }
  }

  check_kicad_access()
}
