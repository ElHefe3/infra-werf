//! Isolated plugin views (S12/F23/T047).
//!
//! A plugin may ship a web page for a workspace tab. InfraWerf shows it in a
//! separate webview whose content comes from this small loopback server, so
//! the page has its own origin (`http://127.0.0.1:<port>`), which Tauri
//! treats as remote: it cannot call InfraWerf's commands. Every request must
//! carry the view's secret path prefix and the right `Host`, so another
//! program on this PC, or another page, cannot read the view or use its API.
//!
//! The page talks to InfraWerf only through the view message API under
//! `/<secret>/infrawerf/v1/`. This module parses and routes requests; every
//! decision about what a plugin may see or do is made by the [`ViewApi`]
//! the desktop shell supplies, against the owner's grants, in the core.
//! Responses carry a strict content security policy that keeps the page on
//! its own origin.
use crate::ServerId;
use crate::plugin_install::clean_path;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::time::Instant;
use ts_rs::{Config, TS};

/// The version of the view message API, part of its paths.
pub const VIEW_API_VERSION: u32 = 1;
pub const API_PREFIX: &str = "infrawerf/v1/";
const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_ASSET_BYTES: u64 = 5 * 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(10);
/// Connections served at once; one page needs few. Further ones are closed
/// at once (T047 review, finding 4).
pub const MAX_CONNECTIONS: usize = 16;
/// Time from accepting a connection to the end of its request headers.
const HEADER_DEADLINE: Duration = Duration::from_secs(5);

/// The content security policy directives a report may name; anything else
/// is logged as `other`, so a page cannot put its own words in the log.
const DIRECTIVES: &[&str] = &[
    "default-src",
    "script-src",
    "script-src-elem",
    "script-src-attr",
    "style-src",
    "style-src-elem",
    "style-src-attr",
    "img-src",
    "font-src",
    "connect-src",
    "media-src",
    "frame-src",
    "child-src",
    "worker-src",
    "manifest-src",
    "object-src",
    "base-uri",
    "form-action",
    "frame-ancestors",
];

/// The scheme and host of a blocked address, made of safe characters only.
fn blocked_target(uri: &str) -> String {
    let mut parts = uri.splitn(2, "://");
    let scheme = parts.next().unwrap_or_default();
    let rest = parts.next();
    let clean = |s: &str| {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
            .take(80)
            .collect::<String>()
            .to_ascii_lowercase()
    };
    match rest {
        Some(rest) => format!(
            "{}://{}",
            clean(scheme),
            clean(rest.split(['/', '?', '#']).next().unwrap_or_default())
        ),
        None => clean(scheme.split(':').next().unwrap_or_default()),
    }
}

/// What the page receives from an API call.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiReply {
    pub status: u16,
    pub body: serde_json::Value,
}

impl ApiReply {
    pub fn ok(body: impl Serialize) -> Self {
        Self {
            status: 200,
            body: serde_json::to_value(body).unwrap_or(serde_json::Value::Null),
        }
    }
    /// A refusal with a stable code and a sentence.
    pub fn refused(status: u16, code: &str, message: &str) -> Self {
        Self {
            status,
            body: serde_json::json!({ "error": code, "message": message }),
        }
    }
}

/// The view's link to InfraWerf. Implemented by the desktop shell; every
/// method decides against the owner's grants at the moment it is called.
pub trait ViewApi: Send + Sync {
    /// One message API call: `method` is `GET` or `POST`, `path` is what
    /// follows `infrawerf/v1/`, `body` is the request body (POST only).
    fn call(&self, method: &str, path: &str, body: &[u8]) -> ApiReply;
    /// Something the page tried that was blocked: a content security policy
    /// report, or a request without the secret or with the wrong host.
    fn blocked(&self, what: &str, detail: &str);
}

/// A running view server. Dropping it stops the server.
pub struct ViewServer {
    port: u16,
    token: String,
    entry: String,
    stop: Arc<AtomicBool>,
}

/// The files a view serves: read once, when it opens, by the same reader
/// and limits as an install (links refused), keyed by their exact path.
/// Nothing on disk is read while the view is open (T047 review, 7 and 8).
pub type ViewFiles = std::collections::BTreeMap<String, Vec<u8>>;

fn csp(token: &str) -> String {
    format!(
        "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
         font-src 'self'; connect-src 'self'; media-src 'self'; frame-src 'none'; child-src 'none'; \
         worker-src 'none'; manifest-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; \
         frame-ancestors 'none'; report-uri /{token}/{API_PREFIX}csp-report"
    )
}

fn content_type(path: &str) -> &'static str {
    let extension = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

