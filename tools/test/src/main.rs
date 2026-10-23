//! Concrete native scenarios; all generic native mechanisms live in kedra_test.
mod authoring;
mod capture;
mod catalog;
mod scenarios;
use scenarios::{editor, runtime};
mod project;
mod sources;
use clap::{Parser, Subcommand};
use kedra_test::{Result, evidence, process::NativeProcess};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser)]
#[command(about = "Native Rust tests and explicit source authoring")]
struct Options {
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Concrete shared-project switch contract (not an arbitrary application runner).
    NativeSwitch {
        /// Manifest of a complete materialized test_project game workspace.
        #[arg(long)]
        manifest_path: PathBuf,
        #[arg(long)]
        assets: PathBuf,
        #[arg(long)]
        project_assets: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Concrete independent docking projection contract, with a verified visual delivery.
    NativeProjection {
        #[arg(long)]
        manifest_path: PathBuf,
        #[arg(long)]
        assets: PathBuf,
        #[arg(long)]
        project_assets: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Run native behavior checks with input and GPU fences, without screenshot coverage.
    Features {
        #[arg(long)]
        output: Option<PathBuf>,
        /// Comma-separated subset of the named concrete Rust feature scenarios.
        #[arg(long)]
        only: Option<String>,
    },
    /// Capture and compare the 22 concrete button/field cases in two sessions.
    Components {
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        policy: Option<PathBuf>,
        /// Comma-separated in-scope diagnostic subset; never full acceptance.
        #[arg(long)]
        only: Option<String>,
    },
    /// Explicitly generate a local Figma plugin or import its downloaded bundle.
    Export {
        #[arg(
            long,
            required_unless_present = "text_bundle",
            conflicts_with = "text_bundle"
        )]
        text_plugin: bool,
        #[arg(long)]
        text_bundle: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&evidence::read(path, 1024 * 1024)?)?)
}

fn new_output(root: &Path, requested: Option<PathBuf>) -> Result<PathBuf> {
    let output = if let Some(path) = requested {
        let path = if path.is_absolute() {
            path
        } else {
            root.join(path)
        };
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir(&path)?;
        path
    } else {
        let parent = root.join("target/test/ui-runs");
        fs::create_dir_all(&parent)?;
        tempfile::Builder::new()
            .prefix("rust-")
            .tempdir_in(parent)?
            .keep()
    };
    Ok(output.canonicalize()?)
}

fn command(executable: &Path, args: &[&std::ffi::OsStr], root: &Path) -> Result<()> {
    eprintln!("Running {}…", executable.display());
    if !Command::new(executable)
        .current_dir(root)
        .args(args)
        .status()?
        .success()
    {
        return Err("Native asset command failed".into());
    }
    Ok(())
}

fn comparison_config(
    scenario: &catalog::Scenario,
    source: &sources::Source,
    layer: &str,
    profile: &Value,
) -> Value {
    let [x, y, width, height] = scenario.region();
    json!({"schema":1,"scenario":scenario.id,"fixture":"production-ui-corpus-v1","profile":profile,
        "reference":{"image":"reference.png","sha256":source.assets[layer]["sha256"],
            "source":format!("https://www.figma.com/design/{}/UI-KIT?node-id={}",catalog::FILE,scenario.node.replace(':',"-")),
            "kind":"design_reference","approval":"pending","export_size":[640,360],"crop":[0,0,640,360],
            "crop_reason":"Complete independent Figma export; no crop or resampling."},
        "animation":{"mode":"static"},"comparison":{"normalized_yiq_tolerance":0.05,
            "budget":{"max_changed_pixels":640*360*2/100,"max_changed_ppm":20000},"max_excluded_ppm":0,
            "allow_uniform_reason":if layer=="shape" {Some("Glyph-free diagnostic; nonuniform full frames, retained layout and exact restoration independently required")} else {None}},
        "exclusions":[],"critical_regions":[{"name":"region-0","rect":[x,y,width,height],
            "budget":{"max_changed_pixels":width*height*2/100,"max_changed_ppm":20000}}]})
}

