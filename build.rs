use std::process::Command;

fn main() {
    // Set build time
    let output = Command::new("date")
        .args(&["+%Y-%m-%d %H:%M:%S UTC"])
        .output();
    
    let build_time = match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(_) => {
            // Fallback for Windows or if date command fails
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string()
        }
    };
    
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    
    // Re-run if build script changes
    println!("cargo:rerun-if-changed=build.rs");
    
    // Re-run if any source files change
    println!("cargo:rerun-if-changed=src/");
}