struct Request {
    method: String,
    target: String,
    host: Option<String>,
    origin: Option<String>,
    fetch_site: Option<String>,
    body: Vec<u8>,
}

/// Reads one request within the limits; `None` for anything malformed or
/// too slow. The headers must arrive within `HEADER_DEADLINE` in all.
fn read_request(stream: &TcpStream) -> Option<Request> {
    let started = Instant::now();
    let mut reader = BufReader::new(stream.take((MAX_HEADER_BYTES + MAX_BODY_BYTES) as u64));
    let mut head = Vec::new();
    loop {
        let left = HEADER_DEADLINE.checked_sub(started.elapsed())?;
        stream
            .set_read_timeout(Some(left.max(Duration::from_millis(1))))
            .ok()?;
        let mut line = Vec::new();
        let read = reader.read_until(b'\n', &mut line).ok()?;
        if read == 0 {
            return None;
        }
        head.extend_from_slice(&line);
        if head.len() > MAX_HEADER_BYTES {
            return None;
        }
        if line == b"\r\n" || line == b"\n" {
            break;
        }
    }
    let text = std::str::from_utf8(&head).ok()?;
    let mut lines = text.lines();
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let target = first.next()?.to_owned();
    let mut request = Request {
        method,
        target,
        host: None,
        origin: None,
        fetch_site: None,
        body: Vec::new(),
    };
    let mut length = 0usize;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().to_owned();
        match name.trim().to_ascii_lowercase().as_str() {
            "host" => request.host = Some(value),
            "origin" => request.origin = Some(value),
            "sec-fetch-site" => request.fetch_site = Some(value),
            "content-length" => length = value.parse().ok()?,
            "transfer-encoding" => return None,
            _ => {}
        }
    }
    if length > MAX_BODY_BYTES {
        return None;
    }
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok()?;
    request.body = vec![0; length];
    reader.read_exact(&mut request.body).ok()?;
    Some(request)
}

