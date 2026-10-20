//! Authenticated, bounded HTTP transport for Bevy's BRP mailbox.
//! Axum/Tokio own HTTP, body limits and shutdown; Bevy owns JSON-RPC execution.
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use bevy::prelude::*;
use bevy_remote::{BrpError, BrpMessage, BrpRequest, BrpResponse, BrpSender};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Write,
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use subtle::ConstantTimeEq;

#[cfg(test)]
#[path = "transport_tests.rs"]
mod api_tests;

/// Local connection descriptor. Treat this file like a credential; never commit
/// it or include it in normal logs/artifacts shared with other users.
#[derive(Clone, Serialize, Deserialize)]
pub struct Endpoint {
    /// Loopback URL. This service intentionally does not bind public interfaces.
    pub url: String,
    /// Random capability token for this process's automation authority.
    pub token: String,
    /// Process session used for stale-target checks.
    pub session: String,
}
#[derive(Resource)]
struct Listener(Mutex<Option<TcpListener>>);
#[derive(Resource)]
struct Credentials {
    endpoint: Endpoint,
    path: PathBuf,
}
impl Drop for Credentials {
    fn drop(&mut self) {
        // Do not unlink a user replacement or follow a replacement symlink.
        let owned = std::fs::symlink_metadata(&self.path).is_ok_and(|meta| meta.is_file())
            && std::fs::read(&self.path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Endpoint>(&bytes).ok())
                .is_some_and(|endpoint| {
                    endpoint.session == self.endpoint.session
                        && endpoint.token == self.endpoint.token
                });
        if owned {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
#[derive(Clone)]
struct ServerState {
    token: Arc<String>,
    sender: async_channel::Sender<BrpMessage>,
    slots: Arc<tokio::sync::Semaphore>,
}
#[derive(Resource)]
struct ServerTask {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for ServerTask {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Enable remote access explicitly. Binds loopback synchronously, publishes an
/// owner-only Unix credential file without overwriting, and starts at Startup.
/// On Windows the containing directory must already have an appropriate ACL.
/// The token grants application-user capabilities; this is not an OS sandbox.
pub fn enable(
    app: &mut App,
    port: u16,
    credential_file: &Path,
    artifact_root: &Path,
) -> Result<Endpoint, String> {
    if app.world().contains_resource::<Credentials>() {
        return Err("Remote access is already enabled".into());
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .map_err(|e| format!("Cannot bind automation listener: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let endpoint = Endpoint {
        url: format!("http://127.0.0.1:{port}/"),
        token: format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        ),
        session: app.world().resource::<crate::AppSession>().id.clone(),
    };
    let parent = credential_file
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(&endpoint).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist_noclobber(credential_file)
        .map_err(|e| format!("Cannot publish automation credential without overwriting: {e}"))?;
    app.insert_resource(Listener(Mutex::new(Some(listener))))
        .insert_resource(Credentials {
            endpoint: endpoint.clone(),
            path: credential_file.into(),
        })
        .insert_resource(crate::capture::ArtifactRoot(artifact_root.into()))
        .add_systems(Startup, start);
    Ok(endpoint)
}
fn start(world: &mut World) {
    let result = (|| -> Result<ServerTask, String> {
        let listener = world
            .resource::<Listener>()
            .0
            .lock()
            .map_err(|_| "Automation listener lock poisoned")?
            .take()
            .ok_or("Automation listener already started")?;
        let state = ServerState {
            token: Arc::new(world.resource::<Credentials>().endpoint.token.clone()),
            sender: (**world.resource::<BrpSender>()).clone(),
            slots: Arc::new(tokio::sync::Semaphore::new(16)),
        };
        let (stop, receiver) = tokio::sync::oneshot::channel();
        let thread=std::thread::Builder::new().name("kedra-automation-http".into()).spawn(move || {
            let runtime=match tokio::runtime::Builder::new_current_thread().enable_all().build() {Ok(runtime)=>runtime,Err(error)=>{error!("Cannot start automation runtime: {error}");return;}};
            runtime.block_on(async move {
                let listener=match tokio::net::TcpListener::from_std(listener) {Ok(listener)=>listener,Err(error)=>{error!("Cannot adopt automation listener: {error}");return;}};
                let server=axum::serve(listener,router(state)).into_future();
                tokio::select! {
                    result=server=>{if let Err(error)=result {error!("Automation transport stopped: {error}");}},
                    _=receiver=>{},
                }
            });
            runtime.shutdown_timeout(Duration::from_millis(100));
        }).map_err(|e|e.to_string())?;
        Ok(ServerTask {
            stop: Some(stop),
            thread: Some(thread),
        })
    })();
    match result {
        Ok(task) => {
            world.insert_resource(task);
        }
        Err(error) => {
            error!("{error}");
            world.write_message(AppExit::error());
        }
    }
}
fn router(state: ServerState) -> Router {
    Router::new()
        .route("/", post(rpc))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), authorize))
        .with_state(state)
}
async fn authorize(State(state): State<ServerState>, request: Request, next: Next) -> Response {
    // Browser origins are intentionally not supported, including same-machine
    // web pages. There is no wildcard CORS or unauthenticated discovery route.
    if request.headers().contains_key("origin") {
        return StatusCode::FORBIDDEN.into_response();
    }
    let supplied = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if supplied.len() != state.token.len()
        || !bool::from(supplied.as_bytes().ct_eq(state.token.as_bytes()))
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(_slot) = state.slots.clone().try_acquire_owned() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    match tokio::time::timeout(Duration::from_secs(6), next.run(request)).await {
        Ok(response) => response,
        Err(_) => StatusCode::REQUEST_TIMEOUT.into_response(),
    }
}
async fn rpc(State(state): State<ServerState>, Json(value): Json<Value>) -> Response {
    if let Value::Array(batch) = value {
        if batch.is_empty() || batch.len() > 16 {
            return Json(error(None, -32600, "Batch must contain 1..16 requests")).into_response();
        }
        let mut responses = Vec::new();
        for value in batch {
            if let Some(response) = one(&state, value).await {
                responses.push(response);
            }
        }
        if responses.is_empty() {
            StatusCode::NO_CONTENT.into_response()
        } else {
            Json(responses).into_response()
        }
    } else {
        match one(&state, value).await {
            Some(response) => Json(response).into_response(),
            None => StatusCode::NO_CONTENT.into_response(),
        }
    }
}
fn error(id: Option<Value>, code: i16, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
async fn one(state: &ServerState, value: Value) -> Option<Value> {
    let id = value.get("id").cloned();
    let request = match serde_json::from_value::<BrpRequest>(value) {
        Ok(request) => request,
        Err(_) => return Some(error(id, -32600, "Invalid JSON-RPC 2.0 request")),
    };
    if request.method.contains("+watch") {
        return Some(error(
            id,
            -32601,
            "Use bounded observations; streaming is not exposed",
        ));
    }
    let (sender, receiver) = async_channel::bounded(1);
    let execution = async {
        state
            .sender
            .send(BrpMessage {
                method: request.method,
                params: request.params,
                sender,
            })
            .await
            .map_err(|_| ())?;
        receiver.recv().await.map_err(|_| ())
    };
    let result=match tokio::time::timeout(Duration::from_secs(5),execution).await {
        Ok(Ok(result))=>result,
        _=>Err(BrpError {code:-32000,message:"Application response timed out; command execution may still complete. Check its operation receipt before retrying.".into(),data:None}),
    };
    request.id.as_ref()?;
    serde_json::to_value(BrpResponse::new(request.id, result)).ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "Transport security assertions")]
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    #[test]
    fn explicit_enablement_never_overwrites_credentials_and_uses_private_unix_mode() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        std::fs::write(&path, b"existing user content").unwrap();
        let mut app = App::new();
        app.insert_resource(crate::AppSession {
            id: "session".into(),
        });
        assert!(enable(&mut app, 0, &path, directory.path()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"existing user content");
        let path = directory.path().join("new-session.json");
        let endpoint = enable(&mut app, 0, &path, directory.path()).unwrap();
        assert!(endpoint.url.starts_with("http://127.0.0.1:"));
        assert_eq!(endpoint.token.len(), 64);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
                0
            );
        }
        drop(app);
        assert!(!path.exists());
    }
    #[test]
    fn credential_cleanup_preserves_a_replacement_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let endpoint = Endpoint {
            url: "http://127.0.0.1:1/".into(),
            token: "private".into(),
            session: "original".into(),
        };
        std::fs::write(&path, serde_json::to_vec(&endpoint).unwrap()).unwrap();
        let credentials = Credentials {
            endpoint: endpoint.clone(),
            path: path.clone(),
        };
        std::fs::write(&path, b"user replacement").unwrap();
        drop(credentials);
        assert_eq!(std::fs::read(&path).unwrap(), b"user replacement");
        std::fs::write(&path, serde_json::to_vec(&endpoint).unwrap()).unwrap();
        drop(Credentials {
            endpoint,
            path: path.clone(),
        });
        assert!(!path.exists());
    }
    #[test]
    fn http_rejects_missing_wrong_auth_browser_origin_and_oversized_body() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            // Use a real Bevy mailbox resource rather than constructing its
            // private sender wrapper or bypassing the public BRP contract.
            let mut app = App::new();
            app.add_plugins((
                MinimalPlugins,
                bevy::asset::AssetPlugin::default(),
                crate::KedraAppPlugin,
            ));
            app.update();
            let state = ServerState {
                token: Arc::new("a".repeat(64)),
                sender: (**app.world().resource::<BrpSender>()).clone(),
                slots: Arc::new(tokio::sync::Semaphore::new(16)),
            };
            for token in [None, Some("Bearer wrong")] {
                let mut request = Request::builder()
                    .method("POST")
                    .uri("/")
                    .header("content-type", "application/json");
                if let Some(token) = token {
                    request = request.header("authorization", token);
                }
                let response = router(state.clone())
                    .oneshot(request.body(Body::from("{}")).unwrap())
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            }
            let request = Request::builder()
                .method("POST")
                .uri("/")
                .header("authorization", format!("Bearer {}", state.token))
                .header("origin", "http://example.invalid")
                .body(Body::empty())
                .unwrap();
            assert_eq!(
                router(state.clone())
                    .oneshot(request)
                    .await
                    .unwrap()
                    .status(),
                StatusCode::FORBIDDEN
            );
            let request = Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", state.token))
                .body(Body::from("x".repeat(65537)))
                .unwrap();
            assert_eq!(
                router(state).oneshot(request).await.unwrap().status(),
                StatusCode::PAYLOAD_TOO_LARGE
            );
        });
    }
}
