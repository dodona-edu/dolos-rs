use axum::Router;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use std::io::{Error, Result};
use std::path::{Path, PathBuf};
use tower_http::services::ServeDir;

/// Where the report is served, and whether to open a browser.
pub struct Server {
    pub host: String,
    pub port: u16,
    pub open_browser: bool,
}

/// Serve `report_dir` until the process is stopped.
pub fn serve(report_dir: &Path, server: &Server) -> Result<()> {
    let router = build_router(report_dir.to_path_buf());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind((server.host.as_str(), server.port)).await?;
        let url = format!("http://{}:{}", server.host, server.port);
        println!("Dolos is available on {url}");

        if server.open_browser {
            println!("Opening the web page in your browser...");
            // A browser that does not start is not fatal: the server is already usable.
            if let Err(e) = open::that_detached(&url) {
                eprintln!("Could not open the browser: {e}");
            }
        }

        println!("Press Ctrl-C to exit.");
        axum::serve(listener, router).await.map_err(Error::other)
    })
}

/// Route the report directory and the frontend.
fn build_router(report_dir: PathBuf) -> Router {
    Router::new()
        // The frontend reads the report from `/data`.
        // TODO: the TypeScript frontend also reads `kgrams.csv`, which this CLI does not write yet.
        .nest_service("/data", ServeDir::new(report_dir))
        // TODO: serve the dolos-web frontend assets here once the Rust CLI moves into
        // dodona-edu/dolos. Every non-`/data` path must resolve against the frontend webroot,
        // and a path that ends in `/` must resolve to its `index.html`.
        .fallback(frontend_unavailable)
}

/// Placeholder for the frontend that is not bundled yet.
async fn frontend_unavailable() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "The dolos-web frontend is not bundled in this build. The report data is served under /data.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    /// `/data` serves the report files; every other path falls back to the
    /// missing frontend.
    #[tokio::test]
    async fn test_routes() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("pairs.csv"), "file1_id\n").unwrap();
        let router = build_router(dir.path().to_path_buf());

        let get = |path: &str| {
            router
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        };

        assert_eq!(
            get("/data/pairs.csv").await.unwrap().status(),
            StatusCode::OK
        );
        assert_eq!(
            get("/data/missing.csv").await.unwrap().status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(get("/").await.unwrap().status(), StatusCode::NOT_FOUND);
    }
}