fn respond(stream: &mut TcpStream, status: u16, kind: &str, body: &[u8], token: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
         Content-Security-Policy: {}\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n\
         Cache-Control: no-store\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Resource-Policy: same-origin\r\n\
         Permissions-Policy: camera=(), microphone=(), geolocation=(), usb=(), serial=(), hid=(), bluetooth=(), \
         clipboard-read=(), clipboard-write=(), display-capture=(), payment=(), publickey-credentials-get=()\r\n\
         Connection: close\r\n\r\n",
        body.len(),
        csp(token)
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn json(stream: &mut TcpStream, reply: &ApiReply, token: &str) {
    let body = serde_json::to_vec(&reply.body).unwrap_or_default();
    respond(stream, reply.status, "application/json", &body, token);
}

fn handle(mut stream: TcpStream, port: u16, token: &str, files: &ViewFiles, api: &dyn ViewApi) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let Some(request) = read_request(&stream) else {
        respond(&mut stream, 400, "text/plain", b"Bad request", token);
        return;
    };
    // The page's own origin only: no other host name (DNS rebinding), and
    // no request from another site.
    let origin = format!("http://127.0.0.1:{port}");
    if request.host.as_deref() != Some(&origin["http://".len()..]) {
        api.blocked("wrong_host", request.host.as_deref().unwrap_or(""));
        respond(&mut stream, 403, "text/plain", b"Forbidden", token);
        return;
    }
    let path = request.target.split(['?', '#']).next().unwrap_or_default();
    let Some(rest) = path.strip_prefix(&format!("/{token}/")) else {
        api.blocked("no_secret", "");
        respond(&mut stream, 404, "text/plain", b"Not found", token);
        return;
    };
    if let Some(call) = rest.strip_prefix(API_PREFIX) {
        let cross_site = request.origin.as_deref().is_some_and(|o| o != origin)
            || request
                .fetch_site
                .as_deref()
                .is_some_and(|site| site != "same-origin" && site != "none");
        if cross_site {
            api.blocked("cross_origin", request.origin.as_deref().unwrap_or(""));
            json(
                &mut stream,
                &ApiReply::refused(403, "cross_origin", "Requests come from the view only."),
                token,
            );
            return;
        }
        if call == "csp-report" && request.method == "POST" {
            let report: serde_json::Value =
                serde_json::from_slice(&request.body).unwrap_or_default();
            let inner = report.get("csp-report").unwrap_or(&report);
            let directive = inner
                .get("effective-directive")
                .or_else(|| inner.get("violated-directive"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let blocked = inner
                .get("blocked-uri")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // Only the scheme and host of what was blocked; never a path.
            let directive = DIRECTIVES
                .iter()
                .find(|d| **d == directive)
                .copied()
                .unwrap_or("other");
            api.blocked(
                "content_policy",
                &format!("{directive} {}", blocked_target(blocked)),
            );
            json(&mut stream, &ApiReply::ok(serde_json::json!({})), token);
            return;
        }
        if request.method != "GET" && request.method != "POST" {
            json(
                &mut stream,
                &ApiReply::refused(405, "method", "Use GET or POST."),
                token,
            );
            return;
        }
        let reply = api.call(&request.method, call, &request.body);
        json(&mut stream, &reply, token);
        return;
    }
    if request.method != "GET" {
        respond(&mut stream, 405, "text/plain", b"Method not allowed", token);
        return;
    }
    let Ok(parts) = clean_path(rest) else {
        respond(&mut stream, 404, "text/plain", b"Not found", token);
        return;
    };
    if parts.iter().any(|part| part.starts_with('.')) {
        respond(&mut stream, 404, "text/plain", b"Not found", token);
        return;
    }
    // Exactly a file the view read when it opened: no short names, no
    // files added or changed since.
    match files
        .get(&parts.join("/"))
        .filter(|bytes| bytes.len() as u64 <= MAX_ASSET_BYTES)
    {
        Some(bytes) => respond(&mut stream, 200, content_type(rest), bytes, token),
        None => respond(&mut stream, 404, "text/plain", b"Not found", token),
    }
    let _ = stream.shutdown(Shutdown::Both);
}

impl ViewServer {
    /// Starts serving `files` on a random loopback port. `entry` is the
    /// page the view opens.
    pub fn start(files: ViewFiles, entry: &str, api: Arc<dyn ViewApi>) -> std::io::Result<Self> {
        let files = Arc::new(files);
        let active = Arc::new(AtomicUsize::new(0));
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let token = uuid::Uuid::new_v4().simple().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, secret) = (stop.clone(), token.clone());
        std::thread::Builder::new()
            .name(format!("plugin-view-{port}"))
            .spawn(move || {
                for stream in listener.incoming() {
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                        active.fetch_sub(1, Ordering::SeqCst);
                        let _ = stream.shutdown(Shutdown::Both);
                        continue;
                    }
                    let (secret, files, api, done) =
                        (secret.clone(), files.clone(), api.clone(), active.clone());
                    let spawned = std::thread::Builder::new()
                        .name("plugin-view-request".into())
                        .spawn(move || {
                            handle(stream, port, &secret, &files, api.as_ref());
                            done.fetch_sub(1, Ordering::SeqCst);
                        });
                    if spawned.is_err() {
                        active.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            })?;
        Ok(Self {
            port,
            token,
            entry: entry.to_owned(),
            stop,
        })
    }
    pub fn origin(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
    pub fn port(&self) -> u16 {
        self.port
    }
    /// The address the view opens.
    pub fn url(&self) -> String {
        format!("{}/{}/{}", self.origin(), self.token, self.entry)
    }
}

impl Drop for ViewServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the flag and the port closes.
        let _ = TcpStream::connect_timeout(
            &std::net::SocketAddr::from(([127, 0, 0, 1], self.port)),
            Duration::from_millis(200),
        );
    }
}

/// An action a view asked for, waiting for the owner or finished.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ViewActionState {
    Waiting,
    /// The owner pressed Run and it is running now.
    Running,
    Declined,
    Done,
    Failed,
    /// The view or the plugin's permissions went away first.
    Cancelled,
}

/// What InfraWerf shows the owner when a view asks to run a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ViewActionRequest {
    pub id: String,
    pub view: String,
    pub plugin_id: String,
    pub plugin_name: String,
    pub server_id: ServerId,
    pub server_name: String,
    pub contribution_id: String,
    pub title: String,
    /// The exact command, with `{name}` where a parameter goes.
    pub argv: Vec<String>,
    /// Parameter name to the discovered workload's name, as chosen by the view.
    pub parameters: BTreeMap<String, String>,
}

