#![allow(clippy::unwrap_used, reason = "Headless HTTP/mailbox API assertions")]
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;

fn host() -> (App, ServerState) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::input::InputPlugin,
        WindowPlugin {
            primary_window: None,
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        },
        crate::KedraAppPlugin,
    ));
    app.update();
    let state = ServerState {
        token: Arc::new("a".repeat(64)),
        sender: (**app.world().resource::<BrpSender>()).clone(),
        slots: Arc::new(tokio::sync::Semaphore::new(16)),
        wakeup: kedra_wakeup::Wakeup::default(),
    };
    (app, state)
}
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
async fn send(app: &mut App, state: ServerState, value: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri("/")
        .header("authorization", format!("Bearer {}", state.token))
        .header("content-type", "application/json")
        .body(Body::from(value.to_string()))
        .unwrap();
    // Execute through the real Bevy mailbox while Axum awaits each response.
    let (response, ()) = tokio::join!(router(state).oneshot(request), async {
        for _ in 0..40 {
            app.update();
            tokio::task::yield_now().await;
        }
    });
    let response = response.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}

#[test]
fn authenticated_http_executes_real_discovery_observation_and_receipt_errors() {
    runtime().block_on(async {
        let (mut app, state) = host();
        let (status, response) = send(&mut app, state.clone(), json!({"jsonrpc":"2.0","id":"describe","method":"app.describe"})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response["id"], "describe");
        assert_eq!(response["result"]["session"], app.world().resource::<crate::AppSession>().id);
        assert!(response["result"]["methods"].get("app.input").is_some());
        let (_, response) = send(&mut app, state.clone(), json!({"jsonrpc":"2.0","id":2,"method":"app.observe"})).await;
        assert_eq!(response["result"]["windows"], json!([]));
        let window = app.world_mut().spawn(Window::default()).id();
        let operation = uuid::Uuid::new_v4();
        let session = app.world().resource::<crate::AppSession>().id.clone();
        let (_, response) = send(&mut app, state.clone(), json!({"jsonrpc":"2.0","id":"resize","method":"app.resize","params":{"session":session,"id":operation,"window":window.to_bits(),"width":800,"height":600}})).await;
        assert_eq!(response["result"]["state"], "pending");
        let (_, response) = send(&mut app, state.clone(), json!({"jsonrpc":"2.0","id":"receipt","method":"app.operation","params":{"id":operation}})).await;
        assert_eq!(response["result"]["state"], "succeeded");
        assert_eq!(app.world().get::<Window>(window).unwrap().size(), Vec2::new(800.,600.));
        let (_, response) = send(&mut app, state.clone(), json!({"jsonrpc":"2.0","id":3,"method":"app.operation","params":{"id":uuid::Uuid::new_v4()}})).await;
        assert_eq!(response["error"]["code"], -32602);
        let (_, response) = send(&mut app, state.clone(), json!({"jsonrpc":"2.0","id":4,"method":"world.spawn_entity","params":{}})).await;
        assert_eq!(response["error"]["code"], -32601);
        let (_, response) = send(&mut app, state, json!({"jsonrpc":"2.0","id":5,"method":"app.input","params":{"unknown":true}})).await;
        assert_eq!(response["error"]["code"], -32602);
    });
}

#[test]
fn batches_notifications_malformed_requests_and_streaming_have_explicit_contracts() {
    runtime().block_on(async {
        let (mut app, state) = host();
        for batch in [json!([]), Value::Array(vec![json!({}); 17])] {
            let (_, response) = send(&mut app, state.clone(), batch).await;
            assert_eq!(response["error"]["code"], -32600);
        }
        let (_, response) = send(&mut app, state.clone(), json!({"id":17})).await;
        assert_eq!(response["id"], 17);
        assert_eq!(response["error"]["code"], -32600);
        let (_, response) = send(
            &mut app,
            state.clone(),
            json!([
                {"jsonrpc":"2.0","id":1,"method":"app.describe"},
                {"jsonrpc":"2.0","method":"app.describe"},
                {"jsonrpc":"2.0","id":2,"method":"app.observe+watch"}
            ]),
        )
        .await;
        let batch = response.as_array().unwrap();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0]["result"]["protocol"], 1);
        assert_eq!(batch[1]["error"]["code"], -32601);
        for notification in [
            json!({"jsonrpc":"2.0","method":"app.describe"}),
            json!([{"jsonrpc":"2.0","method":"app.describe"}]),
        ] {
            let (status, response) = send(&mut app, state.clone(), notification).await;
            assert_eq!(status, StatusCode::NO_CONTENT);
            assert!(response.is_null());
        }
    });
}

