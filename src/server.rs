use crate::{outbox, repository, store::Store, sync};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf};
use tower_http::services::{ServeDir, ServeFile};
type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;
async fn blocking<F>(f: F) -> ApiResult
where
    F: FnOnce() -> anyhow::Result<Value> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| err(e.to_string()))?
        .map(Json)
        .map_err(|e| err(e.to_string()))
}
fn err(s: String) -> (StatusCode, Json<Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({"error":s})))
}
async fn guard(headers: HeaderMap, request: axum::extract::Request, next: Next) -> Response {
    // Reject DNS rebinding and cross-origin access to local/private repository data.
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let allowed = host
        .split(':')
        .next()
        .is_some_and(|h| h == "localhost" || h == "127.0.0.1");
    let origin_ok = headers.get("origin").is_none_or(|v| {
        v.to_str()
            .ok()
            .is_some_and(|v| v == format!("http://{host}"))
    });
    let site_ok = headers
        .get("sec-fetch-site")
        .is_none_or(|v| v != "cross-site");
    if !allowed || !origin_ok || !site_ok {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"Only same-origin localhost requests are allowed"})),
        )
            .into_response();
    }
    if request.method() != Method::GET
        && !headers
            .get("content-type")
            .is_some_and(|v| v.to_str().unwrap_or("").starts_with("application/json"))
    {
        return StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response();
    }
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response.headers_mut().insert("content-security-policy","default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'".parse().unwrap());
    response
}
#[derive(Deserialize)]
struct Add {
    full_name: String,
}
async fn image(State(s): State<Store>, Query(q): Query<HashMap<String, String>>) -> Response {
    let Some(url) = q.get("url").cloned() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match tokio::task::spawn_blocking(move || crate::media::read(&s, &url)).await {
        Ok(Ok((bytes, mime))) => (
            [
                ("content-type", mime),
                ("cache-control", "private, max-age=3600"),
            ],
            bytes,
        )
            .into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn repos(State(s): State<Store>) -> ApiResult {
    blocking(move || Ok(json!(s.repos()?))).await
}
async fn add(State(s): State<Store>, Json(input): Json<Add>) -> ApiResult {
    blocking(move || s.add(&input.full_name)).await
}
#[derive(Deserialize)]
struct Remove {
    confirm: bool,
}
async fn removal_preview(
    State(s): State<Store>,
    Path((o, r)): Path<(String, String)>,
) -> ApiResult {
    blocking(move || crate::launcher::removal_preview(&s, &format!("{o}/{r}"))).await
}
async fn remove_repo(
    State(s): State<Store>,
    Path((o, r)): Path<(String, String)>,
    Json(input): Json<Remove>,
) -> ApiResult {
    blocking(move || {
        anyhow::ensure!(input.confirm, "Removal must be confirmed");
        s.remove(&format!("{o}/{r}"))
    })
    .await
}
async fn sync_repo(State(s): State<Store>, Path((o, r)): Path<(String, String)>) -> ApiResult {
    blocking(move || sync::sync(&s, &format!("{o}/{r}"), false, false)).await
}
async fn items(
    State(s): State<Store>,
    Path((o, r)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult {
    blocking(move || {
        let mut result = s.items(
            &format!("{o}/{r}"),
            q.get("kind").map(String::as_str).unwrap_or("issue"),
            q.get("state").map(String::as_str).unwrap_or("open"),
            q.get("q").map(String::as_str).unwrap_or(""),
        )?;
        if let Some(label) = q.get("label").filter(|label| !label.is_empty()) {
            result.retain(|item| {
                item["labels"]
                    .as_array()
                    .is_some_and(|labels| labels.iter().any(|l| l["name"].as_str() == Some(label)))
            });
        }
        Ok(json!(result))
    })
    .await
}
async fn labels(State(s): State<Store>, Path((o, r)): Path<(String, String)>) -> ApiResult {
    blocking(move || {
        let snapshot = s.load(&format!("{o}/{r}"))?;
        let labels: std::collections::BTreeSet<String> = snapshot
            .items
            .values()
            .flat_map(|v| v["labels"].as_array().into_iter().flatten())
            .filter_map(|v| v["name"].as_str().map(str::to_owned))
            .collect();
        Ok(json!(labels))
    })
    .await
}
async fn detail(State(s): State<Store>, Path((o, r, n)): Path<(String, String, u64)>) -> ApiResult {
    blocking(move || repository::detail(&s, &format!("{o}/{r}"), n)).await
}
async fn tree(
    State(s): State<Store>,
    Path((o, r)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult {
    blocking(move || {
        repository::tree(
            &s,
            &format!("{o}/{r}"),
            q.get("ref").map(String::as_str).unwrap_or("HEAD"),
            q.get("path").map(String::as_str).unwrap_or(""),
        )
    })
    .await
}
async fn blob(
    State(s): State<Store>,
    Path((o, r)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult {
    blocking(move || {
        repository::blob(
            &s,
            &format!("{o}/{r}"),
            q.get("ref").map(String::as_str).unwrap_or("HEAD"),
            q.get("path").map(String::as_str).unwrap_or(""),
        )
    })
    .await
}

async fn outgoing(State(s): State<Store>, Query(q): Query<HashMap<String, String>>) -> Response {
    let md = q.get("format").is_some_and(|v| v == "md");
    let result = blocking(move || {
        outbox::report(
            &s,
            q.get("repo").map(String::as_str),
            q.get("target").map(String::as_str),
        )
    })
    .await;
    match result {
        Ok(Json(value)) if md => match outbox::markdown(&value) {
            Ok(text) => (
                [
                    ("content-type", "text/markdown; charset=utf-8"),
                    (
                        "content-disposition",
                        "attachment; filename=outgoing-changes.md",
                    ),
                ],
                text,
            )
                .into_response(),
            Err(e) => err(e.to_string()).into_response(),
        },
        other => other.into_response(),
    }
}
async fn queue(State(s): State<Store>, Json(intent): Json<outbox::Intent>) -> ApiResult {
    blocking(move || Ok(serde_json::to_value(outbox::enqueue(&s, intent)?)?)).await
}
#[derive(Deserialize)]
struct OutgoingSync {
    repo: Option<String>,
    #[serde(default)]
    dry_run: bool,
    plan_token: Option<String>,
    #[serde(default = "default_pull")]
    pull: bool,
}
fn default_pull() -> bool {
    true
}
async fn send_outgoing(State(s): State<Store>, Json(input): Json<OutgoingSync>) -> ApiResult {
    blocking(move || {
        if let Some(repo) = &input.repo {
            crate::store::validate_repo(repo)?;
        }
        anyhow::ensure!(
            input.dry_run || input.plan_token.is_some(),
            "Sync requires the reviewed plan_token from a dry-run preview"
        );
        outbox::sync(
            &s,
            input.repo.as_deref(),
            input.dry_run,
            input.pull,
            input.plan_token.as_deref(),
        )
    })
    .await
}
#[derive(Deserialize)]
struct Resolution {
    action: String,
}
async fn resolve_outgoing(
    State(s): State<Store>,
    Path(id): Path<String>,
    Json(input): Json<Resolution>,
) -> ApiResult {
    blocking(move || {
        Ok(serde_json::to_value(outbox::resolve(
            &s,
            &id,
            &input.action,
        )?)?)
    })
    .await
}
#[derive(Deserialize)]
struct Launch {
    tool: String,
    path: Option<String>,
}
async fn launch_context(State(s): State<Store>, Path((o, r)): Path<(String, String)>) -> ApiResult {
    blocking(move || crate::launcher::context(&s, &format!("{o}/{r}"))).await
}
async fn launch(
    State(s): State<Store>,
    Path((o, r)): Path<(String, String)>,
    Json(input): Json<Launch>,
) -> ApiResult {
    blocking(move || {
        crate::launcher::open(&s, &format!("{o}/{r}"), &input.tool, input.path.as_deref())
    })
    .await
}
async fn bind(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    match tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await {
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!("Port {port} is occupied; selecting a free port…");
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await
        }
        result => result,
    }
}
pub fn open_browser(url: &str) {
    let url = url.to_owned();
    std::thread::spawn(move || {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "explorer.exe"
        } else {
            "xdg-open"
        };
        if let Err(error) = crate::process::run(program, &[&url], 15) {
            eprintln!("Open {url} in your browser ({error})");
        }
    });
}
pub async fn serve(
    store: Store,
    port: u16,
    web: PathBuf,
    no_browser: bool,
    url_file: Option<PathBuf>,
) -> anyhow::Result<()> {
    let index = web.join("index.html");
    anyhow::ensure!(
        index.exists(),
        "Frontend build missing at {}; run ogh app install --web-dir /path/to/web/dist after building the UI, or set --web-dir",
        web.display()
    );
    let app = Router::new()
        .route(
            "/api/health",
            get(|State(s): State<Store>| async move {
                Json(json!({"status":"ok","mode":"offline_first","outgoing_queue":true,"desktop_launch":true,"separate_pull_push":true,"data_dir":s.root}))
            }),
        )
        .route("/api/media", get(image))
        .route("/api/outbox", get(outgoing).post(queue))
        .route("/api/outbox/sync", post(send_outgoing))
        .route("/api/outbox/{id}/resolve", post(resolve_outgoing))
        .route("/api/repos", get(repos).post(add))
        .route("/api/repos/{o}/{r}/sync", post(sync_repo))
        .route("/api/repos/{o}/{r}/remove", get(removal_preview).post(remove_repo))
        .route("/api/repos/{o}/{r}/open", get(launch_context).post(launch))
        .route("/api/repos/{o}/{r}/labels", get(labels))
        .route("/api/repos/{o}/{r}/items", get(items))
        .route("/api/repos/{o}/{r}/items/{n}", get(detail))
        .route("/api/repos/{o}/{r}/tree", get(tree))
        .route("/api/repos/{o}/{r}/blob", get(blob))
        .fallback_service(ServeDir::new(web).fallback(ServeFile::new(index)))
        .with_state(store)
        .layer(middleware::from_fn(guard));
    let listener = bind(port).await?;
    let url = format!("http://{}", listener.local_addr()?);
    if let Some(path) = url_file {
        std::fs::write(path, &url)?;
    }
    eprintln!("Offline GitHub → {url} (Ctrl-C to stop)");
    if !no_browser {
        open_browser(&url);
    }
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn occupied_port_uses_another_listener() {
        let occupied = super::bind(0).await.unwrap();
        let port = occupied.local_addr().unwrap().port();
        let next = super::bind(port).await.unwrap();
        assert_ne!(next.local_addr().unwrap().port(), port);
        assert!(next.local_addr().unwrap().ip().is_loopback());
    }
}
