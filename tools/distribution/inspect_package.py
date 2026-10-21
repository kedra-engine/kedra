#!/usr/bin/env python3
"""Inspect and execute actual native distributions without building project code."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import zipfile


def run(arguments: list[str], **kwargs) -> subprocess.CompletedProcess:
    print("+ " + " ".join(map(str, arguments)), flush=True)
    return subprocess.run(list(map(str, arguments)), check=True, **kwargs)


def verify_cli(binary: Path, resources: Path, version: str) -> None:
    environment = os.environ.copy()
    environment.pop("KEDRA_ASSETS", None)
    environment["PATH"] = (str(Path(os.environ.get("SYSTEMROOT", "C:/Windows")) / "System32")
                           if os.name == "nt" else "/usr/bin:/bin")
    with tempfile.TemporaryDirectory(prefix="kedra unrelated launch Юникод ") as launch:
        for arguments, expected in [(["--version"], version), (["assets", "verify"], "SUCCESS")]:
            result = run([binary, *arguments], cwd=launch, env=environment,
                         capture_output=True, text=True, timeout=90)
            print(result.stdout + result.stderr, flush=True)
            if expected not in result.stdout + result.stderr:
                raise ValueError("Installed executable did not produce expected redirected diagnostics")
        result = subprocess.run([str(binary), "--invalid-distribution-option"], env=environment,
                                cwd=launch, capture_output=True, text=True, timeout=30)
    if result.returncode == 0 or "error:" not in result.stderr:
        raise ValueError("Installed CLI failures must retain nonzero status and stderr")
    notices = resources.parent / "licenses" / "dependency-notices.html"
    if not notices.is_file() or notices.stat().st_size == 0:
        raise ValueError("Installed dependency license notices missing")
    if not list((notices.parent / "covered-sources").glob("*.crate")):
        raise ValueError("Installed MPL source archives missing")
    if not (resources / "fonts/Inter-OFL.txt").is_file():
        raise ValueError("Installed font license notice missing")


def inspect_macos(package: Path, work: Path, target: str, version: str) -> None:
    run(["hdiutil", "verify", package])
    mount = work / "readonly mount"
    mount.mkdir()
    run(["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, package])
    app = work / "installed Юникод space" / "Kedra.app"
    try:
        shutil.copytree(mount / "Kedra.app", app, symlinks=True)
    finally:
        run(["hdiutil", "detach", mount])
    info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
    if info.get("CFBundleIdentifier") != "dev.kedra.editor" or info.get("CFBundleShortVersionString") != version:
        raise ValueError("Installed app identity differs from release metadata")
    binary = app / "Contents/MacOS/kedra"
    architecture = "arm64" if target.startswith("aarch64") else "x86_64"
    verify_macho(binary, architecture)
    run(["otool", "-L", binary])
    run(["vtool", "-show-build", binary])
    icon = app / "Contents/Resources" / info["CFBundleIconFile"]
    run(["sips", "-g", "pixelWidth", "-g", "pixelHeight", icon])
    verify_cli(binary, app / "Contents/Resources/assets", version)


def verify_macho(binary: Path, architecture: str) -> None:
    run(["lipo", binary, "-verify_arch", architecture])


def inspect_linux(packages: dict[str, Path], work: Path, version: str) -> None:
    portable = work / "portable Юникод space"
    portable.mkdir()
    with tarfile.open(packages["tar.gz"], "r:gz") as archive:
        archive.extractall(portable, filter="data")
    binary = portable / "kedra/kedra"
    header = run(["readelf", "-h", binary], capture_output=True, text=True).stdout
    if "Advanced Micro Devices X86-64" not in header:
        raise ValueError("Portable ELF machine is not x86_64")
    dependencies = run(["ldd", binary], capture_output=True, text=True).stdout
    print(dependencies)
    if "not found" in dependencies:
        raise ValueError("Portable executable has unresolved native libraries")
    symbols = run(["objdump", "-T", binary], capture_output=True, text=True).stdout
    versions = re.findall(r"GLIBC_([0-9]+\.[0-9]+(?:\.[0-9]+)?)", symbols)
    print("Required GLIBC floor:", max(versions, key=lambda value: tuple(map(int, value.split('.')))))
    verify_cli(binary, binary.parent / "assets", version)
    inspect_appimage(packages["appimage"], work, version)


def inspect_appimage(appimage: Path, work: Path, version: str) -> None:
    appimage.chmod(appimage.stat().st_mode | 0o100)
    run([appimage, "--appimage-extract"], cwd=work, stdout=subprocess.DEVNULL)
    appdir = work / "squashfs-root"
    verify_cli(appdir / "AppRun", appdir / "usr/lib/kedra/assets", version)


def inspect_windows(packages: dict[str, Path], work: Path, version: str) -> None:
    portable = work / "portable Юникод space"
    with zipfile.ZipFile(packages["zip"]) as archive:
        archive.extractall(portable)
    binary = portable / "kedra/kedra.exe"
    sysroot = run(["rustc", "--print", "sysroot"], capture_output=True, text=True).stdout.strip()
    reader = Path(sysroot) / "lib/rustlib/x86_64-pc-windows-msvc/bin/llvm-readobj.exe"
    headers = run([reader, "--file-headers", "--coff-resources", binary],
                  capture_output=True, text=True).stdout
    if any(value not in headers for value in ["IMAGE_FILE_MACHINE_AMD64", "IMAGE_SUBSYSTEM_WINDOWS_GUI", "GROUP_ICON"]):
        raise ValueError("Windows PE lacks expected architecture, GUI subsystem or native icon")
    verify_cli(binary, binary.parent / "assets", version)
    install = work / "installed Юникод space"
    run([packages["nsis"], "/S", "/D=" + str(install)], timeout=180)
    verify_cli(install / "kedra.exe", install / "assets", version)
    run([install / "uninstall.exe", "/S"], timeout=180)
    deadline = time.monotonic() + 60
    while (install / "kedra.exe").exists() and time.monotonic() < deadline:
        time.sleep(.2)
    if (install / "kedra.exe").exists():
        raise ValueError("Uninstaller left the installed application executable")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--target", required=True)
    parser.add_argument("--source", required=True, help="Exact SHA, or local for nonpublishable builds")
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    directory = args.directory.resolve()
    inventory = directory / "artifact-inventory.json"
    try:
        data = json.loads(inventory.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"Cannot read package inventory: {error}") from error
    expected_source = None if args.source == "local" else args.source
    if (data["application"] != "kedra" or data["version"] != args.version
            or data["target"] != args.target or data["source_revision"] != expected_source):
        raise ValueError("Package inventory identity differs from the requested source/target")
    packages = {}
    for artifact in data["artifacts"]:
        name = artifact["name"]
        if Path(name).name != name:
            raise ValueError("Invalid package inventory filename")
        path = directory / name
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        if digest != artifact["sha256"] or path.stat().st_size != artifact["bytes"]:
            raise ValueError("Package inventory checksum or size mismatch")
        packages[artifact["format"]] = path
    work = directory / "native-inspection"
    work.mkdir()
    if sys.platform == "darwin" and args.target.endswith("apple-darwin"):
        inspect_macos(packages["dmg"], work, args.target, args.version)
    elif sys.platform == "linux" and args.target == "x86_64-unknown-linux-gnu":
        inspect_linux(packages, work, args.version)
    elif sys.platform == "win32" and args.target == "x86_64-pc-windows-msvc":
        inspect_windows(packages, work, args.version)
    else:
        raise ValueError("Inspection requires the package's native runner OS")
    print("SUCCESS: native package metadata, installed resources, CLI status and notices verified", flush=True)
    print("UNVERIFIED: physical input, OS trust, GUI console appearance and native rendered frames", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"FAILED: native inspection: {error}") from error
