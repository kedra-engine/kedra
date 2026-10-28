//! Assembled editor behavior acceptance; no scenario language or test registry.
mod browser;
mod dock_fixture;
mod docking;
mod edit_scripts;
mod hierarchy;
mod inspector;
mod navigation;
mod navigation_geometry;
mod scene;
mod scripting;
mod startup;
mod support;
mod workspace;
use kedra_test::{Result, process::NativeProcess};
use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};
use support::Editor;

pub(crate) fn run(root: &Path, output: &Path, only: Option<&str>) -> Result<()> {
    let canonical = crate::project::source(root);
    let source_before = crate::project::inventory(&canonical)?;
    let names = [
        "startup",
        "luau",
        "editor-scripts",
        "browser",
        "hierarchy",
        "inspector",
        "inspector-ui",
        "inspector-collections",
        "workspace",
        "window-chrome",
        "docking",
        "docking-states",
        "scene-view",
        "scene-navigation",
        "inspector-fixture",
        "dock-fixture",
    ];
    let selected: Vec<_> = names
        .into_iter()
        .filter(|name| only.is_none_or(|s| s.split(',').any(|n| n == *name)))
        .collect();
    if selected.is_empty() || only.is_some_and(|s| s.split(',').count() != selected.len()) {
        return Err("Unknown or duplicate native feature selection".into());
    }
    let public = output.join("public");
    fs::create_dir(&public)?;
    let mut report =
        json!({"schema":1,"result":"failed","design_acceptance":"not_granted","cases":{}});
    report["scope"] = json!(if only.is_some() {
        "selected_features"
    } else {
        "complete_feature_suite"
    });
    report["verification"] = json!("behavior_only");
    report["project_source"] = json!({"path":"tests/fixtures/test_project",
        "sha256":kedra_test::evidence::digest(&serde_json::to_vec(&source_before)?),
        "working_copies":"private/<scenario>/project"});
    for name in &selected {
        report["cases"][*name] = json!({"status":"not_run"});
    }
    kedra_test::report::write(&public, &report)?;
    let setup = (|| -> Result<_> {
        kedra_test::process::ensure_desktop()?;
        let manifest = root.join("Cargo.toml");
        let editor = kedra_test::cargo::build(&manifest, "kedra", "kedra", false, &[])?;
        let delivery = output.join("private/editor-delivery");
        fs::create_dir_all(output.join("private"))?;
        crate::command(
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
        crate::command(
            &editor,
            &["assets".as_ref(), "verify".as_ref(), delivery.as_os_str()],
            root,
        )?;
        let project_delivery = crate::project::delivery(root, output, &editor)?;
        Ok((editor, delivery, project_delivery))
    })();
    let (editor, delivery, project_delivery) = match setup {
        Ok(value) => value,
        Err(error) => {
            report["setup"] = json!({"status":"failed","error":error.to_string()});
            kedra_test::report::write(&public, &report)?;
            return Err(error);
        }
    };
    for name in selected {
        eprintln!("Native feature: {name}");
        let directory = output.join("private").join(name);
        if name == "dock-fixture" {
            let result = dock_fixture::run(root, output, &delivery);
            let mut case = match result {
                Ok(data) => json!({"status":"passed","evidence":data}),
                Err(error) => json!({"status":"failed","error":error.to_string()}),
            };
            let retained = public.join("dock-fixture/report.json");
            if retained.is_file() {
                let data = crate::read_json(&retained)?;
                case["evidence"] = data;
            }
            report["cases"][name] = case;
            kedra_test::report::write(&public, &report)?;
            continue;
        }
        let private = directory.join("session");
        let state = directory.join("state");
        let artifacts = private.join("frames");
        let mut captures = Vec::new();
        let mut hierarchy_phases = Value::Null;
        let result = (|| -> Result<Value> {
            let project = if name != "inspector-fixture" {
                Some(crate::project::copy(root, &directory.join("project"))?)
            } else {
                None
            };
            let mut command = if name == "inspector-fixture" {
                let mut command = Command::new(&editor);
                command
                    .args(["--scene", "editor://fixtures/inspector.scene.ron"])
                    .arg("--assets")
                    .arg(&delivery)
                    .args(["--remote-port", "0", "--remote-credentials"])
                    .arg(private.join("session.json"))
                    .arg("--remote-artifacts")
                    .arg(&artifacts);
                command
            } else {
                let mut command = Command::new(&editor);
                if !matches!(name, "startup" | "luau") {
                    command
                        .arg(project.as_ref().ok_or("Missing shared project copy")?)
                        .arg("--project-assets")
                        .arg(&project_delivery);
                }
                command
                    .arg("--assets")
                    .arg(&delivery)
                    .args(["--remote-port", "0", "--remote-credentials"])
                    .arg(private.join("session.json"))
                    .arg("--remote-artifacts")
                    .arg(&artifacts)
                    .arg("--state-directory")
                    .arg(&state);
                command
            };
            command.current_dir(root);
            if name == "editor-scripts" {
                command.args([
                    "--editor-script",
                    "Editor/rename-pair.luau",
                    "--edit-source",
                    "game://Hud.prefab.ron",
                    "--edit-source",
                    "game://Cuboid.prefab.ron",
                ]);
            }
            let mut native = NativeProcess::spawn(&mut command, &private)?;
            let client = native.connect()?;
            let c = Editor::new(
                &client,
                root,
                &artifacts,
                &state,
                project.as_deref(),
                "editor.workspace",
            );
            if name != "inspector-fixture" {
                c.isolated()?;
            }
            let execution = (|| -> Result<Value> {
                Ok(match name {
                    "startup" => startup::run(&c)?,
                    "luau" => scripting::run(&c)?,
                    "editor-scripts" => edit_scripts::run(&c)?,
                    "browser" => browser::run(&c)?,
                    "hierarchy" => hierarchy::run(&c, &mut hierarchy_phases)?,
                    "inspector" => inspector::run(&c)?,
                    "inspector-ui" => inspector::ui(&c, false)?,
                    "inspector-collections" => inspector::ui(&c, true)?,
                    "workspace" | "window-chrome" | "docking" | "docking-states" => {
                        let project = c.open_project()?;
                        let mut data = workspace::run(&c, name)?;
                        data["project"] = json!(project);
                        data
                    }
                    "scene-view" | "scene-navigation" => scene::run(
                        &c,
                        name,
                        project.as_deref().ok_or("Missing shared project copy")?,
                    )?,
                    "inspector-fixture" => inspector::fixture(&c)?,
                    _ => return Err("Unsupported feature".into()),
                })
            })();
            if let Err(error) = &execution {
                fs::write(
                    private.join("failure.json"),
                    serde_json::to_vec_pretty(&json!({"error":error.to_string(),
                    "editor":client.rpc("editor.state",Value::Null).ok(),"workspace":c.state().ok(),"observation":client.observe().ok()}))?,
                )?;
            }
            captures = c.captures.borrow().clone();
            let mut data = execution?;
            native.finish(|| {
                let _ = client.input(client.window()?, json!({"kind":"close_window"}));
                Ok(())
            })?;
            if name == "docking" {
                let private = directory.join("restore-session");
                let artifacts = private.join("frames");
                let mut command = Command::new(&editor);
                command
                    .current_dir(root)
                    .arg(support::text(&data["project"])?)
                    .arg("--project-assets")
                    .arg(&project_delivery)
                    .arg("--assets")
                    .arg(&delivery)
                    .args(["--remote-port", "0", "--remote-credentials"])
                    .arg(private.join("session.json"))
                    .arg("--remote-artifacts")
                    .arg(&artifacts)
                    .arg("--state-directory")
                    .arg(&state);
                let mut restored = NativeProcess::spawn(&mut command, &private)?;
                let client = restored.connect()?;
                let check = Editor::new(
                    &client,
                    root,
                    &artifacts,
                    &state,
                    project.as_deref(),
                    "editor.workspace",
                );
                check.isolated()?;
                check.until("restored workspace", || Ok(check.state()?["bound"] == true))?;
                support::check(
                    check.state()?["layout"] == data["layout"],
                    "Persisted docking topology changed on restart",
                )?;
                support::check(
                    support::array(&client.observe()?["windows"])?.len()
                        == support::object(&data["layout"]["hosts"])?.len(),
                    "Persisted native host inventory differs",
                )?;
                let observed = client.observe()?;
                for w in support::array(&observed["windows"])? {
                    check.frame(
                        w["id"].as_u64().ok_or("Invalid restored window")?,
                        "restored docking host",
                    )?;
                }
                restored.finish(|| {
                    let _ = client.input(client.window()?, json!({"kind":"close_window"}));
                    Ok(())
                })?;
                data["restart"] = json!({"status":"passed","frames":check.captures.into_inner()});
            }
            let evidence = public.join(name);
            fs::create_dir_all(&evidence)?;
            fs::write(
                evidence.join("report.json"),
                serde_json::to_vec_pretty(&data)?,
            )?;
            Ok(json!({"status":"passed","evidence":data,"renderer_frames":c.captures.into_inner()}))
        })();
        report["cases"][name] = match result {
            Ok(data) => data,
            Err(error) => {
                eprintln!("FAILED {name}: {error}");
                json!({"status":"failed","error":error.to_string(),"renderer_frames":captures})
            }
        };
        if name == "hierarchy" {
            report["cases"][name]["phases"] = hierarchy_phases;
        }
        kedra_test::report::write(&public, &report)?;
    }
    let unchanged = crate::project::inventory(&canonical)? == source_before;
    report["project_source"]["unchanged"] = json!(unchanged);
    let passed = unchanged
        && report["cases"]
            .as_object()
            .is_some_and(|cases| cases.values().all(|v| v["status"] == "passed"));
    if passed {
        report["result"] = json!("passed");
    }
    kedra_test::report::write(&public, &report)?;
    eprintln!(
        "Native features: {} — {}",
        report["result"],
        public.join("index.html").display()
    );
    if !passed {
        return Err("Native feature failure; inspect retained evidence".into());
    }
    Ok(())
}
