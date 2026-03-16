// Shared helpers for integration tests

use std::path::PathBuf;
use std::process::{Command, Output};

/// Get path to the compiled binary.
/// Tries release first, falls back to debug.
pub fn binary_path() -> PathBuf {
    let release = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("release")
        .join("siteone-crawler");
    if release.exists() {
        return release;
    }
    // Fall back to debug build
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("siteone-crawler")
}

/// Run the crawler with given arguments and return Output.
pub fn run_crawler(args: &[&str]) -> Output {
    Command::new(binary_path())
        .args(args)
        .output()
        .expect("Failed to execute crawler binary")
}

/// Run crawler and parse stdout as JSON.
pub fn run_crawler_json(args: &[&str]) -> serde_json::Value {
    let output = run_crawler(args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // JSON output may be preceded by progress lines on stderr, but stdout should be pure JSON
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "Failed to parse JSON output: {}\nFirst 500 chars: {}",
            e,
            &stdout[..stdout.len().min(500)]
        )
    })
}

/// Create a temporary directory that is cleaned up when dropped.
pub struct TempDir {
    pub path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("siteone-test-{}-{}", prefix, std::process::id()));
        if path.exists() {
            std::fs::remove_dir_all(&path).ok();
        }
        std::fs::create_dir_all(&path).expect("Failed to create temp dir");
        TempDir { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}
