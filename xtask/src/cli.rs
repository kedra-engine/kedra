//! Parsing and terminal task selection use clap and dialoguer.
use clap::{Parser, ValueEnum};
use std::io::IsTerminal;

#[derive(Parser)]
#[command(
    name = "cargo xtask",
    about = "Kedra repository checks with live logs and a run summary"
)]
pub(crate) struct Options {
    /// Task to run; omit to choose in an interactive terminal.
    #[arg(value_enum)]
    pub task: Option<Task>,
    /// Disable prompts and terminal animation (automatic in CI or redirected output).
    #[arg(long)]
    pub non_interactive: bool,
    #[command(flatten)]
    pub visual: crate::visual::Options,
    /// With test, arguments after -- select an ordinary native scenario command.
    #[arg(last = true)]
    pub test_arguments: Vec<std::ffi::OsString>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum Task {
    Check,
    Files,
    Architecture,
    Fmt,
    Lint,
    Test,
    Doc,
    Features,
    Visual,
}

pub(crate) enum Stage {
    Files,
    Architecture,
    Cargo {
        label: &'static str,
        args: &'static [&'static str],
        rustdoc: bool,
    },
}

impl Stage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Files => "File growth policy",
            Self::Architecture => "Dependency architecture",
            Self::Cargo { label, .. } => label,
        }
    }
}

impl Task {
    pub fn name(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Files => "files",
            Self::Architecture => "architecture",
            Self::Fmt => "fmt",
            Self::Lint => "lint",
            Self::Test => "test",
            Self::Doc => "doc",
            Self::Features => "features",
            Self::Visual => "visual",
        }
    }

    pub fn stages(self) -> Vec<Stage> {
        match self {
            Self::Visual => Vec::new(),
            Self::Features => vec![
                Stage::Cargo {
                    label: "Default features",
                    args: &["check", "--workspace", "--all-targets", "--locked"],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Minimal features",
                    args: &[
                        "check",
                        "--workspace",
                        "--all-targets",
                        "--no-default-features",
                        "--locked",
                    ],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Independent application runtime",
                    args: &[
                        "check",
                        "-p",
                        "kedra_app",
                        "--lib",
                        "--no-default-features",
                        "--locked",
                    ],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Independent Luau runtime",
                    args: &[
                        "check",
                        "-p",
                        "kedra_script",
                        "--lib",
                        "--no-default-features",
                        "--locked",
                    ],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Independent scene data",
                    args: &[
                        "check",
                        "-p",
                        "kedra_scene",
                        "--lib",
                        "--no-default-features",
                        "--locked",
                    ],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Profiling-disabled application contract",
                    args: &[
                        "test",
                        "-p",
                        "kedra_profiling",
                        "--no-default-features",
                        "--locked",
                    ],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Independent asset data",
                    args: &[
                        "check",
                        "-p",
                        "kedra_assets",
                        "--lib",
                        "--no-default-features",
                        "--locked",
                    ],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Independent UI runtime",
                    args: &[
                        "check",
                        "-p",
                        "kedra_ui",
                        "--lib",
                        "--no-default-features",
                        "--locked",
                    ],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Headless readiness adapter",
                    args: &[
                        "check",
                        "-p",
                        "kedra_wakeup",
                        "--lib",
                        "--no-default-features",
                        "--locked",
                    ],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Shipping diagnostic boundary",
                    args: &[
                        "test",
                        "-p",
                        "kedra_app",
                        "--lib",
                        "--profile",
                        "release-boundary",
                        "--no-default-features",
                        "--locked",
                        "launch::tests",
                    ],
                    rustdoc: false,
                },
            ],
            Self::Check => [
                Self::Files,
                Self::Architecture,
                Self::Fmt,
                Self::Lint,
                Self::Test,
                Self::Doc,
            ]
            .into_iter()
            .flat_map(Self::stages)
            .collect(),
            Self::Files => vec![Stage::Files],
            Self::Architecture => vec![Stage::Architecture],
            Self::Fmt => vec![Stage::Cargo {
                label: "Rust formatting",
                args: &["fmt", "--all", "--", "--check"],
                rustdoc: false,
            }],
            Self::Lint => vec![Stage::Cargo {
                label: "Clippy",
                args: &[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--locked",
                    "--",
                    "-F",
                    "unsafe_code",
                    "-D",
                    "warnings",
                ],
                rustdoc: false,
            }],
            Self::Test => vec![
                Stage::Cargo {
                    label: "Unit and integration tests",
                    args: &[
                        "test",
                        "--workspace",
                        "--all-targets",
                        "--all-features",
                        "--locked",
                    ],
                    rustdoc: false,
                },
                Stage::Cargo {
                    label: "Documentation tests",
                    args: &["test", "--workspace", "--doc", "--all-features", "--locked"],
                    rustdoc: false,
                },
            ],
            Self::Doc => vec![Stage::Cargo {
                label: "API documentation",
                args: &[
                    "doc",
                    "--workspace",
                    "--no-deps",
                    "--all-features",
                    "--locked",
                ],
                rustdoc: true,
            }],
        }
    }
}

