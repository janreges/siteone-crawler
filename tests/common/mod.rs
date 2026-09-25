// Shared helpers for integration tests

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// Path to the binary Cargo built for this test run — never a leftover `target/release` build,
/// which would make the tests check whatever version happens to be lying there.
pub fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_siteone-crawler"))
}

/// Run the crawler with given arguments and return Output.
pub fn run_crawler(args: &[&str]) -> Output {
    Command::new(binary_path())
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

/// One canned answer of a `RecordingServer`: the request path it serves and the response
/// headers and body (`Content-Length` and `Connection: close` are added automatically). A
/// CGI-style `Status` header (e.g. `500 Internal Server Error`) replaces the default `200 OK`.
pub struct Route {
    pub path: &'static str,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

/// A minimal HTTP/1.1 server on 127.0.0.1 for tests that need exact response bytes (e.g. a
/// Brotli-encoded body, which the built-in server cannot produce) or need to see what the
/// crawler sent. Unknown paths get a 404. Stopped when dropped.
pub struct RecordingServer {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl RecordingServer {
    pub fn start(routes: Vec<Route>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("a bound address").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (recorded, stopped) = (requests.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = stream else { continue };
                let head = read_request_head(&mut stream);
                let path = head
                    .split_whitespace()
                    .nth(1)
                    .and_then(|target| target.split('?').next())
                    .unwrap_or("/")
                    .to_string();
                recorded.lock().unwrap().push(head);
                let response = match routes.iter().find(|route| route.path == path) {
                    Some(route) => {
                        let status = route.headers.iter().find(|(name, _)| *name == "Status");
                        let headers: Vec<(&str, String)> = route
                            .headers
                            .iter()
                            .filter(|(name, _)| *name != "Status")
                            .cloned()
                            .collect();
                        raw_http_response(status.map_or("200 OK", |(_, value)| value), &headers, &route.body)
                    }
                    None => raw_http_response("404 Not Found", &[], b""),
                };
                stream.write_all(&response).ok();
            }
        });
        RecordingServer {
            port,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The head (request line and headers) of every request received so far.
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for RecordingServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the blocking `accept` so the thread sees the flag.
        TcpStream::connect(("127.0.0.1", self.port)).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

/// One canned answer of a `MockLlm`: requests whose path starts with `path_prefix` get `status`
/// and the JSON `body`, after `delay_ms` milliseconds.
pub struct MockResponse {
    pub path_prefix: &'static str,
    pub status: u16,
    pub body: String,
    pub delay_ms: u64,
}

/// A minimal OpenAI-compatible LLM endpoint on 127.0.0.1 (base URL `url()`, e.g. for
/// `--ai-endpoint`) that answers with canned bodies such as the captured provider responses in
/// `tests/fixtures/ai-responses/`. The responses of one path prefix are served in order and the
/// last one repeats once they are used up; the first prefix (in the order given) that matches a
/// request path serves it, other paths get a 404. Every request body is recorded. Each connection
/// is answered on its own thread, so concurrent AI requests are not serialized. Stopped when
/// dropped.
pub struct MockLlm {
    port: u16,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MockLlm {
    pub fn start(responses: Vec<MockResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("a bound address").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let responses = Arc::new(responses);
        // How many requests each path prefix has answered so far.
        let served: Arc<Mutex<Vec<(&'static str, usize)>>> = Arc::new(Mutex::new(Vec::new()));
        let (recorded, stopped) = (requests.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut stream) = stream else { continue };
                let (responses, served, recorded) = (responses.clone(), served.clone(), recorded.clone());
                std::thread::spawn(move || {
                    let (head, body) = read_request(&mut stream);
                    let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                    recorded.lock().unwrap().push(body);
                    let answer = responses
                        .iter()
                        .find(|response| path.starts_with(response.path_prefix))
                        .map(|first| {
                            let prefix = first.path_prefix;
                            let candidates: Vec<&MockResponse> =
                                responses.iter().filter(|r| r.path_prefix == prefix).collect();
                            let mut served = served.lock().unwrap();
                            let index = match served.iter_mut().find(|(p, _)| *p == prefix) {
                                Some((_, count)) => {
                                    *count += 1;
                                    *count - 1
                                }
                                None => {
                                    served.push((prefix, 1));
                                    0
                                }
                            };
                            candidates[index.min(candidates.len() - 1)]
                        });
                    let response = match answer {
                        Some(answer) => {
                            std::thread::sleep(Duration::from_millis(answer.delay_ms));
                            let status = format!("{} {}", answer.status, reason_phrase(answer.status));
                            raw_http_response(
                                &status,
                                &[("Content-Type", "application/json".to_string())],
                                answer.body.as_bytes(),
                            )
                        }
                        None => raw_http_response("404 Not Found", &[], b""),
                    };
                    stream.write_all(&response).ok();
                });
            }
        });
        MockLlm {
            port,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    /// The OpenAI-style base URL, `http://127.0.0.1:<port>/v1`.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The body of every request received so far, in arrival order.
    pub fn request_bodies(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for MockLlm {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the blocking `accept` so the thread sees the flag.
        TcpStream::connect(("127.0.0.1", self.port)).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Mock",
    }
}

/// Reads one request: its head and, per `Content-Length`, its body.
fn read_request(stream: &mut TcpStream) -> (String, String) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    let head_end = loop {
        if let Some(pos) = data.windows(4).position(|window| window == b"\r\n\r\n") {
            break pos + 4;
        }
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break data.len(),
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    while data.len() < head_end + length {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    }
    let body = String::from_utf8_lossy(&data[head_end..data.len().min(head_end + length)]).into_owned();
    (head, body)
}

fn read_request_head(stream: &mut TcpStream) -> String {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => head.extend_from_slice(&buf[..n]),
        }
    }
    String::from_utf8_lossy(&head).into_owned()
}

fn raw_http_response(status: &str, headers: &[(&str, String)], body: &[u8]) -> Vec<u8> {
    let mut head = format!("HTTP/1.1 {status}\r\n");
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()));
    let mut response = head.into_bytes();
    response.extend_from_slice(body);
    response
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

/// A redirect rule of `RedirectServer`: a request for `host` (any host when `None`) and `path`
/// (any path when `None`) gets `301 Moved Permanently` to `location`, in which `{port}` is replaced
/// by the server port and `{path}` by the request path and query.
pub struct Redirect {
    pub host: Option<&'static str>,
    pub path: Option<&'static str>,
    pub location: &'static str,
}

/// Minimal HTTP/1.1 server on 127.0.0.1 for crawls that need redirects, which the built-in
/// `--serve-offline` server cannot produce. A request matching a `Redirect` rule gets a 301, any
/// other request the file below `root` (`/x/` → `x/index.html`, `/x` → `x`, `x.html` or
/// `x/index.html`) or a 404. One request per connection. Stopped when dropped.
pub struct RedirectServer {
    port: u16,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl RedirectServer {
    pub fn start(root: &Path, redirects: Vec<Redirect>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("the local address").port();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let thread_stop = stop.clone();
        let root = root.to_path_buf();
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if thread_stop.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                if let Ok(stream) = stream {
                    redirect_server_respond(stream, &root, &redirects, port);
                }
            }
        });
        RedirectServer {
            port,
            stop,
            thread: Some(thread),
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for RedirectServer {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        // wake up the blocking accept()
        TcpStream::connect(("127.0.0.1", self.port)).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
    }
}

fn redirect_server_respond(mut stream: TcpStream, root: &Path, redirects: &[Redirect], port: u16) {
    use std::io::{Read, Write};

    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut request = Vec::new();
    let mut buffer = [0u8; 4096];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => request.extend_from_slice(&buffer[..read]),
        }
    }
    let request = String::from_utf8_lossy(&request);
    let mut request_line = request.lines().next().unwrap_or("").split_whitespace();
    let method = request_line.next().unwrap_or("GET");
    let target = request_line.next().unwrap_or("/");
    let host = request
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("host").then(|| value.trim())
        })
        .unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    let path = target.split(['?', '#']).next().unwrap_or("/");

    let redirect = redirects
        .iter()
        .find(|rule| rule.host.is_none_or(|h| h == host) && rule.path.is_none_or(|p| p == path));
    let (status, content_type, location, body) = if let Some(rule) = redirect {
        let location = rule
            .location
            .replace("{port}", &port.to_string())
            .replace("{path}", target);
        ("301 Moved Permanently", "text/html", Some(location), Vec::new())
    } else {
        let relative = path.trim_start_matches('/');
        let candidates = if relative.is_empty() || relative.ends_with('/') {
            vec![format!("{relative}index.html")]
        } else {
            vec![
                relative.to_string(),
                format!("{relative}.html"),
                format!("{relative}/index.html"),
            ]
        };
        let file = candidates
            .iter()
            .map(|candidate| root.join(candidate))
            .find(|file| !path.contains("..") && file.is_file());
        match file {
            Some(file) => {
                let content_type = match file.extension().and_then(|ext| ext.to_str()) {
                    Some("html") => "text/html; charset=utf-8",
                    Some("css") => "text/css",
                    Some("png") => "image/png",
                    _ => "application/octet-stream",
                };
                ("200 OK", content_type, None, std::fs::read(file).unwrap_or_default())
            }
            None => ("404 Not Found", "text/html", None, b"Not found".to_vec()),
        }
    };

    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(location) = location {
        response.push_str(&format!("Location: {location}\r\n"));
    }
    response.push_str("\r\n");
    stream.write_all(response.as_bytes()).ok();
    if method != "HEAD" {
        stream.write_all(&body).ok();
    }
    stream.flush().ok();
}
