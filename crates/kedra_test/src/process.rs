//! Native child ownership. Hosts authorize graceful shutdown, never this library.
use crate::{Client, Result};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// Owns one native process, private log and fresh credentials. Drop kills and
/// reaps an unfinished child as a safety net; only `finish` proves clean shutdown.
pub struct NativeProcess {
    child: Child,
    credentials: PathBuf,
}

impl NativeProcess {
    /// Start a project-configured command. The private directory must be new;
    /// creates `native.log` there with owner-only directory access on Unix.
    /// Caller supplies matching credential/artifact arguments in `command`.
    pub fn spawn(command: &mut Command, private: &Path) -> Result<Self> {
        if let Some(parent) = private.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir(private)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(private, fs::Permissions::from_mode(0o700))?;
        }
        let log = fs::File::create_new(private.join("native.log"))?;
        eprintln!(
            "Native diagnostics: {}",
            private.join("native.log").display()
        );
        let child = command
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()?;
        Ok(Self {
            child,
            credentials: private.join("session.json"),
        })
    }

    /// Wait up to 45 seconds for credentials, rejecting early process exit.
    /// Connection failures retain the private log; Drop still reaps the child.
    pub fn connect(&mut self) -> Result<Client> {
        let deadline = Instant::now() + Duration::from_secs(45);
        loop {
            self.require_running()?;
            if self.credentials.is_file() {
                return Client::connect(&self.credentials);
            }
            if Instant::now() >= deadline {
                return Err("Native startup timed out; inspect private native.log".into());
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Reject a host that exited before completing the sequence.
    pub fn require_running(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_some() {
            return Err("Native host exited before sequence completion".into());
        }
        Ok(())
    }

    /// Invoke the project's shutdown authorization and require successful exit
    /// within ten seconds. A host may retire transport before an input receipt;
    /// the callback owns that interpretation. Callback errors and forced cleanup
    /// always fail this operation. Credentials are removed even on failure.
    pub fn finish(mut self, shutdown: impl FnOnce() -> Result<()>) -> Result<()> {
        self.require_running()?;
        let authorization = shutdown();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait()? {
                authorization?;
                if !status.success() {
                    return Err("Native host exited unsuccessfully".into());
                }
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err("Native host required forced cleanup; graceful shutdown failed".into())
    }
}

impl Drop for NativeProcess {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = fs::remove_file(&self.credentials);
    }
}

/// Establish an unlocked native desktop without persisting user identity.
/// Linux requires an explicit X11 display; this is not hosted certification.
pub fn ensure_desktop() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ioreg")
            .args(["-n", "Root", "-d", "1", "-a"])
            .output()?;
        if !output.status.success() {
            return Err("Cannot inspect native console".into());
        }
        let value = plist::Value::from_reader(std::io::Cursor::new(output.stdout))?;
        let root = value
            .as_dictionary()
            .ok_or("Invalid native console registry")?;
        let users = root
            .get("IOConsoleUsers")
            .and_then(plist::Value::as_array)
            .ok_or("Missing native console users")?;
        let active: Vec<_> = users
            .iter()
            .filter_map(plist::Value::as_dictionary)
            .filter(|user| {
                user.get("kCGSSessionOnConsoleKey")
                    .and_then(plist::Value::as_boolean)
                    == Some(true)
            })
            .collect();
        if root
            .get("IOConsoleLocked")
            .and_then(plist::Value::as_boolean)
            != Some(false)
            || active.len() != 1
            || active[0]
                .get("kCGSessionLoginDoneKey")
                .and_then(plist::Value::as_boolean)
                != Some(true)
            || active[0]
                .get("CGSSessionScreenIsLocked")
                .is_some_and(|v| v.as_boolean() != Some(false))
        {
            return Err("Native capture requires a confirmed unlocked console".into());
        }
    }
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_none() {
        return Err("Native capture requires X11 and a window manager".into());
    }
    Ok(())
}