#[test]
fn concurrency_slots_reject_and_recover_and_same_length_wrong_tokens_are_denied() {
    runtime().block_on(async {
        let (mut app, state) = host();
        let permits = state.slots.clone().try_acquire_many_owned(16).unwrap();
        let (status, _) = send(&mut app, state.clone(), json!({})).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        drop(permits);
        let (status, response) = send(
            &mut app,
            state.clone(),
            json!({"jsonrpc":"2.0","id":1,"method":"app.describe"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(response.get("result").is_some());
        let request = Request::builder()
            .method("POST")
            .uri("/")
            .header("authorization", format!("Bearer {}b", "a".repeat(63)))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            router(state).oneshot(request).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    });
}

#[test]
fn closed_mailbox_returns_uncertain_execution_error_without_replaying() {
    runtime().block_on(async {
        let (sender, receiver) = async_channel::bounded(1);
        drop(receiver);
        let state = ServerState {
            token: Arc::new("private".into()),
            sender,
            slots: Arc::new(tokio::sync::Semaphore::new(1)),
            wakeup: kedra_wakeup::Wakeup::default(),
        };
        let response = one(
            &state,
            json!({"jsonrpc":"2.0","id":9,"method":"app.describe"}),
        )
        .await
        .unwrap();
        assert_eq!(response["id"], 9);
        assert_eq!(response["error"]["code"], -32000);
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("may still complete")
        );
    });
}

#[test]
fn duplicate_enablement_and_occupied_ports_leave_credentials_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let (mut app, _) = host();
    let path = directory.path().join("session.json");
    enable(&mut app, 0, &path, directory.path()).unwrap();
    assert!(!path.exists());
    app.update();
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        enable(&mut app, 0, &path, directory.path())
            .err()
            .unwrap()
            .contains("already enabled")
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let (mut other, _) = host();
    let rejected = directory.path().join("rejected.json");
    assert!(
        enable(
            &mut other,
            occupied.local_addr().unwrap().port(),
            &rejected,
            directory.path()
        )
        .is_err()
    );
    assert!(!rejected.exists());
}

#[test]
fn listener_startup_and_drop_own_the_server_thread_and_credential() {
    use std::io::{Read, Write};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.json");
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        crate::KedraAppPlugin,
    ));
    let endpoint = enable(&mut app, 0, &path, directory.path()).unwrap();
    // Plugin/GPU initialization and Startup have not run: a bound listener must
    // not advertise a mailbox that cannot yet execute discovery.
    assert!(!path.exists());
    let unpublished = path.clone();
    app.add_systems(
        Last,
        (move || assert!(!unpublished.exists()))
            .run_if(bevy::ecs::schedule::common_conditions::run_once),
    );
    app.update();
    assert!(app.world().resource::<ServerTask>().thread.is_some());
    assert!(path.is_file());
    let published: Endpoint = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(published.session, endpoint.session);
    // Submit discovery exactly once through the actual bound HTTP listener,
    // immediately after publication, while the host continues its normal ticks.
    let request = std::thread::spawn(move || {
        let address = published
            .url
            .strip_prefix("http://")
            .unwrap()
            .trim_end_matches('/');
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(8)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(8)))
            .unwrap();
        let body = json!({"jsonrpc":"2.0","id":1,"method":"app.describe"}).to_string();
        write!(stream, "POST / HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", published.token, body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"));
        let (_, body) = response.split_once("\r\n\r\n").unwrap();
        serde_json::from_str::<Value>(body).unwrap()
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while !request.is_finished() && std::time::Instant::now() < deadline {
        app.update();
        std::thread::yield_now();
    }
    assert!(request.is_finished(), "First discovery did not complete");
    let response = request.join().unwrap();
    assert_eq!(response["result"]["session"], endpoint.session);
    assert!(response.get("error").is_none());
    drop(app);
    assert!(!path.exists());
}

