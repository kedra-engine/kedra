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
    DependencyPolicy,
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
    DependencyPolicy,
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
            Self::DependencyPolicy => "Dependency exception policy",
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
            Self::DependencyPolicy => "dependency-policy",
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
                Self::DependencyPolicy,
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
            Self::DependencyPolicy => vec![Stage::DependencyPolicy],
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
    fn check_admits_repository_policy_before_running_cargo_gates() {
        let stages = Task::Check.stages();
        let Some(first_cargo) = stages
            .iter()
            .position(|stage| matches!(stage, Stage::Cargo { .. }))
        else {
            panic!("check must execute Cargo gates");
        };
        assert!(
            stages[..first_cargo]
                .iter()
                .any(|s| matches!(s, Stage::Files))
        );
        assert!(
            stages[..first_cargo]
                .iter()
                .any(|s| matches!(s, Stage::Architecture))
        );
        assert!(
            stages[..first_cargo]
                .iter()
                .any(|s| matches!(s, Stage::DependencyPolicy))
        );
        for command in ["fmt", "clippy", "test", "doc"] {
            assert!(stages.iter().any(
                |stage| matches!(stage, Stage::Cargo { args, .. } if args.first() == Some(&command))
            ));
        }
        assert!(
            stages
                .iter()
                .any(|stage| matches!(stage, Stage::Cargo { args, .. } if args.contains(&"--doc")))
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
        assert!(
            commands
                .iter()
                .all(|args| args.contains(&"--locked") && !args.contains(&"--all-features"))
        );
        for minimal in [false, true] {
            assert!(commands.iter().any(|args| args.first() == Some(&"check")
                && args.contains(&"--workspace")
                && args.contains(&"--all-targets")
                && args.contains(&"--no-default-features") == minimal));
        }
        for package in [
            "kedra_app",
            "kedra_script",
            "kedra_scene",
            "kedra_assets",
            "kedra_ui",
            "kedra_wakeup",
        ] {
            assert!(
                commands.iter().any(|args| args.first() == Some(&"check")
                    && args.windows(2).any(|pair| pair == ["-p", package])
                    && args.contains(&"--lib")
                    && args.contains(&"--no-default-features")
                    && !args.contains(&"--workspace")),
                "missing isolated {package}"
            );
        }
        assert!(commands.iter().any(|args| {
            args.first() == Some(&"test")
                && args
                    .windows(2)
                    .any(|pair| pair == ["-p", "kedra_profiling"])
                && args.contains(&"--no-default-features")
        }));
        assert!(commands.iter().any(|args| {
            args.first() == Some(&"test")
                && args.windows(2).any(|pair| pair == ["-p", "kedra_app"])
                && args
                    .windows(2)
                    .any(|pair| pair == ["--profile", "release-boundary"])
                && args.contains(&"launch::tests")
        }));
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
