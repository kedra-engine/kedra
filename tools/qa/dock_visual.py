#!/usr/bin/env python3
"""Recompare stable native docking evidence against independent Figma references.

No native application is launched. Acceptance policy changes do not certify new
input behavior; the source mechanics/stability results remain linked explicitly.
"""
import argparse
import json
from pathlib import Path
import shutil

from cargo_build import build_executable
from dock_fixture import ROOT, compare, digest, verify_references


class CaptureDescription:
    def __init__(self, observation):
        self.description = observation["fixture"]
        self.ui = observation["ui"]

    def fixture(self):
        return self.description

    def observe(self):
        return self.ui


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--captures", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="New report directory")
    parser.add_argument("--visual-policy", choices=("strict", "accepted"), required=True)
    args = parser.parse_args()
    source = args.captures.resolve()
    previous = json.loads((source / "report.json").read_text())
    references = verify_references()
    if (previous.get("stability") != "passed" or not previous.get("mechanics", "").startswith("passed")
            or previous.get("cleanup_errors") or previous.get("error")
            or len(previous["runs"]) < 2):
        raise ValueError("Require complete stable mechanics evidence with normal cleanup")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    report = {"schema": 1, "kind": "offline_visual_recomparison",
              "captures": str(source), "source_report_sha256": digest(source / "report.json"),
              "visual_policy": args.visual_policy, "runs": [], "status": "incomplete"}
    try:
        executable = build_executable(ROOT, package="xtask", manifest=Path("xtask/Cargo.toml"),
                                      name="xtask", kind="bin")
        for index, run in enumerate(previous["runs"], start=1):
            if set(run) != set(references):
                raise ValueError("Native capture set is incomplete")
            directory = args.output / f"run-{index}"
            directory.mkdir()
            results = {}
            report["runs"].append(results)
            for name, entry in references.items():
                original = source / f"run-{index}"
                image = original / f"{name}.actual.png"
                sha = digest(image)
                if sha != run[name]["sha256"] or sha != previous["runs"][0][name]["sha256"]:
                    raise ValueError("Native frame was replaced or is not stable across runs")
                observation = json.loads((original / f"{name}.observe.json").read_text())
                evidence = json.loads((original / f"{name}.evidence.json").read_text())
                if (evidence["sha256"] != sha or evidence["scenario"] != name
                        or evidence["fixture"] != "docking.fixture.v1"
                        or evidence["profile"]["logical_size"] != entry["size"]
                        or evidence["profile"]["dpr"] != 1):
                    raise ValueError("Stored capture provenance does not match the native frame")
                shutil.copyfile(image, directory / image.name)
                matched = compare(name, entry, {"frame": evidence["receipt_frame"]}, sha,
                                  CaptureDescription(observation), directory,
                                  executable=executable, visual_policy=args.visual_policy,
                                  capture_profile=evidence["profile"])
                results[name] = {"sha256": sha, "visual_passed": matched}
                print(f"run {index}: {name}: {args.visual_policy}: {'PASS' if matched else 'FAIL'}",
                      flush=True)
        report["status"] = ("passed" if all(c["visual_passed"] for run in report["runs"]
                                             for c in run.values()) else "failed")
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
        report["status"] = "failed"
    finally:
        (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Visual {args.visual_policy}: {report['status']} — {args.output / 'report.json'}", flush=True)
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
