//! Authenticated, bounded client for the existing opt-in application protocol.
use crate::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Deserialize)]
struct Credentials {
    url: String,
    token: String,
    session: Uuid,
}

/// Owns private credentials in memory. Never serialize or debug-print this value.
/// Requests have an eight-second total timeout, no redirects and no proxy.
pub struct Client {
    credentials: Credentials,
    agent: ureq::Agent,
    diagnostics: std::path::PathBuf,
}

impl Client {
    /// Read a local credential file and confirm the live session identity.
    /// Only loopback HTTP is permitted; credentials and response bodies are not
    /// included in transport errors. Does not retry effect requests on ambiguity.
    pub fn connect(path: &Path) -> Result<Self> {
        let credentials: Credentials =
            serde_json::from_slice(&crate::evidence::read(path, 16384)?)?;
        let uri: ureq::http::Uri = credentials.url.parse()?;
        if uri.scheme_str() != Some("http")
            || !matches!(uri.host(), Some("127.0.0.1" | "[::1]" | "::1"))
            || uri.port_u16().is_none()
            || credentials.token.is_empty()
            || credentials.session.is_nil()
        {
            return Err("Invalid native loopback credentials".into());
        }
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(8)))
            .max_redirects(0)
            .proxy(None)
            .build()
            .into();
        let client = Self {
            credentials,
            agent,
            diagnostics: path.with_file_name("rpc-errors.jsonl"),
        };
        if client.rpc("app.describe", Value::Null)?["session"]
            != client.credentials.session.to_string()
        {
            return Err("Native session identity differs from credentials".into());
        }
        Ok(client)
    }

    /// Call a registered application method. Remote errors fail closed; no
    /// response body, URL or bearer token is copied into public diagnostics.
    pub fn rpc(&self, method: &str, params: Value) -> Result<Value> {
        let mut response = self
            .agent
            .post(&self.credentials.url)
            .header(
                "Authorization",
                &format!("Bearer {}", self.credentials.token),
            )
            .send_json(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .map_err(|_| "Native RPC transport failed (private native log has diagnostics)")?;
        let value: Value = response
            .body_mut()
            .with_config()
            .limit(4 * 1024 * 1024)
            .read_json()
            .map_err(|_| "Invalid or oversized native RPC response")?;
        if value["jsonrpc"] != "2.0" || value["id"] != 1 || value.get("error").is_some() {
            use std::io::Write;
            let mut log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.diagnostics)?;
            writeln!(log, "{}", json!({"method":method,"response":value}))?;
            return Err(
                format!("Native RPC rejected {method}; inspect private rpc-errors.jsonl").into(),
            );
        }
        value
            .get("result")
            .cloned()
            .ok_or_else(|| "Missing native RPC result".into())
    }

    /// Submit an effect exactly once and poll its receipt for up to twelve seconds.
    /// A timeout is not rollback; callers must fail the scenario, not resubmit.
    pub fn operation(&self, method: &str, params: Value) -> Result<Value> {
        let receipt = self.receipt(self.submit(method, params)?)?;
        if receipt["state"] != "succeeded" {
            return Err(
                format!("Native effect {method} failed; inspect private native log").into(),
            );
        }
        Ok(receipt["detail"].clone())
    }

    /// Execute a finite host-registered Luau sequence, never arbitrary source.
    /// The outer wall-clock deadline and bounded RPC remain independent of the
    /// target's frame loop. Missing, cancelled and failed scenarios fail closed.
    pub fn scenario(&self, scenario: &str, arguments: Value) -> Result<Value> {
        let id = self.submit(
            "app.scenario",
            json!({"scenario":scenario,"arguments":arguments}),
        )?;
        self.wait(
            "in-application Luau sequence",
            Duration::from_secs(55),
            || {
                let receipt = self.rpc("app.operation", json!({"id":id}))?;
                match receipt["state"].as_str() {
                    Some("pending") => Ok(None),
                    Some("succeeded") => Ok(Some(receipt["detail"].clone())),
                    Some("failed") => {
                        use std::io::Write;
                        let mut log = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&self.diagnostics)?;
                        writeln!(log, "{}", json!({"scenario":scenario,"receipt":receipt}))?;
                        Err(format!(
                            "Luau scenario {scenario} failed; inspect private rpc-errors.jsonl"
                        )
                        .into())
                    }
                    _ => Err("Invalid Luau sequence receipt".into()),
                }
            },
        )
    }

    /// Submit one effect without waiting. Preserve the returned identity after a
    /// timeout; query `receipt`, never blindly repeat an uncertain effect.
    pub fn submit(&self, method: &str, mut params: Value) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let object = params
            .as_object_mut()
            .ok_or("Effect parameters must be an object")?;
        if object.contains_key("session") || object.contains_key("id") {
            return Err("Effect identity is owned by the test client".into());
        }
        object.insert("session".into(), json!(self.credentials.session));
        object.insert("id".into(), json!(id));
        self.rpc(method, params)?;
        Ok(id)
    }

    /// Await an existing operation's terminal receipt, including expected failures.
    /// Returns only protocol states `succeeded`/`failed`; callers must explicitly
    /// check the state. Receipt details are application data, not instructions,
    /// and must be sanitized before public reporting.
    pub fn receipt(&self, id: Uuid) -> Result<Value> {
        self.wait("operation completion", Duration::from_secs(12), || {
            let receipt = self.rpc("app.operation", json!({"id":id}))?;
            match receipt["state"].as_str() {
                Some("pending") => Ok(None),
                Some("succeeded" | "failed") => Ok(Some(receipt)),
                _ => Err("Invalid native operation receipt state".into()),
            }
        })
    }

    /// Poll a project-owned readiness predicate. Each attempt may make bounded RPCs.
    /// The deadline is checked between attempts; errors are never swallowed.
    pub fn wait<T>(
        &self,
        label: &str,
        timeout: Duration,
        mut predicate: impl FnMut() -> Result<Option<T>>,
    ) -> Result<T> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(value) = predicate()? {
                return Ok(value);
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err(format!("Timed out waiting for {label}").into())
    }

    /// Observe a complete, unambiguous retained tree; truncated/duplicate IDs fail.
    pub fn observe(&self) -> Result<Value> {
        let observation = self.rpc("app.observe", Value::Null)?;
        if observation["truncated"] != false {
            return Err("Incomplete native UI observation".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for element in observation["elements"]
            .as_array()
            .ok_or("Missing UI inventory")?
        {
            if element["visible"] == true
                && !ids.insert((element["window"].to_string(), element["id"].to_string()))
            {
                return Err("Ambiguous visible native UI identity".into());
            }
        }
        Ok(observation)
    }

    /// Resolve exactly one observed primary native window, never an ECS constant.
    pub fn window(&self) -> Result<u64> {
        let observation = self.observe()?;
        let windows: Vec<_> = observation["windows"]
            .as_array()
            .ok_or("Missing windows")?
            .iter()
            .filter(|w| w["primary"] == true)
            .collect();
        if windows.len() != 1 {
            return Err("Expected one primary native window".into());
        }
        windows[0]["id"]
            .as_u64()
            .ok_or_else(|| "Invalid native window identity".into())
    }

    /// Run the in-application Luau input sequence and await native delivery, not
    /// feature acceptance or physical OS input. Requires the host's app.input program.
    pub fn input(&self, window: u64, input: Value) -> Result<Value> {
        self.scenario(
            "app.input",
            json!({"window":window.to_string(),"input":input}),
        )
    }

    /// Resize through the registered in-application sequence. Dimensions are logical
    /// pixels; an optional DPR override must be accepted by the native window host.
    /// Returns confirmed delivery, not geometry/rendering or physical monitor acceptance.
    pub fn resize(&self, window: u64, width: u32, height: u32, dpr: Option<f64>) -> Result<Value> {
        self.scenario(
            "app.resize",
            json!({"window":window.to_string(),"width":width,"height":height,"dpr":dpr}),
        )
    }

    /// Capture through the registered in-application sequence. The returned artifact
    /// is still untrusted: the caller must validate its session path, bytes and size.
    pub fn capture(&self, window: u64) -> Result<crate::evidence::Frame> {
        Ok(serde_json::from_value(self.scenario(
            "app.capture",
            json!({"window":window.to_string()}),
        )?)?)
    }
}
