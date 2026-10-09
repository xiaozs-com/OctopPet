"""Export only the component implementation layer, leaving eye/other user edits out."""
import argparse
import difflib
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TRACKED = ["src/lib/pdChatBridge.ts", "src/lib/pdChatBridge.test.ts", "src/lib/tauriApi.ts", ".gitignore", "package.json", "package-lock.json",
           "src-tauri/tauri.conf.json", "src-tauri/src/tray.rs",
           "src/main.tsx", "src/windows/SettingsWindow.tsx", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock",
           "src-tauri/src/lib.rs", "src-tauri/src/config_cmd.rs",
           "src-tauri/src/secrets_cmd.rs", "src-tauri/src/pd_bridge_cmd.rs"]
ADDED = ["src/lib/brand.ts", "docs/upstream-sync/PALDEE_SYNC.md",
         "docs/PALDEE_PET_COMPONENT_ACCEPTANCE.md", "docs/PALDEE_PET_STARTUP_ACCEPTANCE.md", "docs/PALDEE_PET_FULL_CLI.md", "src-tauri/src/component_runtime.rs", "scripts/build_pd_component.ps1",
         "scripts/package_pd_component.py", "scripts/test_pd_component.py",
         "scripts/export_pd_component_patch.py", "docs/OCTOPPET_COMPONENT.md", "docs/OCTOPPET_COMPONENT_ACCEPTANCE.md"]

def git(*args, data=None):
    return subprocess.check_output(["git", "-c", "core.safecrlf=false", *args],
                                   cwd=ROOT, input=data, stderr=subprocess.DEVNULL)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="53b675d")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/upstream-sync/snapshots")
    args = parser.parse_args()
    base = git("rev-parse", args.base).decode().strip()
    files = TRACKED + ADDED + sorted(p.relative_to(ROOT).as_posix()
                                    for p in (ROOT / "packaging/third-party-licenses").rglob("*") if p.is_file())
    tracked = [p for p in files if git("ls-files", "--", p).strip()]
    patch = git("diff", "--binary", "--full-index", base, "--", *tracked).decode("utf-8")
    for name in [p for p in files if p not in tracked]:
        content = (ROOT / name).read_text(encoding="utf-8").replace("\r\n", "\n")
        blob = git("hash-object", "--stdin", data=content.encode()).decode().strip()
        patch += f"diff --git a/{name} b/{name}\nnew file mode 100644\nindex {'0'*40}..{blob}\n"
        patch += "".join(difflib.unified_diff([], content.splitlines(True), fromfile="/dev/null", tofile="b/" + name))
    args.output.mkdir(parents=True, exist_ok=True)
    target = args.output / "octoppet-component.patch"
    target.write_text(patch, encoding="utf-8", newline="\n")
    manifest = {"baseline": base, "head": git("rev-parse", "HEAD").decode().strip(),
                "patchSha256": hashlib.sha256(target.read_bytes()).hexdigest(),
                "files": {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in files},
                "scope": "Component and Paldee branding source layer; unrelated eye changes remain separate"}
    (args.output / "octoppet-component.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print("Exported component source patch; index and branch unchanged")

if __name__ == "__main__":
    main()
