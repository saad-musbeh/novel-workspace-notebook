"""Run the native project export save barrier with a dirty second window."""
import argparse
import json
import os
import shlex
import signal
import sqlite3
import subprocess
import tempfile
import time
from pathlib import Path

from docx_export_integration_test import ROOT, extract_rpm, paragraph, seed_project, synthetic_scene


def run(binary, report_path, timeout=100, rpm=None):
    binary = binary.resolve() if binary else None
    report_path.parent.mkdir(parents=True, exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix="docx-multi-window-", dir=ROOT / "work"))
    output = evidence / "synthetic-export.docx"
    rpm_report = None
    if rpm:
        binary, companion, rpm_report = extract_rpm(rpm, evidence, output)
    else:
        companion = binary.with_name("novel-companion")
    if not binary.is_file() or not companion.is_file():
        raise FileNotFoundError(f"Build the app and companion first: {binary} and {companion}")
    data, cache, config = (evidence / name for name in ("data", "cache", "config"))
    for path in (data, cache, config):
        path.mkdir(mode=0o700)
    store = data / "local.novel.workspace.spike"
    store.mkdir(mode=0o700)
    project, scene = seed_project(companion, store)
    result_base = evidence / "result.json"
    script = ROOT / "tests/docx_multi_window_probe.js"
    flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "webkit2gtk-4.1"], text=True))
    hook = evidence / "multi-window-webkit-hook.so"
    subprocess.run(["cc", "-shared", "-fPIC", str(ROOT / "tests/docx_multi_window_webkit_hook.c"), "-o", str(hook), *flags, "-ldl"], check=True, capture_output=True)
    env = {key: value for key, value in os.environ.items() if not key.startswith("NOVEL_") and key != "LD_PRELOAD"}
    env.update({
        "XDG_DATA_HOME": str(data), "XDG_CACHE_HOME": str(cache), "XDG_CONFIG_HOME": str(config),
        "LD_PRELOAD": str(hook), "NOVEL_RELEASE_TEST_SCRIPT": str(script),
        "NOVEL_RELEASE_TEST_RESULT": str(result_base), "NOVEL_PROBE_DIAGNOSTICS": "1",
    })
    if not rpm:
        env["NOVEL_FILE_PROBE_DIRECTORY"] = str(evidence)
    app = subprocess.Popen([str(binary)], cwd=ROOT, env=env, start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    started = time.monotonic()
    try:
        _, stderr = app.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(app.pid, signal.SIGKILL)
        _, stderr = app.communicate()
        partial = {path.name: path.read_text(errors="replace") for path in evidence.glob("result.json.*")}
        progress = {path.name: path.read_text(errors="replace") for path in evidence.glob("result.json.progress.*")}
        raise AssertionError(f"Two-window DOCX save barrier timed out. Reports: {partial}\nProgress: {progress}\n{stderr.decode(errors='replace')[-4000:]}")
    if app.returncode != 0:
        raise AssertionError(f"Desktop exited {app.returncode}: {stderr.decode(errors='replace')[-2500:]}")
    result_files = [path for path in evidence.glob("result.json.*") if len(path.name.split(".")) == 3 and path.name.rsplit(".", 1)[-1].isdigit()]
    if len(result_files) != 2:
        raise AssertionError(f"Expected exactly two native windows, got {len(result_files)}: {[p.name for p in result_files]}")
    windows = [json.loads(path.read_text()) for path in result_files]
    failures = [item for item in windows if item.get("error")]
    if failures:
        raise AssertionError(f"Multi-window WebKit probe failed: {json.dumps(failures, ensure_ascii=False)}")
    main = next((item for item in windows if item.get("windowLabel") == "main"), None)
    child = next((item for item in windows if item.get("windowLabel") != "main"), None)
    if not main or not child:
        raise AssertionError(f"Expected main and detached window reports: {windows}")
    if main.get("savedRevision") != child.get("savedRevision") or main.get("savedRevision") is None:
        raise AssertionError(f"Project save barrier did not settle both windows on the saved revision: {windows}")
    if not main.get("bundle") or main.get("expectedText") != child.get("expectedText"):
        raise AssertionError(f"Main export report does not match child pending text: {windows}")
    if not output.is_file():
        raise AssertionError("Native Word export did not create the synthetic DOCX destination")
    bundle_path = evidence / "saved-project-bundle.json"
    bundle_path.write_text(json.dumps(main["bundle"], ensure_ascii=False, indent=2) + "\n")
    validation = subprocess.run(["python3", str(ROOT / "tools/check-docx-export.py"), str(output), str(bundle_path)], check=True, capture_output=True, text=True)
    with sqlite3.connect(f"file:{store / 'editor-spike.sqlite3'}?mode=ro", uri=True) as db:
        if db.execute("PRAGMA integrity_check").fetchone() != ("ok",) or db.execute("PRAGMA foreign_key_check").fetchall():
            raise AssertionError("Synthetic multi-window project database failed integrity checks")
        head = db.execute("SELECT head FROM manuscript_scene WHERE id=?", (scene["id"],)).fetchone()
        if not head or head[0] != main["savedRevision"]:
            raise AssertionError(f"Saved head differs from export snapshot: {head}, {main['savedRevision']}")
    report = {
        "mode": "real-two-window-save-barrier", "projectId": project["id"], "sceneId": scene["id"],
        "savedRevision": main["savedRevision"], "output": str(output), "bundle": str(bundle_path),
        "checks": main["checks"] + child["checks"] + [validation.stdout.strip(), "Two native windows were exercised", "SQLite integrity and foreign keys pass", "Saved revision matches the DOCX snapshot"],
        "exportMilliseconds": main["exportMilliseconds"], "elapsedSeconds": time.monotonic() - started,
        "evidenceDirectory": str(evidence), "limitations": ["Synthetic WebKit input and native window transfer do not qualify physical keyboard, IME, or compositor behavior."],
    }
    if rpm_report:
        report["releaseRpm"] = rpm_report
        report["limitations"].append("The RPM was extracted and its original file-dialog helper hash recorded; only that helper in the disposable extracted copy was replaced with a fixed-path synthetic chooser. Native GTK chooser interaction is not covered. All other extracted payload files were verified unchanged.")
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/novel-workspace")
    parser.add_argument("--rpm", type=Path, help="Run the packaged release app using an isolated extracted chooser substitution")
    parser.add_argument("--report", type=Path, default=ROOT / "work/docx-multi-window-integration.json")
    parser.add_argument("--timeout", type=int, default=100)
    args = parser.parse_args()
    if args.rpm and args.binary != ROOT / "target/debug/novel-workspace":
        parser.error("Use either --rpm or a custom --binary, not both")
    run(None if args.rpm else args.binary, args.report.resolve(), args.timeout, args.rpm)
