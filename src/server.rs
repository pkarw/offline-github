use crate::{repository, store::Store, sync};
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
async fn repos(State(s): State<Store>) -> ApiResult {
    blocking(move || Ok(json!(s.repos()?))).await
}
async fn add(State(s): State<Store>, Json(input): Json<Add>) -> ApiResult {
    blocking(move || s.add(&input.full_name)).await
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
        Ok(json!(s.items(
            &format!("{o}/{r}"),
            q.get("kind").map(String::as_str).unwrap_or("issue"),
            q.get("state").map(String::as_str).unwrap_or("open"),
            q.get("q").map(String::as_str).unwrap_or("")
        )?))
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
pub async fn serve(store: Store, port: u16, web: PathBuf) -> anyhow::Result<()> {
    let index = web.join("index.html");
    anyhow::ensure!(
        index.exists(),
        "Frontend build missing at {}; run npm ci && npm run build in web/, or set --web-dir",
        web.display()
    );
    let app = Router::new()
        .route(
            "/api/health",
            get(|| async { Json(json!({"status":"ok","read_only":true})) }),
        )
        .route("/api/repos", get(repos).post(add))
        .route("/api/repos/{o}/{r}/sync", post(sync_repo))
        .route("/api/repos/{o}/{r}/items", get(items))
        .route("/api/repos/{o}/{r}/items/{n}", get(detail))
        .route("/api/repos/{o}/{r}/tree", get(tree))
        .route("/api/repos/{o}/{r}/blob", get(blob))
        .fallback_service(ServeDir::new(web).not_found_service(ServeFile::new(index)))
        .with_state(store)
        .layer(middleware::from_fn(guard));
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    eprintln!("Offline GitHub → http://127.0.0.1:{port} (Ctrl-C to stop)");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await?;
    Ok(())
}
