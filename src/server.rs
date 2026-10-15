//! `gitroast serve`: the same roast over HTTP, with health checks and metrics.

use crate::github::{self, FetchError};
use crate::{profile, roast};
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry, TextEncoder,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tiny_http::{Header, Request, Response, Server};

/// How long a roast is reused before asking GitHub again.
const CACHE_TTL: Duration = Duration::from_secs(600);
const CACHE_MAX: usize = 1000;
const WORKERS: usize = 8;

#[derive(Serialize)]
struct RoastBody {
    login: String,
    name: Option<String>,
    score: u32,
    summary: &'static str,
    burns: Vec<String>,
}

struct Metrics {
    registry: Registry,
    requests: IntCounterVec,
    duration: HistogramVec,
    github: IntCounterVec,
    cache: IntCounterVec,
}

impl Metrics {
    fn new() -> Self {
        let registry = Registry::new();
        let requests = IntCounterVec::new(
            Opts::new("gitroast_http_requests_total", "HTTP requests served"),
            &["route", "status"],
        )
        .unwrap();
        let duration = HistogramVec::new(
            HistogramOpts::new(
                "gitroast_http_request_duration_seconds",
                "Time to answer an HTTP request",
            )
            .buckets(vec![0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0]),
            &["route"],
        )
        .unwrap();
        let github = IntCounterVec::new(
            Opts::new(
                "gitroast_github_fetches_total",
                "Profile fetches from GitHub",
            ),
            &["outcome"],
        )
        .unwrap();
        let cache = IntCounterVec::new(
            Opts::new("gitroast_cache_lookups_total", "Roast cache lookups"),
            &["result"],
        )
        .unwrap();
        let info = IntGaugeVec::new(
            Opts::new(
                "gitroast_build_info",
                "Version and commit of the running build",
            ),
            &["version", "commit"],
        )
        .unwrap();
        info.with_label_values(&[env!("CARGO_PKG_VERSION"), commit()])
            .set(1);

        registry.register(Box::new(requests.clone())).unwrap();
        registry.register(Box::new(duration.clone())).unwrap();
        registry.register(Box::new(github.clone())).unwrap();
        registry.register(Box::new(cache.clone())).unwrap();
        registry.register(Box::new(info)).unwrap();
        Metrics {
            registry,
            requests,
            duration,
            github,
            cache,
        }
    }
}

struct App {
    client: github::Client,
    /// Lowercase username -> (when, HTTP status, JSON body).
    cache: Mutex<HashMap<String, (Instant, u16, String)>>,
    metrics: Metrics,
    ready: AtomicBool,
}

/// The commit this binary was built from, set by the Docker build.
fn commit() -> &'static str {
    option_env!("GIT_SHA").unwrap_or("unknown")
}

pub fn run(port: u16) -> std::io::Result<()> {
    let server = Arc::new(
        Server::http(("0.0.0.0", port)).map_err(|e| std::io::Error::other(e.to_string()))?,
    );
    let app = Arc::new(App {
        client: github::Client::from_env(),
        cache: Mutex::new(HashMap::new()),
        metrics: Metrics::new(),
        ready: AtomicBool::new(true),
    });

    // On SIGTERM: fail readiness so Kubernetes stops sending traffic, give it a
    // moment to notice, then stop accepting and let in-flight requests finish.
    {
        let app = app.clone();
        let server = server.clone();
        ctrlc::set_handler(move || {
            log_event("shutdown", "draining");
            app.ready.store(false, Ordering::SeqCst);
            thread::sleep(Duration::from_secs(5));
            for _ in 0..WORKERS {
                server.unblock();
            }
        })
        .map_err(std::io::Error::other)?;
    }

    log_event(
        "start",
        &format!("listening on :{port}, commit {}", commit()),
    );
    let workers: Vec<_> = (0..WORKERS)
        .map(|_| {
            let server = server.clone();
            let app = app.clone();
            thread::spawn(move || {
                while let Ok(request) = server.recv() {
                    handle(&app, request);
                }
            })
        })
        .collect();
    for worker in workers {
        let _ = worker.join();
    }
    log_event("stop", "bye");
    Ok(())
}