pub fn typescript_contracts(config: &Config) -> String {
    [
        ViewActionState::decl(config),
        ViewActionRequest::decl(config),
    ]
    .into_iter()
    .map(|d| {
        format!(
            "export {d}
"
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder {
        blocked: Mutex<Vec<(String, String)>>,
        calls: Mutex<Vec<(String, String)>>,
    }
    impl ViewApi for Recorder {
        fn call(&self, method: &str, path: &str, _: &[u8]) -> ApiReply {
            self.calls
                .lock()
                .unwrap()
                .push((method.into(), path.into()));
            ApiReply::ok(serde_json::json!({ "path": path }))
        }
        fn blocked(&self, what: &str, detail: &str) {
            self.blocked
                .lock()
                .unwrap()
                .push((what.into(), detail.into()));
        }
    }

    fn get(port: u16, target: &str, host: &str, extra: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "GET {target} HTTP/1.1\r\nHost: {host}\r\n{extra}\r\n"
        )
        .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        answer
    }

    #[test]
    fn only_the_secret_path_on_the_right_host_serves_files_and_the_api() {
        let mut files = ViewFiles::new();
        files.insert("view/index.html".into(), b"<p>hello</p>".to_vec());
        files.insert(".secret".into(), b"no".to_vec());
        let recorder = Arc::new(Recorder::default());
        let server = ViewServer::start(files, "view/index.html", recorder.clone()).unwrap();
        let port = server.port();
        let host = format!("127.0.0.1:{port}");
        let secret = server.url().split('/').nth(3).unwrap().to_owned();
        let page = get(port, &format!("/{secret}/view/index.html"), &host, "");
        assert!(page.starts_with("HTTP/1.1 200"), "{page}");
        assert!(page.contains("<p>hello</p>"));
        assert!(page.contains("Content-Security-Policy: default-src 'none'; script-src 'self'"));
        assert!(page.contains("connect-src 'self'"));
        assert!(page.contains("X-Content-Type-Options: nosniff"));
        for (target, host) in [
            ("/view/index.html".to_owned(), host.clone()),
            (
                format!("/{secret}/view/index.html"),
                "evil.example".to_owned(),
            ),
            (
                format!("/{secret}/view/index.html"),
                format!("localhost:{port}"),
            ),
            (format!("/{secret}/../Cargo.toml"), host.clone()),
            (format!("/{secret}/.secret"), host.clone()),
            (format!("/{secret}/view/%2e%2e/.secret"), host.clone()),
            // Only exact names: no case folding, no 8.3 short names.
            (format!("/{secret}/VIEW/INDEX.HTML"), host.clone()),
            (format!("/{secret}/view/INDEX~1.HTM"), host.clone()),
            (format!("/{secret}/SECRET~1"), host.clone()),
        ] {
            let answer = get(port, &target, &host, "");
            assert!(
                !answer.starts_with("HTTP/1.1 200"),
                "{target} {host}: {answer}"
            );
        }
        let api = get(
            port,
            &format!("/{secret}/infrawerf/v1/context"),
            &host,
            "Sec-Fetch-Site: same-origin\r\n",
        );
        assert!(api.contains("\"path\":\"context\""), "{api}");
        let cross = get(
            port,
            &format!("/{secret}/infrawerf/v1/context"),
            &host,
            "Origin: http://127.0.0.1:1\r\n",
        );
        assert!(cross.starts_with("HTTP/1.1 403"));
        assert_eq!(recorder.calls.lock().unwrap().len(), 1);
        let blocked: Vec<String> = recorder
            .blocked
            .lock()
            .unwrap()
            .iter()
            .map(|b| b.0.clone())
            .collect();
        assert!(blocked.contains(&"no_secret".to_owned()));
        assert!(blocked.contains(&"wrong_host".to_owned()));
        assert!(blocked.contains(&"cross_origin".to_owned()));
        // A page cannot write its own words into the log.
        let report = format!(
            "POST /{secret}/infrawerf/v1/csp-report HTTP/1.1\r\nHost: {host}\r\nContent-Length: {{}}\r\n\r\n{{}}"
        );
        let body = r#"{"csp-report":{"effective-directive":"InfraWerf: all good","blocked-uri":"https://trusted.example/a b?c"}}"#;
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "{}",
            report
                .replacen("{}", &body.len().to_string(), 1)
                .replacen("{}", body, 1)
        )
        .unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        let logged = recorder.blocked.lock().unwrap().last().unwrap().clone();
        assert_eq!(
            logged,
            (
                "content_policy".to_owned(),
                "other https://trusted.example".to_owned()
            )
        );
        // Slow or idle connections cannot pile up: the 17th is closed at
        // once, and an idle one is dropped after the header deadline.
        let idle: Vec<TcpStream> = (0..MAX_CONNECTIONS)
            .map(|_| TcpStream::connect(("127.0.0.1", port)).unwrap())
            .collect();
        std::thread::sleep(Duration::from_millis(300));
        let mut extra = TcpStream::connect(("127.0.0.1", port)).unwrap();
        extra
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut nothing = Vec::new();
        assert_eq!(
            extra
                .read_to_end(&mut nothing)
                .map(|n| n == 0)
                .unwrap_or(true),
            true
        );
        std::thread::sleep(HEADER_DEADLINE + Duration::from_millis(500));
        let page = get(port, &format!("/{secret}/view/index.html"), &host, "");
        assert!(page.starts_with("HTTP/1.1 200"), "{page}");
        drop(idle);
        drop(server);
        std::thread::sleep(Duration::from_millis(300));
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    }
}