#[test]
fn unpublished_matching_destination_is_preserved_on_drop_and_failed_publication() {
    for update in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let (mut app, _) = host();
        let endpoint = enable(&mut app, 0, &path, directory.path()).unwrap();
        let bytes = serde_json::to_vec(&endpoint).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        if update {
            app.update();
            assert!(!app.world().resource::<Credentials>().published);
            assert!(!app.world().contains_resource::<ServerTask>());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        drop(app);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn server_initialization_failure_never_publishes_and_releases_listener() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.json");
    let (mut app, _) = host();
    let endpoint = enable(&mut app, 0, &path, directory.path()).unwrap();
    start_with(app.world_mut(), |listener| {
        // Exercise the initialization helper's real error boundary in its owned
        // thread; returning an error must close the listener before returning.
        drop(listener);
        Err("Deterministic HTTP runtime/listener setup failure".into())
    });
    assert!(!path.exists());
    assert!(!app.world().resource::<Credentials>().published);
    assert!(!app.world().contains_resource::<ServerTask>());
    assert!(
        app.world()
            .resource::<Messages<AppExit>>()
            .get_cursor()
            .read(app.world().resource::<Messages<AppExit>>())
            .any(AppExit::is_error)
    );
    let address = endpoint
        .url
        .strip_prefix("http://")
        .unwrap()
        .trim_end_matches('/');
    let _released = TcpListener::bind(address).unwrap();
    drop(app);
    assert!(!path.exists());
}

#[test]
fn missing_startup_listener_reports_application_failure() {
    let (mut app, _) = host();
    app.insert_resource(Listener(Mutex::new(None)));
    start(app.world_mut());
    assert!(!app.world().contains_resource::<ServerTask>());
    assert!(
        app.world()
            .resource::<Messages<AppExit>>()
            .get_cursor()
            .read(app.world().resource::<Messages<AppExit>>())
            .any(AppExit::is_error)
    );
}

#[test]
fn readiness_publication_rejects_a_path_created_during_startup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("session.json");
    let (mut app, _) = host();
    enable(&mut app, 0, &path, directory.path()).unwrap();
    std::fs::write(&path, b"user replacement").unwrap();
    app.update();
    assert_eq!(std::fs::read(&path).unwrap(), b"user replacement");
    assert!(!app.world().contains_resource::<ServerTask>());
    assert!(
        app.world()
            .resource::<Messages<AppExit>>()
            .get_cursor()
            .read(app.world().resource::<Messages<AppExit>>())
            .any(AppExit::is_error)
    );
    drop(app);
    assert_eq!(std::fs::read(&path).unwrap(), b"user replacement");
}

#[test]
fn mailbox_timeout_keeps_the_already_queued_effect_and_reports_uncertainty() {
    runtime().block_on(async {
        let (sender, receiver) = async_channel::bounded(1);
        let state = ServerState {
            token: Arc::new("private".into()),
            sender,
            slots: Arc::new(tokio::sync::Semaphore::new(1)),
            wakeup: kedra_wakeup::Wakeup::default(),
        };
        let response = one(
            &state,
            json!({"jsonrpc":"2.0","id":1,"method":"app.describe"}),
        )
        .await
        .unwrap();
        assert_eq!(response["error"]["code"], -32000);
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("operation receipt")
        );
        let message = receiver.try_recv().unwrap();
        assert_eq!(message.method, "app.describe");
        assert!(message.sender.try_send(Ok(json!({"late":true}))).is_err());
    });
}