fn handle(app: &App, request: Request) {
    let started = Instant::now();
    let path = request.url().split('?').next().unwrap_or("").to_string();
    let method = request.method().to_string();

    let (route, status, content_type, body) = if method != "GET" {
        (
            "other",
            405,
            "text/plain",
            "method not allowed\n".to_string(),
        )
    } else {
        match path.as_str() {
            "/healthz" => ("/healthz", 200, "text/plain", "ok\n".to_string()),
            "/readyz" if app.ready.load(Ordering::SeqCst) => {
                ("/readyz", 200, "text/plain", "ready\n".to_string())
            }
            "/readyz" => ("/readyz", 503, "text/plain", "shutting down\n".to_string()),
            "/metrics" => (
                "/metrics",
                200,
                "text/plain; version=0.0.4",
                metrics_text(app),
            ),
            "/" => (
                "/",
                200,
                "text/plain",
                "gitroast: GET /roast/{username}\n".to_string(),
            ),
            p if p.starts_with("/roast/") => {
                let (status, body) = roast_route(app, &p["/roast/".len()..]);
                ("/roast", status, "application/json", body)
            }
            _ => ("other", 404, "text/plain", "not found\n".to_string()),
        }
    };

    let elapsed = started.elapsed();
    app.metrics
        .requests
        .with_label_values(&[route, &status.to_string()])
        .inc();
    app.metrics
        .duration
        .with_label_values(&[route])
        .observe(elapsed.as_secs_f64());
    // Probes and scrapes would drown out real traffic in the logs.
    if !matches!(route, "/healthz" | "/readyz" | "/metrics") {
        log_request(&method, &path, status, elapsed);
    }

    let header = Header::from_bytes("Content-Type", content_type).unwrap();
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(header);
    let _ = request.respond(response);
}

fn roast_route(app: &App, username: &str) -> (u16, String) {
    if !crate::is_valid_username(username) {
        return (400, error_json("not a valid GitHub username"));
    }
    let key = username.to_ascii_lowercase();

    if let Some((at, status, body)) = app.cache.lock().unwrap().get(&key)
        && at.elapsed() < CACHE_TTL
    {
        app.metrics.cache.with_label_values(&["hit"]).inc();
        return (*status, body.clone());
    }
    app.metrics.cache.with_label_values(&["miss"]).inc();

    let (status, body) = fetch_and_judge(app, username);
    // Cache "user not found" too, or every retry of a typo costs GitHub calls.
    // Rate limits and network errors are not cached: they should be retried.
    if matches!(status, 200 | 404) {
        let mut cache = app.cache.lock().unwrap();
        if cache.len() >= CACHE_MAX {
            cache.retain(|_, (at, _, _)| at.elapsed() < CACHE_TTL);
            if cache.len() >= CACHE_MAX {
                cache.clear();
            }
        }
        cache.insert(key, (Instant::now(), status, body.clone()));
    }
    (status, body)
}

fn fetch_and_judge(app: &App, username: &str) -> (u16, String) {
    let fetched = app
        .client
        .user(username)
        .and_then(|user| Ok((user, app.client.repos(username)?)));
    let (user, repos) = match fetched {
        Ok(data) => {
            app.metrics.github.with_label_values(&["ok"]).inc();
            data
        }
        Err(e) => {
            let (outcome, status) = match e {
                FetchError::NotFound(_) => ("not_found", 404),
                FetchError::RateLimited => ("rate_limited", 503),
                FetchError::Other(_) => ("error", 502),
            };
            app.metrics.github.with_label_values(&[outcome]).inc();
            return (status, error_json(&e.to_string()));
        }
    };

    let profile = profile::Profile {
        user,
        repos,
        today: crate::today(),
    };
    let verdict = roast::judge(&profile);
    let body = RoastBody {
        login: profile.user.login.clone(),
        name: profile.user.name.clone(),
        score: verdict.score,
        summary: verdict.summary,
        burns: verdict.burns.into_iter().map(|b| b.text).collect(),
    };
    (200, serde_json::to_string(&body).unwrap())
}

fn metrics_text(app: &App) -> String {
    let mut buffer = Vec::new();
    TextEncoder::new()
        .encode(&app.metrics.registry.gather(), &mut buffer)
        .unwrap();
    String::from_utf8(buffer).unwrap()
}

fn error_json(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

fn log_request(method: &str, path: &str, status: u16, elapsed: Duration) {
    let line = serde_json::json!({
        "ts": now_secs(),
        "level": if status >= 500 { "error" } else { "info" },
        "event": "request",
        "method": method,
        "path": path,
        "status": status,
        "duration_ms": elapsed.as_secs_f64() * 1000.0,
    });
    println!("{line}");
}

fn log_event(event: &str, message: &str) {
    let line = serde_json::json!({
        "ts": now_secs(),
        "level": "info",
        "event": event,
        "message": message,
    });
    println!("{line}");
}
