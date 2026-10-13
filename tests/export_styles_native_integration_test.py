"""Run profile editing, stale-head refusal, reload, and DOCX export in the desktop app.

Only generated synthetic data is used. RPM testing replaces the chooser only in
a disposable extracted copy and checks that the package itself is unchanged.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import signal
import sqlite3
import subprocess
import tempfile
import time

import docx_export_integration_test as base

ROOT = Path(__file__).resolve().parents[1]
PROFILE = ROOT / "tests/fixtures/docx-export-profile.json"
PROBE = ROOT / "tests/export_styles_release_probe.js"


def run(binary, report_path, evidence_root=None, timeout=110, rpm=None):
    binary = binary.resolve() if binary else None
    report_path.parent.mkdir(parents=True, exist_ok=True)
    parent = evidence_root.resolve() if evidence_root else (ROOT / "work").resolve()
    parent.mkdir(parents=True, exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix="export-styles-native-", dir=parent))
    output = evidence / "synthetic-export.docx"
    rpm_report = None
    if rpm:
        binary, companion, rpm_report = base.extract_rpm(rpm, evidence, output)
    else:
        companion = binary.with_name("novel-companion")
    if not binary or not binary.is_file() or not companion.is_file():
        raise FileNotFoundError(f"Build the app and companion first: {binary} and {companion}")

    data, cache, config = (evidence / name for name in ("data", "cache", "config"))
    for path in (data, cache, config):
        path.mkdir(mode=0o700)
    store = data / "local.novel.workspace.spike"
    store.mkdir(mode=0o700)
    project, _first = base.seed_project(companion, store)

    flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "webkit2gtk-4.1"], text=True))
    library = evidence / "release-webkit-hook.so"
    subprocess.run(["cc", "-shared", "-fPIC", str(ROOT / "tests/release_webkit_hook.c"), "-o", str(library), *flags, "-ldl"], check=True, capture_output=True)
    result_path = evidence / "ui-result.json"
    screenshot = evidence / "export-styles-ui.png"
    env = {key: value for key, value in os.environ.items() if not key.startswith("NOVEL_") and key != "LD_PRELOAD"}
    env.update({
        "XDG_DATA_HOME": str(data), "XDG_CACHE_HOME": str(cache), "XDG_CONFIG_HOME": str(config),
        "LD_PRELOAD": str(library), "NOVEL_RELEASE_TEST_SCRIPT": str(PROBE),
        "NOVEL_RELEASE_TEST_RESULT": str(result_path), "NOVEL_RELEASE_TEST_SCREENSHOT": str(screenshot),
    })
    if not rpm:
        env["NOVEL_FILE_PROBE_DIRECTORY"] = str(evidence)
    started = time.monotonic()
    app = subprocess.Popen([str(binary)], cwd=ROOT, env=env, start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        _stdout, stderr = app.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(app.pid, signal.SIGKILL)
        _stdout, stderr = app.communicate()
        details = result_path.read_text(errors="replace") if result_path.exists() else "No external UI report was returned."
        raise AssertionError(f"Synthetic style workflow timed out at {evidence}.\n{details}\n{stderr.decode(errors='replace')[-3000:]}")
    if app.returncode != 0:
        raise AssertionError(f"Desktop app exited {app.returncode}: {stderr.decode(errors='replace')[-3000:]}")
    if not result_path.is_file():
        raise AssertionError(f"External UI hook produced no result; evidence: {evidence}\n{stderr.decode(errors='replace')[-3000:]}")
    ui = json.loads(result_path.read_text(encoding="utf-8"))
    if "error" in ui:
        raise AssertionError("Export profile UI probe failed: " + json.dumps(ui, ensure_ascii=False))
    if not output.is_file():
        raise AssertionError(f"Native export did not create {output}")
    if not ui.get("staleRefused") or not ui.get("reloadedProfileRevisionId") or not ui.get("profileRevisionId"):
        raise AssertionError(f"UI did not exercise stale refusal, reload, and final profile save: {ui}")
    if ui.get("profile") != json.loads(PROFILE.read_text(encoding="utf-8")):
        raise AssertionError("UI's final saved profile differs from the independent profile fixture")
    if ui.get("outputName") != "synthetic-export.docx":
        raise AssertionError("UI reported a different synthetic output name")

    bundle_path = evidence / "saved-project-bundle.json"
    bundle_path.write_text(json.dumps(ui["bundle"], ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    profile_path = evidence / "saved-export-profile.json"
    profile_path.write_text(json.dumps(ui["profile"], ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    checker = subprocess.run(["python3", str(ROOT / "tools/check-docx-export.py"), str(output), str(bundle_path), "--profile", str(profile_path)], capture_output=True, text=True)
    if checker.returncode:
        raise AssertionError("Independent DOCX/profile verification failed:\n" + checker.stdout + checker.stderr)

    with sqlite3.connect(f"file:{store / 'editor-spike.sqlite3'}?mode=ro", uri=True) as database:
        if database.execute("PRAGMA integrity_check").fetchone() != ("ok",):
            raise AssertionError("Synthetic project database failed integrity_check")
        if database.execute("PRAGMA foreign_key_check").fetchall():
            raise AssertionError("Synthetic project database failed foreign_key_check")
        profile_row = database.execute("SELECT head FROM project_data WHERE project_id=? AND kind='exportProfile'", (project["id"],)).fetchone()
        if not profile_row or profile_row[0] != ui["profileRevisionId"]:
            raise AssertionError(f"UI profile revision is not the persisted exportProfile head: {profile_row}, {ui['profileRevisionId']}")
        saved_scene_heads = database.execute("SELECT s.id,s.head FROM manuscript_scene s JOIN chapter c ON c.id=s.chapter_id WHERE c.project_id=? ORDER BY s.id", (project["id"],)).fetchall()
        if sorted(saved_scene_heads) != sorted(tuple(item) for item in ui["sceneHeadsBefore"]):
            raise AssertionError(f"Saving styles/export changed manuscript scene revisions: database={saved_scene_heads}, before={ui['sceneHeadsBefore']}")
        if not database.execute("SELECT 1 FROM revision_envelope WHERE id=?", (ui["profileRevisionId"],)).fetchone():
            raise AssertionError("Final profile revision is missing from immutable revision history")
        if not database.execute("SELECT 1 FROM revision_envelope WHERE id=?", (ui["reloadedProfileRevisionId"],)).fetchone():
            raise AssertionError("Competing profile revision used for reload was not retained in history")
    if not screenshot.is_file():
        raise AssertionError(f"WebKit screenshot missing; evidence: {evidence}")

    report = {
        "projectId": project["id"], "output": str(output), "bundle": str(bundle_path),
        "profile": str(profile_path), "profileRevisionId": ui["profileRevisionId"],
        "reloadedProfileRevisionId": ui["reloadedProfileRevisionId"], "staleError": ui["staleError"],
        "screenshot": str(screenshot), "evidenceDirectory": str(evidence),
        "checks": ui["checks"] + [checker.stdout.strip(), "Final profile revision matches the persisted head and exported DOCX style profile", "Backend refused the stale head before reload; current profile exported successfully afterward", "Scene revision heads did not change during style saves/export", "SQLite integrity and foreign keys pass"],
        "elapsedSeconds": time.monotonic() - started,
        "limitations": ["Synthetic WebKit input exercises application UI and native IPC, not physical keyboard/IME behavior.", "LibreOffice may be used separately for interoperability; this harness does not claim Microsoft Word validation."],
    }
    if rpm_report:
        report["releaseRpm"] = rpm_report
        report["limitations"].append("The RPM was run from a disposable extraction; only its file-dialog helper was replaced with a fixed synthetic chooser. Native GTK chooser interaction is not covered.")
    else:
        report["limitations"].append("The debug-only chooser override wrote to an isolated synthetic evidence directory; native GTK chooser interaction is not covered.")
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/novel-workspace")
    parser.add_argument("--rpm", type=Path)
    parser.add_argument("--report", type=Path, default=ROOT / "work/export-styles-native-integration.json")
    parser.add_argument("--evidence-root", type=Path)
    parser.add_argument("--timeout", type=int, default=110)
    args = parser.parse_args()
    if args.rpm and args.binary != ROOT / "target/debug/novel-workspace":
        parser.error("Use either --rpm or a custom --binary, not both")
    run(None if args.rpm else args.binary, args.report.resolve(), args.evidence_root, args.timeout, args.rpm)
