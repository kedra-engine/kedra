//! Live child output and progress use duct and indicatif, not a custom TUI.
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(crate) struct Session {
    interactive: bool,
    log: File,
    path: PathBuf,
    bar: Option<ProgressBar>,
    started: Instant,
    results: Vec<(&'static str, bool, Duration)>,
    stdout: Box<dyn Write>,
}

impl Session {
    pub fn new(interactive: bool, task: &str) -> crate::Result<Self> {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/xtask/logs");
        Self::new_at(&directory, interactive, task, Box::new(io::stdout()))
    }

    fn new_at(
        directory: &Path,
        interactive: bool,
        task: &str,
        stdout: Box<dyn Write>,
    ) -> crate::Result<Self> {
        fs::create_dir_all(directory)?;
        let directory = directory.canonicalize()?;
        let (log, path) = tempfile::Builder::new()
            .prefix("run-")
            .suffix(".log")
            .tempfile_in(directory)?
            .keep()?;
        let mut session = Self {
            interactive,
            log,
            path,
            bar: None,
            started: Instant::now(),
            results: Vec::new(),
            stdout,
        };
        session.message(&format!("Kedra / {task}"))?;
        session.message(&format!("Log: {}", session.path.display()))?;
        Ok(session)
    }

    pub fn message(&mut self, message: &str) -> crate::Result<()> {
        writeln!(self.log, "{}", console::strip_ansi_codes(message))?;
        self.log.flush()?;
        if let Some(bar) = &self.bar {
            bar.println(message);
        } else {
            writeln!(self.stdout, "{message}")?;
            self.stdout.flush()?;
        }
        Ok(())
    }

    pub fn stage(
        &mut self,
        index: usize,
        total: usize,
        label: &'static str,
        action: impl FnOnce(&mut Self) -> crate::Result<()>,
    ) -> crate::Result<()> {
        self.message(&format!("\n[{index}/{total}] {label}"))?;
        if self.interactive {
            let bar = ProgressBar::new_spinner();
            bar.set_style(ProgressStyle::with_template(
                "{spinner:.cyan} [{elapsed_precise}] {msg}",
            )?);
            bar.set_message(label);
            bar.enable_steady_tick(Duration::from_millis(100));
            self.bar = Some(bar);
        }
        let started = Instant::now();
        let result = action(self);
        let duration = started.elapsed();
        if let Some(bar) = self.bar.take() {
            bar.finish_and_clear();
        }
        self.results.push((label, result.is_ok(), duration));
        match &result {
            Ok(()) => self.message(&format!("PASS {label} ({:.2}s)", duration.as_secs_f64()))?,
            Err(error) => self.message(&format!(
                "FAIL {label} ({:.2}s): {error}",
                duration.as_secs_f64()
            ))?,
        }
        result
    }

    pub fn cargo(&mut self, args: &[&str], rustdoc: bool) -> crate::Result<()> {
        self.message(&format!("+ cargo {}", args.join(" ")))?;
        let mut command = duct::cmd("cargo", args).stderr_to_stdout();
        if rustdoc {
            command = command.env(
                "RUSTDOCFLAGS",
                "-D warnings -D rustdoc::broken_intra_doc_links",
            );
        }
        // No terminal escapes in redirected logs, even if the caller set colors.
        command = command.env(
            "CARGO_TERM_COLOR",
            if self.interactive { "always" } else { "never" },
        );
        self.stream(command)
    }

    fn stream(&mut self, command: duct::Expression) -> crate::Result<()> {
        // Separate pipe EOF from exit validation: a checked Duct reader can return
        // an error after read_until has buffered an unterminated final diagnostic.
        let reader = command.unchecked().reader()?;
        let mut stream = BufReader::new(reader);
        let mut line = Vec::new();
        loop {
            line.clear();
            let read = stream.read_until(b'\n', &mut line);
            if !line.is_empty() {
                self.message(String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']))?;
            }
            if read? == 0 {
                break;
            }
        }
        // Duct reaps the child at EOF even in unchecked mode. Nonzero status must
        // still fail the stage, but only after every diagnostic has been emitted.
        let status = stream
            .get_ref()
            .try_wait()?
            .ok_or("child still running after EOF")?
            .status;
        if !status.success() {
            return Err(format!("child exited with {status}").into());
        }
        Ok(())
    }

    pub fn finish(&mut self, total: usize) -> crate::Result<()> {
        self.message("\nSummary")?;
        for (label, success, duration) in self.results.clone() {
            self.message(&format!(
                "  {} {label}: {:.2}s",
                if success { "PASS" } else { "FAIL" },
                duration.as_secs_f64()
            ))?;
        }
        let passed = self
            .results
            .iter()
            .filter(|(_, success, _)| *success)
            .count();
        let failed = self.results.len() - passed;
        self.message(&format!(
            "{passed} passed, {failed} failed, {} skipped; total {:.2}s",
            total - self.results.len(),
            self.started.elapsed().as_secs_f64()
        ))?;
        self.message(&format!("Log: {}", self.path.display()))?;
        Ok(())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(bar) = self.bar.take() {
            bar.finish_and_clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_failure_streams_diagnostics_and_retains_log() -> crate::Result<()> {
        let root = tempfile::tempdir()?;
        let mut session = Session::new_at(root.path(), false, "runner-test", Box::new(io::sink()))?;
        let result = session.stage(1, 2, "Intentional child failure", |output| {
            output.cargo(&["kedra-nonexistent-command-for-runner-test"], false)
        });
        assert!(result.is_err());
        session.finish(2)?;
        let log = fs::read_to_string(&session.path)?;
        assert!(log.contains("no such command"));
        assert!(log.contains("FAIL Intentional child failure"));
        assert!(log.contains("0 passed, 1 failed, 1 skipped"));
        assert!(!log.contains('\u{1b}'));
        Ok(())
    }

    #[test]
    fn unterminated_diagnostics_survive_failed_child_exit() -> crate::Result<()> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("fixture.rs");
        let executable = root
            .path()
            .join(format!("output-fixture{}", std::env::consts::EXE_SUFFIX));
        fs::write(
            &source,
            r#"use std::io::Write;
fn main() {
    std::io::stdout().write_all(b"stdout detail\n").unwrap();
    std::io::stderr().write_all("last diagnostic: 🦀".as_bytes()).unwrap();
    if std::env::args().any(|arg| arg == "fail") { std::process::exit(7); }
}"#,
        )?;
        let compilation = std::process::Command::new("rustc")
            .arg(&source)
            .args(["--crate-name", "output_fixture", "-o"])
            .arg(&executable)
            .output()?;
        assert!(
            compilation.status.success(),
            "{}",
            String::from_utf8_lossy(&compilation.stderr)
        );
        for (argument, success) in [("pass", true), ("fail", false)] {
            let mut session = Session::new_at(
                root.path(),
                false,
                "unterminated-test",
                Box::new(io::sink()),
            )?;
            let result = session.stage(1, 1, "Child output", |output| {
                output.stream(duct::cmd(&executable, [argument]).stderr_to_stdout())
            });
            session.finish(1)?;
            assert_eq!(result.is_ok(), success);
            let log = fs::read_to_string(&session.path)?;
            assert!(log.contains("stdout detail"));
            assert!(log.contains("last diagnostic: 🦀"), "{log}");
            assert!(log.contains(if success {
                "1 passed, 0 failed"
            } else {
                "0 passed, 1 failed"
            }));
        }
        Ok(())
    }
}