fn components(
    root: &Path,
    output: &Path,
    source_root: &Path,
    policy_path: &Path,
    only: Option<&str>,
) -> Result<()> {
    let started = std::time::Instant::now();
    let public = output.join("public");
    fs::create_dir(&public)?;
    let mut report = json!({"schema":1,"result":"failed","design_acceptance":"not_granted","processes":[],"cases":{},
        "scope":if only.is_some(){"partial"}else{"buttons-and-fields"}});
    let outcome = (|| -> Result<()> {
        let all = catalog::scenarios(root)?;
        let sources = sources::load(source_root, &all)?;
        let families = read_json(&root.join("docs/ui/corpus/text-families.json"))?;
        let policy = read_json(policy_path)?;
        let fingerprint = sources::fingerprint(&sources, &families)?;
        let lock = evidence::file_digest(&root.join("Cargo.lock"))?;
        report["source_contract_sha256"] = json!(fingerprint);
        report["source_manifest_sha256"] = json!(evidence::file_digest(
            &source_root.join("text-manifest.json")
        )?);
        report["policy_sha256"] = json!(evidence::file_digest(policy_path)?);
        report["families_sha256"] = json!(evidence::file_digest(
            &root.join("docs/ui/corpus/text-families.json")
        )?);
        report["actual_dependency_lock_sha256"] = json!(lock);
        report["reviewed_dependency_lock_sha256"] = policy["dependency_lock_sha256"].clone();
        let selected: Vec<_> = all
            .iter()
            .filter(|s| only.is_none_or(|names| names.split(',').any(|name| name == s.id)))
            .collect();
        if let Some(names) = only {
            let names: std::collections::BTreeSet<_> = names.split(',').collect();
            if selected.len() != names.len() || selected.is_empty() {
                return Err("Unsupported diagnostic case selection".into());
            }
        }
        for scenario in &selected {
            report["cases"][&scenario.id] =
                json!({"status":"not_run","runs":[{},{}],"comparisons":{}});
        }
        fs::write(
            public.join("policy.json"),
            evidence::read(policy_path, 1024 * 1024)?,
        )?;
        fs::write(
            public.join("families.json"),
            evidence::read(&root.join("docs/ui/corpus/text-families.json"), 1024 * 1024)?,
        )?;
        sources::copy(source_root, &public.join("source"), &sources)?;
        kedra_test::visual::inspect_reference(
            &public.join("source/integrity-manifest.json"),
            &public.join("source-integrity"),
        )?;
        if policy["scope"] != "catalog-component-buttons-and-fields" {
            return Err("Wrong policy scope".into());
        }
        // Stale locks remain a hard preflight failure, including --only. Never
        // silently refresh hashes, waive policy or launch an unreviewed profile.
        kedra_test::text::verify_profile(&policy, &policy["profile"], &lock, &fingerprint)?;
        let platform = if cfg!(target_os = "macos") {
            "darwin"
        } else {
            std::env::consts::OS
        };
        if policy["profile"]["platform"] != platform {
            return Err("No reviewed policy for this native platform".into());
        }
        kedra_test::process::ensure_desktop()?;
        let manifest = root.join("Cargo.toml");
        let editor = kedra_test::cargo::build(&manifest, "kedra", "kedra", false, &[])?;
        let host = editor.clone();
        let delivery = output.join("private/editor-delivery");
        fs::create_dir_all(output.join("private"))?;
        command(
            &editor,
            &[
                "assets".as_ref(),
                "build".as_ref(),
                root.join("assets/editor").as_os_str(),
                delivery.as_os_str(),
                "--namespace".as_ref(),
                "editor".as_ref(),
            ],
            root,
        )?;
        command(
            &editor,
            &["assets".as_ref(), "verify".as_ref(), delivery.as_os_str()],
            root,
        )?;
        let cases: Vec<_> = selected.iter().map(|s| s.id.clone()).collect();
        let capture_report = kedra_test::suite::capture_twice(
            output,
            &cases,
            |_, private| {
                let artifacts = private.join("frames");
                let mut command = Command::new(&host);
                command
                    .current_dir(root)
                    .arg("--assets")
                    .arg(&delivery)
                    .args([
                        "--scene",
                        "editor://fixtures/catalog.scene.ron",
                        "--remote-port",
                        "0",
                        "--remote-credentials",
                    ])
                    .arg(private.join("session.json"))
                    .arg("--remote-artifacts")
                    .arg(&artifacts);
                let mut native = NativeProcess::spawn(&mut command, private)?;
                let client = native.connect()?;
                Ok((native, client, artifacts))
            },
            |client, artifacts, case, repeat| {
                let scenario = selected
                    .iter()
                    .find(|s| s.id == case)
                    .ok_or("Unknown concrete scenario")?;
                capture::scenario(client, artifacts, &public, repeat, scenario, &policy)
            },
            |client| {
                let _ = client.input(client.window()?, json!({"kind":"key","key":"f12"}));
                Ok(())
            },
        )?;
        report["processes"] = capture_report["processes"].clone();
        report["cases"] = capture_report["cases"].clone();
        for scenario in selected {
            let result = (|| -> Result<Value> {
                kedra_test::suite::verify_case(
                    &report,
                    &scenario.id,
                    &["full", "shape", "restored"],
                )?;
                let runs = report["cases"][&scenario.id]["runs"].clone();
                let source = &sources[&scenario.id];
                let family = scenario.family(&families)?;
                if source.metadata["roles"]
                    .as_object()
                    .ok_or("Missing source roles")?
                    .keys()
                    .ne(family["roles"]
                        .as_object()
                        .ok_or("Missing family roles")?
                        .keys())
                {
                    return Err("Independent source role inventory differs from family".into());
                }
                let mut result = report["cases"][&scenario.id].clone();
                result["stability"] = json!("passed");
                result["expected_roles"] = source.metadata["roles"].clone();
                result["actual_roles"] =
                    kedra_test::text::bind_roles(&runs[0]["full"]["snapshot"], &family["roles"])?;
                let mut errors = Vec::new();
                for run in runs.as_array().ok_or("Missing runs")? {
                    let actual =
                        kedra_test::text::bind_roles(&run["full"]["snapshot"], &family["roles"])?;
                    let typography =
                        kedra_test::text::typography(&source.metadata["roles"], &actual, &policy)?;
                    errors.extend(
                        typography["errors"]
                            .as_array()
                            .ok_or("Invalid typography report")?
                            .iter()
                            .cloned(),
                    );
                }
                result["typography"] =
                    json!({"status":if errors.is_empty(){"passed"}else{"failed"},"errors":errors});
                for layer in ["full", "shape"] {
                    let actual = &runs[0][layer];
                    result["comparisons"][layer] = kedra_test::comparison::compare(
                        &public,
                        &PathBuf::from(format!("{}/{layer}", scenario.id)),
                        &sources::local(source_root, &source.assets[layer]["file"])?,
                        &public.join(actual["image"].as_str().ok_or("Missing native image")?),
                        comparison_config(scenario, source, layer, &actual["profile"]),
                        actual,
                    )?;
                }
                result["shape"] = kedra_test::text::shape(
                    &scenario.id,
                    &result["comparisons"]["shape"],
                    &runs[0]["shape"],
                    &source.assets["shape"]["sha256"],
                    &policy,
                    &lock,
                    &fingerprint,
                )?;
                result["status"] = if result["typography"]["status"] == "passed" {
                    result["shape"]["status"].clone()
                } else {
                    json!("failed")
                };
                Ok(result)
            })();
            match result {
                Ok(result) => report["cases"][&scenario.id] = result,
                Err(error) => {
                    report["cases"][&scenario.id]["status"] = json!("failed");
                    report["cases"][&scenario.id]["error"] = json!(error.to_string());
                }
            }
            kedra_test::report::write(&public, &report)?;
        }
        let cases = report["cases"].as_object().ok_or("Missing cases")?;
        if cases.values().any(|r| {
            !matches!(
                r["status"].as_str(),
                Some("passed" | "accepted_known_difference")
            )
        }) {
            return Err("Native component suite failed; inspect retained evidence".into());
        }
        report["result"] = json!(if cases
            .values()
            .any(|r| r["status"] == "accepted_known_difference")
        {
            "passed_with_accepted_differences"
        } else {
            "passed"
        });
        Ok(())
    })();
    if let Err(error) = &outcome {
        report["error"] = json!(error.to_string());
    }
    report["duration_seconds"] = json!(started.elapsed().as_secs_f64());
    kedra_test::report::write(&public, &report)?;
    eprintln!(
        "Native components: {} — {}",
        report["result"],
        public.join("index.html").display()
    );
    outcome
}

fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    match Options::parse().command {
        Action::NativeSwitch {
            manifest_path,
            assets,
            project_assets,
            output,
        } => {
            let output = new_output(&root, output)?;
            runtime::run_switch(&manifest_path, &assets, &project_assets, &output)
        }
        Action::NativeProjection {
            manifest_path,
            assets,
            project_assets,
            output,
        } => {
            let output = new_output(&root, output)?;
            runtime::run_projection(&manifest_path, &assets, &project_assets, &output)
        }
        Action::Features { output, only } => {
            let output = new_output(&root, output)?;
            editor::run(&root, &output, only.as_deref())
        }
        Action::Components {
            output,
            source,
            policy,
            only,
        } => {
            let output = new_output(&root, output)?;
            components(
                &root,
                &output,
                &root.join(source.unwrap_or_else(|| PathBuf::from("docs/ui/corpus/text"))),
                &root.join(
                    policy.unwrap_or_else(|| PathBuf::from("docs/ui/corpus/text-policy.json")),
                ),
                only.as_deref(),
            )
        }
        Action::Export {
            output,
            text_plugin,
            text_bundle,
        } => {
            let output = new_output(&root, output)?;
            let scenarios = catalog::scenarios(&root)?;
            if text_plugin {
                authoring::plugin(&root, &output.join("plugin"), &scenarios)
            } else {
                authoring::import(
                    &root.join(text_bundle.ok_or("Missing explicit bundle")?),
                    &output.join("exports"),
                    &scenarios,
                )
            }
        }
    }
}
