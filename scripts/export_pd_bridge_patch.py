"""Export the current PD bridge diff, including added files, without staging."""
import argparse
import difflib
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TRACKED = ["CHANGELOG.md", "src-tauri/src/lib.rs", "src/hooks/useChatController.ts",
           "src/lib/tauriApi.ts", "src/windows/ChatWindow.test.tsx"]
ADDED = ["src-tauri/src/pd_bridge_cmd.rs", "src/lib/pdChatBridge.ts",
         "src/lib/pdChatBridge.test.ts"]

def git(*args, data=None):
    return subprocess.check_output(["git", "-c", "core.safecrlf=false", *args],
                                   cwd=ROOT, input=data, stderr=subprocess.DEVNULL)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT/"docs/upstream-sync/snapshots")
    parser.add_argument("--base", default="385d26138871a32f3d821e6128672d47cf94ba0f",
                        help="桥接前的基线提交；同步上游后应显式指定新基线")
    parser.add_argument("--mascot", action="store_true", help="导出默认眨眼修正，而非桥接")
    parser.add_argument("--staged", action="store_true", help="Export the reviewed index rather than concurrent worktree edits")
    args = parser.parse_args()
    tracked_files = ["src/components/MascotImage.tsx", "public/mascots/taiji-bot/README.md"] if args.mascot else TRACKED
    added_files = ["src/styles/mascot.css", "src/components/MascotImage.test.tsx"] if args.mascot else ADDED
    name_prefix = "taiji-idle" if args.mascot else "octop-pet-bridge"
    baseline = git("rev-parse", args.base).decode().strip()
    tracked = [p for p in tracked_files + added_files if git("ls-files", "--", p).strip()]
    patch = git("diff", *(["--cached"] if args.staged else []), "--binary", "--full-index", baseline, "--", *tracked).decode("utf-8")
    added = [p for p in added_files if p not in tracked]
    for name in added:
        content = (ROOT/name).read_text(encoding="utf-8").replace("\r\n", "\n")
        blob = git("hash-object", "--stdin", data=content.encode()).decode().strip()
        patch += f"diff --git a/{name} b/{name}\nnew file mode 100644\nindex {'0'*40}..{blob}\n"
        patch += "".join(difflib.unified_diff([], content.splitlines(True),
                                            fromfile="/dev/null", tofile="b/"+name))
    args.output.mkdir(parents=True, exist_ok=True)
    target = args.output/(name_prefix+".patch")
    target.write_text(patch, encoding="utf-8", newline="\n")
    manifest = {
        "baselineCommit": baseline,
        "workingHead": git("rev-parse", "HEAD").decode().strip(),
        "purpose": "Taiji idle eye-only UI animation" if args.mascot else "PD bridge; Taiji visual source changes are in separate snapshots",
        "patchSha256": hashlib.sha256(target.read_bytes()).hexdigest(),
        "snapshotSource": "index" if args.staged else "worktree",
        "files": {p: hashlib.sha256(git("show", ":"+p) if args.staged else (ROOT/p).read_bytes()).hexdigest()
                  for p in tracked_files+added_files}
    }
    (args.output/(name_prefix+".json")).write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    print("Exported bridge patch and manifest; Git index and branch unchanged.")

if __name__ == "__main__":
    main()
