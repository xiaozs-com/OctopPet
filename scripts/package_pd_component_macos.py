"""Package Paldee Pet for macOS with UNSIGNED manifests.

Mirrors `scripts/package_pd_component.py` (Windows) so both platforms publish the
same artifact set and the helper installs them through one path. The differences
are the platform id (`macos-x86_64`, matching `components/platforms.py`), the
Mach-O checks in place of PE checks, and executable bits inside the archive.

No private keys, global catalogs or uploads happen here. Signing the manifest is
a controlled release step.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ID = "octoppet"
PLATFORM = "macos-x86_64"
ENTRY = "paldee-pet"
BRIDGE = "pd-device-bridge"
PROTOCOL = "octoppet-component@1"
CPU_TYPE_X86_64 = 0x01000007
EXECUTABLE_MODE = 0o755
# The helper's archive extraction drops the executable bit on nested binaries,
# so archive permissions are a hint; the caller must restore them after install.
UNIX_CREATE_SYSTEM = 3


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def macho_cpu_type(path):
    with path.open("rb") as file:
        header = file.read(8)
    if len(header) != 8:
        raise ValueError(f"Not a 64-bit Mach-O executable: {path}")
    magic = header[:4]
    if magic == b"\xcf\xfa\xed\xfe":
        order = "<"
    elif magic == b"\xfe\xed\xfa\xcf":
        order = ">"
    else:
        raise ValueError(f"Not a thin 64-bit Mach-O executable: {path}")
    return struct.unpack_from(f"{order}I", header, 4)[0]


def macho_dependencies(path):
    """Non-system dynamic libraries, which a self-contained package may not have."""
    result = subprocess.run(["/usr/bin/otool", "-L", str(path)], capture_output=True, text=True)
    if result.returncode != 0:
        return []
    foreign = []
    for line in result.stdout.splitlines()[1:]:
        target = line.strip().split(" (")[0]
        if not target:
            continue
        if target.startswith(("/usr/lib/", "/System/Library/", "@rpath/", "@loader_path/", "@executable_path/")):
            continue
        foreign.append(target)
    return foreign


def collect_licenses(destination, bridge_root):
    records, missing, seen = [], [], set()
    legal_root = ROOT / "packaging/third-party-licenses"
    sources = json.loads((legal_root / "SOURCES.json").read_text(encoding="utf-8"))

    def collect(name, version, root, license_id, extra=""):
        files = sorted(
            p
            for p in root.iterdir()
            if p.is_file()
            and p.name.upper().startswith(
                ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT", "UNLICENSE")
            )
        )
        if extra and (root / extra).is_file() and (root / extra) not in files:
            files.append(root / extra)
        if not files:
            key = name.removeprefix("rust-") + "-" + version
            if name.startswith("rust-unic-") and version == "0.9.0":
                key = "unic-0.9.0"
            for record in sources.get(key, []):
                path = legal_root / record["file"]
                if hashlib.sha256(path.read_bytes()).hexdigest() != record["sha256"]:
                    raise ValueError(f"Pinned license fingerprint mismatch: {path}")
                files.append(path)
        if not files:
            missing.append(f"{name}@{version}: {license_id}")
            return
        target = destination / re.sub(r"[^a-zA-Z0-9._-]", "_", f"{name}-{version}")
        target.mkdir(parents=True, exist_ok=True)
        for file in files:
            shutil.copy2(file, target / file.name)
        source = (
            f"https://crates.io/api/v1/crates/{name.removeprefix('rust-')}/{version}/download"
            if name.startswith("rust-")
            else f"https://www.npmjs.com/package/{name.removeprefix('npm-')}/v/{version}"
        )
        records.append({"name": name, "version": version, "license": license_id, "source": source})

    lock = json.loads((ROOT / "package-lock.json").read_text(encoding="utf-8"))
    for name, info in lock["packages"].items():
        if not name or info.get("dev"):
            continue
        root = ROOT / name
        package = json.loads((root / "package.json").read_text(encoding="utf-8"))
        collect("npm-" + package["name"], package["version"], root, str(package.get("license", "")))
    for manifest in [ROOT / "src-tauri/Cargo.toml", bridge_root / "native/Cargo.toml"]:
        metadata = json.loads(
            subprocess.check_output(
                [
                    "cargo", "metadata", "--offline", "--locked", "--format-version", "1",
                    "--filter-platform", "x86_64-apple-darwin", "--manifest-path", str(manifest),
                ],
                text=True,
            )
        )
        resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
        for package in metadata["packages"]:
            if package["id"] not in resolved or package["source"] is None or package["id"] in seen:
                continue
            seen.add(package["id"])
            collect(
                "rust-" + package["name"], package["version"],
                Path(package["manifest_path"]).parent,
                package.get("license") or "", package.get("license_file") or "",
            )
    if missing:
        raise ValueError("Missing dependency license texts:\n" + "\n".join(missing))
    write_json(destination / "THIRD_PARTY_NOTICES.json", {"dependencies": records})
    shutil.copy2(legal_root / "SOURCES.json", destination / "PINNED_LICENSE_SOURCES.json")


def validate_archive(archive, manifest, catalog):
    if archive.stat().st_size != manifest["size"]:
        raise ValueError("ZIP byte size mismatch")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if digest != manifest["sha256"]:
        raise ValueError("ZIP SHA-256 mismatch")
    with zipfile.ZipFile(archive) as bundle:
        if bundle.testzip() is not None:
            raise ValueError("ZIP integrity failed")
        for name in bundle.namelist():
            if name.startswith(("/", "\\")) or ".." in name.replace("\\", "/").split("/") or ":" in name:
                raise ValueError("Unsafe ZIP path")
        metadata = json.loads(bundle.read("component.json"))
        if not (
            metadata["id"] == manifest["id"] == catalog["id"] == ID
            and metadata["version"] == manifest["version"]
            and metadata["entry"] == manifest["entry"] == ENTRY
            and metadata["supported_runtime_platforms"] == [manifest["platform"]] == [PLATFORM]
            and list(catalog["manifests"]) == [PLATFORM]
            and metadata["protocol"] == PROTOCOL
        ):
            raise ValueError("Component metadata mismatch")
        if "signature" in catalog or "components" in catalog:
            raise ValueError("Only an unsigned single-component catalog entry is allowed")
        if not {"LICENSE", ENTRY, BRIDGE}.issubset(bundle.namelist()):
            raise ValueError("Entry, bridge or license missing")
        for name in (ENTRY, BRIDGE):
            if not bundle.getinfo(name).external_attr >> 16 & 0o111:
                raise ValueError(f"Archive lost the executable bit: {name}")
        with tempfile.TemporaryDirectory(prefix="paldee-pet-zip-check-") as temporary:
            bundle.extractall(temporary)
            for name in (ENTRY, BRIDGE):
                if macho_cpu_type(Path(temporary) / name) != CPU_TYPE_X86_64:
                    raise ValueError("Incorrect executable architecture")
    return {
        "id": ID, "version": manifest["version"], "platform": PLATFORM, "entry": ENTRY,
        "size": manifest["size"], "sha256": digest, "signature": "pending",
    }


def package(args):
    version = args.version
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Version must be x.y.z")
    base = args.base_url.rstrip("/")
    if not base.startswith("https://") or not base.endswith("/paldee-pet"):
        raise ValueError("Base URL must be an HTTPS paldee-pet directory")
    for path in [args.pet, args.bridge]:
        if macho_cpu_type(path) != CPU_TYPE_X86_64:
            raise ValueError(f"Only macOS x86_64 is supported: {path}")
        foreign = macho_dependencies(path)
        if foreign:
            raise ValueError(f"Unbundled dynamic library dependency: {path.name}: {foreign}")
    if not (args.bridge_root / "LICENSE").is_file():
        raise ValueError("Bridge license is required")
    with tempfile.TemporaryDirectory(prefix="paldee-pet-version-check-") as temporary:
        env = dict(os.environ, OCTOPPET_DATA_DIR=temporary)
        result = subprocess.run(
            [str(args.pet.resolve()), "status", "--json"],
            capture_output=True, text=True, encoding="utf-8", env=env, timeout=20, check=True,
        )
        status = json.loads(result.stdout)
        if status.get("protocol") != PROTOCOL or status.get("version") != version:
            raise ValueError("Executable version/protocol does not match")
    output = args.output.resolve()
    archive = output / version / f"paldee-pet-{version}-{PLATFORM}.zip"
    if archive.exists():
        raise FileExistsError("Immutable version already exists; use a new version or separate output")
    archive.parent.mkdir(parents=True, exist_ok=True)
    metadata = {
        "id": ID,
        "name": json.loads((ROOT / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["productName"],
        "version": version,
        "protocol": PROTOCOL,
        "capabilities": ["desktop.pet@1", "desktop.helper-cli@2"],
        "supported_runtime_platforms": [PLATFORM],
        "entry": ENTRY,
        "launchable": True,
        "settings": [
            {
                "key": "start_with_helper",
                "name": "随“屏幕自动化小助手”启动",
                "description": "默认关闭。保存后，下次启动屏幕自动化小助手时启动桌宠；不设置系统自启动。",
                "value_type": "boolean",
                "default": False,
                "restart_required": False,
            }
        ],
        "retains_user_data_on_uninstall": True,
    }
    with tempfile.TemporaryDirectory(prefix="paldee-pet-package-") as temporary:
        staging = Path(temporary)
        shutil.copy2(args.pet, staging / ENTRY)
        shutil.copy2(args.bridge, staging / BRIDGE)
        shutil.copy2(ROOT / "LICENSE", staging / "LICENSE")
        write_json(staging / "component.json", metadata)
        legal = staging / "licenses"
        legal.mkdir()
        shutil.copy2(args.bridge_root / "LICENSE", legal / "PD_BRIDGE_LICENSE.txt")
        collect_licenses(legal, args.bridge_root)
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
            for path in sorted(staging.rglob("*")):
                if not path.is_file():
                    continue
                info = zipfile.ZipInfo(path.relative_to(staging).as_posix(), (2026, 1, 1, 0, 0, 0))
                info.create_system = UNIX_CREATE_SYSTEM
                if path.relative_to(staging).as_posix() in (ENTRY, BRIDGE):
                    info.external_attr = EXECUTABLE_MODE << 16
                bundle.writestr(info, path.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
    manifest = {
        "id": ID, "version": version, "platform": PLATFORM,
        "download_url": f"{base}/{version}/{archive.name}",
        "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
        "size": archive.stat().st_size,
        "entry": ENTRY, "license": "MIT", "source": "https://github.com/xiaozs-com/paldee-pet",
        "minimum_helper_version": "1.2.6",
    }
    catalog = {
        "id": ID, "name": metadata["name"], "category": "交互增强",
        "description": "独立桌面宠物，与远程 Octop 聊天并按需调用本机屏幕自动化小助手CLI。",
        "supported_platforms": ["darwin"], "manifests": {PLATFORM: f"{base}/latest-{PLATFORM}.json"},
        "required_capabilities": [], "acquisition": "free", "retains_user_data_on_uninstall": True,
    }
    write_json(output / f"latest-{PLATFORM}.unsigned.json", manifest)
    write_json(output / f"latest-{PLATFORM}.json", manifest)
    write_json(output / "catalog-entry.json" if args.single_catalog else output / f"catalog-entry-{PLATFORM}.json", catalog)
    write_json(output / "package-validation.json", validate_archive(archive, manifest, catalog))
    (output / "UNSIGNED_MANIFEST.txt").write_text(
        f"latest-{PLATFORM}.json is UNSIGNED. Sign it in the controlled release environment before uploading.\n"
        "No private keys or global catalog.json are read, generated, merged or uploaded by this repository.\n",
        encoding="utf-8",
    )
    return archive


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", default=json.loads((ROOT / "package.json").read_text())["version"])
    parser.add_argument("--pet", type=Path, required=True)
    parser.add_argument("--bridge", type=Path, required=True)
    parser.add_argument("--bridge-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "build/component-release/paldee-pet")
    parser.add_argument("--base-url", default="https://www.xiaozs.com/sah/components/paldee-pet")
    parser.add_argument(
        "--single-catalog", action="store_true",
        help="Write catalog-entry.json (Windows already owns that name) instead of a platform-suffixed entry.",
    )
    print(package(parser.parse_args()))


if __name__ == "__main__":
    main()
