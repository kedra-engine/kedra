"""Explicit offline regression check for the reviewed Gitleaks provenance policy.

Requires Python 3.11+ and Gitleaks 8.24.2. No service credentials or network calls.
Evidence is retained under target/; missing/wrong tools fail rather than skip.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[2]
VERSION = "8.24.2"


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def tool_version(binary):
    result = subprocess.run(
        [binary, "version"], capture_output=True, text=True, encoding="utf-8", check=True
    )
    if result.stdout.strip().removeprefix("v") == VERSION:
        return
    # `go install ...@v8.24.2` does not set the release version linker variable.
    go = shutil.which("go")
    require(go, "Cannot verify Go-installed scanner version without go")
    info = subprocess.run(
        [go, "version", "-m", binary], capture_output=True, text=True,
        encoding="utf-8", check=True
    )
    require(
        any(
            line.split()[:3]
            == ["mod", "github.com/zricethezav/gitleaks/v8", "v" + VERSION]
            for line in info.stdout.splitlines()
        ),
        "Gitleaks must resolve to pinned version " + VERSION,
    )


def inventory(config):
    require(set(config) == {"title", "extend", "rules"}, "Unexpected config scope")
    require(config["extend"] == {"useDefault": True}, "Must retain default rules")
    require(len(config["rules"]) == 1, "Only generic-api-key may be extended")
    rule = config["rules"][0]
    require(set(rule) == {"id", "allowlists"}, "Detector attributes must be inherited")
    require(rule["id"] == "generic-api-key", "Unexpected rule override")
    require(len(rule["allowlists"]) == 22, "Expected 22 reviewed file bindings")
    files = {}
    for entry in rule["allowlists"]:
        require(
            set(entry)
            == {"description", "condition", "paths", "regexTarget", "regexes"},
            "Unexpected allowlist criteria",
        )
        require(entry["condition"] == "AND", "Path/content must use AND")
        require(entry["regexTarget"] == "match", "Must bind individual finding")
        require(len(entry["paths"]) == 1, "Each table must bind one exact path")
        pattern = entry["paths"][0]
        match = re.fullmatch(
            r"\^docs/ui/corpus/text/([a-z-]+)\\\.json\$", pattern
        )
        require(match is not None, "Path must be anchored and literal")
        path = Path("docs/ui/corpus/text") / (match[1] + ".json")
        require(path not in files, "Duplicate path binding")
        data = json.loads((ROOT / path).read_text(encoding="utf-8"))
        require(len(data["instances"]) == 1, "Ambiguous component provenance")
        expected = [
            '^file_key": "' + re.escape(data["file_key"]) + '"$',
            '^key": "' + re.escape(data["instances"][0]["key"]) + '"$',
        ]
        require(entry["regexes"] == expected, "Exact field/value binding changed")
        files[path] = data
    manifest = json.loads(
        (ROOT / "docs/ui/corpus/text/text-manifest.json").read_text(encoding="utf-8")
    )
    require(set(manifest["cases"]) == {p.stem for p in files}, "Corpus scope changed")
    for path in files:
        require(
            hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
            == manifest["cases"][path.stem]["metadata"]["sha256"],
            "Metadata no longer matches provenance manifest",
        )
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--gitleaks", default="gitleaks")
    args = parser.parse_args()
    binary = shutil.which(args.gitleaks)
    require(binary, "Install pinned Gitleaks; scanner regression cannot be skipped")
    # Scanner cases run in isolated working directories; retain the caller's
    # resolved tool location even when --gitleaks is a relative executable path.
    binary = str(Path(binary).resolve(strict=True))
    tool_version(binary)
    config = ROOT / ".gitleaks.toml"
    files = inventory(tomllib.loads(config.read_text(encoding="utf-8")))
    output_root = ROOT / "target"
    output_root.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="gitleaks-policy-", dir=output_root))
    default = output / "defaults.toml"
    default.write_text("[extend]\nuseDefault = true\n", encoding="utf-8")
    env = {k: v for k, v in os.environ.items() if not k.startswith("GITLEAKS_")}

    def scan(label, contents, count, rule="generic-api-key", policy=config):
        directory = output / label
        directory.mkdir()
        for path, text in contents.items():
            destination = directory / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(text, encoding="utf-8")
        report = output / (label + ".json")
        command = [
            binary, "dir", ".", "--config", str(policy), "--redact",
            "--no-banner", "--ignore-gitleaks-allow", "--gitleaks-ignore-path",
            str(directory), "--report-format", "json", "--report-path", str(report),
        ]
        result = subprocess.run(
            command, cwd=directory, env=env, capture_output=True, text=True,
            encoding="utf-8"
        )
        (output / (label + ".log")).write_text(
            result.stdout + result.stderr, encoding="utf-8"
        )
        require(report.is_file(), label + ": scanner produced no report")
        findings = json.loads(report.read_text(encoding="utf-8"))
        require(result.returncode == (1 if count else 0), label + ": wrong exit")
        require(len(findings) == count, label + ": wrong finding count")
        require(all(f["RuleID"] == rule for f in findings), label + ": wrong rule")
        print(f"PASS {label}: {len(findings)} findings", flush=True)

    original = {path: (ROOT / path).read_text(encoding="utf-8") for path in files}
    scan("default-public-ids", original, 44, policy=default)
    scan("reviewed-public-ids", original, 0)
    # Deterministic synthetic values: never use an actual account credential.
    fake = hashlib.sha256(b"Kedra offline detector-policy synthetic value").hexdigest()
    variants = {
        "replaced-file-id": {}, "replaced-component-id": {},
        "cross-file-component-id": {}, "swapped-field-values": {},
        "neighbor-credential": {}, "same-line-credential": {}, "additional-key": {},
        "known-id-different-field": {}, "unrelated-path": {},
        "path-prefix": {}, "path-suffix": {}, "provider-rule": {},
    }
    keys = [data["instances"][0]["key"] for data in files.values()]
    for index, (path, data) in enumerate(files.items()):
        file_id, component_id = data["file_key"], data["instances"][0]["key"]
        text = original[path]
        variants["replaced-file-id"][path] = text.replace(file_id, fake)
        variants["replaced-component-id"][path] = text.replace(component_id, fake)
        variants["cross-file-component-id"][path] = text.replace(
            component_id, keys[(index + 1) % len(keys)]
        )
        variants["swapped-field-values"][path] = json.dumps({
            "file_key": component_id, "key": file_id
        }, indent=2)
        variants["neighbor-credential"][path] = text[:-2] + (
            ',\n  "api_key": "' + fake + '"\n}\n'
        )
        variants["same-line-credential"][path] = (
            '{"file_key": "' + file_id + '", "api_key": "' + fake + '"}\n'
        )
        variants["additional-key"][path] = text[:-2] + (
            ',\n  "additional": {"key": "' + fake + '"}\n}\n'
        )
        variants["known-id-different-field"][path] = json.dumps({
            "api_key": component_id
        })
        variants["unrelated-path"][Path("unrelated") / path.name] = text
        variants["path-prefix"][Path("other") / path] = text
        variants["path-suffix"][Path(str(path) + ".backup")] = text
        # A provider-specific default detector must also remain active.
        provider = "gh" + "p_" + hashlib.sha256(fake.encode("utf-8")).hexdigest()[:36]
        variants["provider-rule"][path] = '{"credential": "' + provider + '"}\n'
    for label, contents in variants.items():
        count = 44 if label in {
            "swapped-field-values", "unrelated-path", "path-prefix", "path-suffix"
        } else 22
        rule = "github-pat" if label == "provider-rule" else "generic-api-key"
        scan(label, contents, count, rule)
    print(f"PASS policy structure and 14 scanner cases; evidence: {output}")


if __name__ == "__main__":
    main()
