// Shared helpers for integration tests

use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

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

/// Run the binary Cargo built for this test run. Unlike `run_crawler`, it never picks up a
/// stale `target/release` build, so it suits tests of behaviour that changed recently.
pub fn run_built_crawler(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_siteone-crawler"))
        .args(args)
        .output()
        .expect("Failed to execute crawler binary")
}

/// The crawler's built-in server (`--serve-offline`) serving a local directory, so a crawl can
/// run without network access. Stopped when dropped.
pub struct LocalServer {
    child: Child,
    port: u16,
}

impl LocalServer {
    pub fn start(root: &Path) -> Self {
        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port();
        let child = Command::new(env!("CARGO_BIN_EXE_siteone-crawler"))
            .args([
                format!("--serve-offline={}", root.display()),
                format!("--serve-port={port}"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the built-in server starts");
        let server = LocalServer { child, port };
        assert!(
            (0..50).any(|_| {
                std::thread::sleep(Duration::from_millis(100));
                TcpStream::connect(("127.0.0.1", port)).is_ok()
            }),
            "the built-in server accepts connections"
        );
        server
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
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
