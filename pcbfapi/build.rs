use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
  #[cfg(target_os = "windows")]
  {
    println!("cargo:rustc-link-lib=ucrt");
    println!("cargo:rustc-link-lib=msvcrt");
  }
  
  let out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
  let binaries_dir = Path::new(&out_dir).join("binaries");
  
  if !binaries_dir.exists() {
    fs::create_dir_all(&binaries_dir).unwrap();
  }
  
  let target = env::var("TARGET").unwrap_or_else(|_| "aarch64-apple-darwin".to_string());
  
  let (release_name, binary_name) = if target.contains("apple-darwin") {
    if target.contains("aarch64") {
      ("qdrant-aarch64-apple-darwin.tar.gz", "qdrant")
    } else {
      ("qdrant-x86_64-apple-darwin.tar.gz", "qdrant")
    }
  } else if target.contains("windows") {
    ("qdrant-x86_64-pc-windows-gnu.zip", "qdrant.exe")
  } else {
    ("qdrant-x86_64-unknown-linux-gnu.tar.gz", "qdrant")
  };
  
  let qdrant_version = "v1.19.0";
  let url = format!("https://github.com/qdrant/qdrant/releases/download/{}/{}", qdrant_version, release_name);
  let archive_path = binaries_dir.join(release_name);
  let final_bin_path = binaries_dir.join("qdrant_bin");
  
  if !final_bin_path.exists() {
    println!("cargo:warning=Downloading Qdrant from {}", url);
    
    let status = Command::new("curl")
      .args(["-L", "-o", archive_path.to_str().unwrap(), &url])
      .status()
      .expect("Failed to execute curl");
    
    if status.success() {
      if release_name.ends_with(".tar.gz") {
        Command::new("tar")
          .args(["-xzf", archive_path.to_str().unwrap(), "-C", binaries_dir.to_str().unwrap()])
          .status()
          .expect("Failed to extract tar.gz");
      } else if release_name.ends_with(".zip") {
        Command::new("unzip")
          .args(["-o", archive_path.to_str().unwrap(), "-d", binaries_dir.to_str().unwrap()])
          .status()
          .expect("Failed to extract zip");
      }
      
      let extracted_bin = binaries_dir.join(binary_name);
      fs::rename(extracted_bin, &final_bin_path).unwrap();
      let _ = fs::remove_file(archive_path);
    } else {
      panic!("Failed to download Qdrant binary");
    }
  }
  
  println!("cargo:rerun-if-changed=build.rs");
}