pub(crate) fn interactive(options: &Options) -> bool {
    !options.non_interactive
        && std::env::var_os("CI").is_none()
        && std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && std::io::stderr().is_terminal()
        && std::env::var("TERM").as_deref() != Ok("dumb")
}

pub(crate) fn choose_task(task: Option<Task>, interactive: bool) -> crate::Result<Task> {
    if let Some(task) = task {
        return Ok(task);
    }
    if !interactive {
        return Err(
            "choose a task explicitly outside an interactive terminal; run cargo xtask --help"
                .into(),
        );
    }
    let tasks = Task::value_variants();
    let selected = dialoguer::Select::new()
        .with_prompt("Kedra: choose a repository check")
        .items(tasks.iter().map(|task| task.name()).collect::<Vec<_>>())
        .default(0)
        .interact_opt()?;
    selected
        .map(|index| tasks[index])
        .ok_or_else(|| "task selection cancelled".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_tasks_never_need_a_prompt() -> crate::Result<()> {
        assert_eq!(choose_task(Some(Task::Fmt), false)?, Task::Fmt);
        assert!(choose_task(None, false).is_err());
        let rust = Options::try_parse_from(["xtask", "test"])?;
        assert_eq!(rust.task, Some(Task::Test));
        assert!(rust.test_arguments.is_empty());
        let native = Options::try_parse_from(["xtask", "test", "--", "features"])?;
        assert_eq!(native.task, Some(Task::Test));
        assert_eq!(
            native.test_arguments,
            [std::ffi::OsString::from("features")]
        );
        Ok(())
    }

    #[test]
    fn check_plan_keeps_all_gates_in_order() {
        let labels: Vec<_> = Task::Check.stages().iter().map(Stage::label).collect();
        assert_eq!(
            labels,
            [
                "File growth policy",
                "Dependency architecture",
                "Rust formatting",
                "Clippy",
                "Unit and integration tests",
                "Documentation tests",
                "API documentation"
            ]
        );
    }

    #[test]
    fn feature_plan_checks_default_minimal_and_independent_runtime_separately() {
        let stages = Task::Features.stages();
        let commands: Vec<_> = stages
            .iter()
            .map(|stage| match stage {
                Stage::Cargo { args, rustdoc, .. } => {
                    assert!(!rustdoc);
                    *args
                }
                _ => panic!("feature checks must run Cargo"),
            })
            .collect();
        assert_eq!(
            commands,
            vec![
                vec!["check", "--workspace", "--all-targets", "--locked"],
                vec![
                    "check",
                    "--workspace",
                    "--all-targets",
                    "--no-default-features",
                    "--locked"
                ],
                vec![
                    "check",
                    "-p",
                    "kedra_app",
                    "--lib",
                    "--no-default-features",
                    "--locked"
                ],
                vec![
                    "check",
                    "-p",
                    "kedra_script",
                    "--lib",
                    "--no-default-features",
                    "--locked"
                ],
                vec![
                    "check",
                    "-p",
                    "kedra_scene",
                    "--lib",
                    "--no-default-features",
                    "--locked"
                ],
                vec![
                    "test",
                    "-p",
                    "kedra_profiling",
                    "--no-default-features",
                    "--locked"
                ],
                vec![
                    "check",
                    "-p",
                    "kedra_assets",
                    "--lib",
                    "--no-default-features",
                    "--locked"
                ],
                vec![
                    "check",
                    "-p",
                    "kedra_ui",
                    "--lib",
                    "--no-default-features",
                    "--locked"
                ],
                vec![
                    "check",
                    "-p",
                    "kedra_wakeup",
                    "--lib",
                    "--no-default-features",
                    "--locked"
                ],
                vec![
                    "test",
                    "-p",
                    "kedra_app",
                    "--lib",
                    "--profile",
                    "release-boundary",
                    "--no-default-features",
                    "--locked",
                    "launch::tests"
                ],
            ]
        );
    }

    #[test]
    fn lint_keeps_the_independent_workspace_unsafe_guard() {
        let stages = Task::Lint.stages();
        let Stage::Cargo { args, .. } = &stages[0] else {
            panic!("lint must invoke Cargo");
        };
        assert!(args.windows(2).any(|pair| pair == ["-F", "unsafe_code"]));
        assert!(args.windows(2).any(|pair| pair == ["-D", "warnings"]));
    }
}
