"""Exercise the real debug desktop Word export using only a synthetic project.

The WebKit probe and file destination are supplied externally through the
test-only hook and debug-only chooser override. All writing data lives under a
unique work/ directory, never the user's normal XDG folders.
"""
import argparse
import hashlib
import json
import os
import shlex
import shutil
import signal
import sqlite3
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def synthetic_scene(title, blocks):
    return {"schemaVersion": 1, "title": title, "document": {"type": "doc", "content": blocks}}


def paragraph(text, lang="en", direction="ltr", marks=None):
    node = {"type": "text", "text": text}
    if marks:
        node["marks"] = [{"type": mark} for mark in marks]
    return {"type": "paragraph", "attrs": {"lang": lang, "dir": direction}, "content": [node]}


def seed_project(companion, store):
    process = subprocess.Popen([str(companion), str(store)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    def rpc(operation, args):
        process.stdin.write(json.dumps({"id": 1, "op": operation, "args": args}, ensure_ascii=False) + "\n")
        process.stdin.flush()
        response = json.loads(process.stdout.readline())
        if not response.get("ok"):
            raise AssertionError(f"Synthetic fixture RPC {operation} failed: {response}")
        return response["value"]

    try:
        project = rpc("create_project", {"title": "Synthetic Word Export Integration"})
        chapter = rpc("create_chapter", {"projectId": project["id"], "title": "Synthetic chapter"})
        first = rpc("create_scene", {"chapterId": chapter["id"], "scene": synthetic_scene("Arabic 中文 scene", [
            paragraph("سلام مِمِ Chinese 中文 繁體 👩🏽‍🚀", "ar", "rtl", ["bold"]),
            paragraph("A second synthetic paragraph.", "en", "ltr"),
        ])})
        rpc("create_scene", {"chapterId": chapter["id"], "scene": synthetic_scene("Second scene", [
            paragraph("Synthetic archive history remains available.", "en", "ltr"),
        ])})
        return project, first
    finally:
        process.stdin.close()
        if process.wait(timeout=10) != 0:
            raise AssertionError("Synthetic companion failed while seeding: " + process.stderr.read()[-2000:])
        process.stdout.close()
        process.stderr.close()


def extract_rpm(rpm, evidence, output):
    extraction = evidence / "rpm-extracted"
    extraction.mkdir()
    producer = subprocess.Popen(["rpm2cpio", str(rpm.resolve())], stdout=subprocess.PIPE)
    unpack = subprocess.run(["cpio", "-idmu", "--quiet"], stdin=producer.stdout, cwd=extraction, capture_output=True)
    producer.stdout.close()
    if producer.wait() != 0 or unpack.returncode != 0:
        raise RuntimeError("Could not extract RPM payload: " + unpack.stderr.decode(errors="replace")[-1500:])
    original = {str(path.relative_to(extraction)): hashlib.sha256(path.read_bytes()).hexdigest() for path in extraction.rglob("*") if path.is_file()}
    binary = extraction / "usr/bin/novel-workspace"
    companion = extraction / "usr/bin/novel-companion"
    helper = extraction / "usr/lib/Novel Workspace/runtime/file_dialog.py"
    if not helper.is_file():
        raise FileNotFoundError(f"Packaged local chooser helper is missing: {helper}")
    chooser_source = """#!/usr/bin/python3
import json,sys
request=json.loads(sys.stdin.buffer.readline(4097))
if request.get('mode')!='save' or request.get('format')!='docx':
 print(json.dumps({'ok':False,'error':'Only the synthetic DOCX save is allowed.'})); raise SystemExit(1)
print(json.dumps({'ok':True,'cancelled':False,'path':PATH}))
""".replace("PATH", repr(str(output.resolve())))
    helper.write_text(chooser_source)
    changed = [str(path.relative_to(extraction)) for path in extraction.rglob("*") if path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() != original.get(str(path.relative_to(extraction)))]
    if changed != ["usr/lib/Novel Workspace/runtime/file_dialog.py"]:
        raise AssertionError(f"RPM integration changed unexpected extracted payload: {changed}")
    return binary, companion, {"rpm": str(rpm.resolve()), "rpmSha256": hashlib.sha256(rpm.read_bytes()).hexdigest(), "originalChooserSha256": original[str(helper.relative_to(extraction))], "changedExtractedPayload": changed}


def run(binary, report_path, evidence_root=None, timeout=100, rpm=None):
    binary = binary.resolve() if binary else None
    report_path.parent.mkdir(parents=True, exist_ok=True)
    parent = evidence_root.resolve() if evidence_root else (ROOT / "work").resolve()
    parent.mkdir(parents=True, exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix="docx-export-", dir=parent))
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
    project, first = seed_project(companion, store)

    flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "webkit2gtk-4.1"], text=True))
    library = evidence / "release-webkit-hook.so"
    subprocess.run(["cc", "-shared", "-fPIC", str(ROOT / "tests/release_webkit_hook.c"), "-o", str(library), *flags, "-ldl"], check=True, capture_output=True)

    result_base = evidence / "ui-result.json"
    probe = ROOT / "tests/docx_release_probe.js"
    bundle_path = evidence / "saved-project-bundle.json"
    screenshot = evidence / "export-ui.png"
    env = {key: value for key, value in os.environ.items() if not key.startswith("NOVEL_") and key != "LD_PRELOAD"}
    env.update({
        "XDG_DATA_HOME": str(data), "XDG_CACHE_HOME": str(cache), "XDG_CONFIG_HOME": str(config),
        "LD_PRELOAD": str(library), "NOVEL_RELEASE_TEST_SCRIPT": str(probe),
        "NOVEL_RELEASE_TEST_RESULT": str(result_base), "NOVEL_RELEASE_TEST_SCREENSHOT": str(screenshot),
    })
    if not rpm:
        env["NOVEL_FILE_PROBE_DIRECTORY"] = str(evidence)
    started = time.monotonic()
    app = subprocess.Popen([str(binary)], cwd=ROOT, env=env, start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        stdout, stderr = app.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(app.pid, signal.SIGKILL)
        stdout, stderr = app.communicate()
        partial = result_base
        details = partial.read_text(errors="replace") if partial.exists() else "No external UI report was returned."
        raise AssertionError(f"Synthetic DOCX desktop workflow timed out at {evidence}.\n{details}\n{stderr.decode(errors='replace')[-3000:]}")
    if app.returncode != 0:
        raise AssertionError(f"Desktop app exited {app.returncode}: {stderr.decode(errors='replace')[-3000:]}")
    ui_result = result_base
    if not ui_result.is_file():
        raise AssertionError(f"External UI hook produced no result; evidence: {evidence}\n{stderr.decode(errors='replace')[-3000:]}")
    ui = json.loads(ui_result.read_text())
    if "error" in ui:
        raise AssertionError(f"DOCX UI probe failed: {json.dumps(ui, ensure_ascii=False)}")
    if not output.is_file():
        raise AssertionError(f"Native export reported success but did not create {output}")
    if not ui.get("savedRevision") or not ui.get("bundle"):
        raise AssertionError(f"UI did not return the saved project snapshot: {ui}")
    bundle_path.write_text(json.dumps(ui["bundle"], ensure_ascii=False, indent=2) + "\n")
    validation = subprocess.run(["python3", str(ROOT / "tools/check-docx-export.py"), str(output), str(bundle_path)], check=True, capture_output=True, text=True)

    with sqlite3.connect(f"file:{store / 'editor-spike.sqlite3'}?mode=ro", uri=True) as database:
        if database.execute("PRAGMA integrity_check").fetchone() != ("ok",):
            raise AssertionError("Synthetic project database failed integrity_check")
        if database.execute("PRAGMA foreign_key_check").fetchall():
            raise AssertionError("Synthetic project database failed foreign_key_check")
        head = database.execute("SELECT head FROM manuscript_scene WHERE id=?", (first["id"],)).fetchone()
        if not head or head[0] != ui["savedRevision"]:
            raise AssertionError(f"Exported snapshot revision is not the persisted scene head: {head}, {ui['savedRevision']}")
        if not database.execute("SELECT 1 FROM revision_envelope WHERE id=?", (ui["beforeRevision"],)).fetchone():
            raise AssertionError("Pre-export manuscript history revision was lost")
    if not screenshot.is_file():
        raise AssertionError(f"WebKit screenshot missing; evidence: {evidence}")

    report = {
        "projectId": project["id"], "sceneId": first["id"], "savedRevision": ui["savedRevision"],
        "output": str(output), "bundle": str(bundle_path), "screenshot": str(screenshot),
        "checks": ui["checks"] + [validation.stdout.strip(), "SQLite integrity and foreign keys pass", "Saved revision matches the persisted project head", "Pre-export history remains in the project"],
        "exportMilliseconds": ui["exportMilliseconds"], "archiveRevisionCountBefore": ui["archiveRevisionCountBefore"],
        "archiveRevisionCountAfter": ui["archiveRevisionCountAfter"], "elapsedSeconds": time.monotonic() - started,
        "evidenceDirectory": str(evidence), "limitations": ["Synthetic WebKit input exercises the real application and native IPC, not physical keyboard or IME behavior."],
    }
    if rpm_report:
        report["releaseRpm"] = rpm_report
        report["limitations"].append("The RPM was extracted and its original file-dialog helper hash recorded; only that helper in the disposable extracted copy was replaced with a fixed-path synthetic chooser. Native GTK chooser interaction is not covered. All other extracted payload files were verified unchanged.")
    else:
        report["limitations"].append("The debug-only chooser destination override wrote to the isolated synthetic evidence directory; native GTK chooser interaction is not covered.")
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/novel-workspace")
    parser.add_argument("--rpm", type=Path, help="Also run the packaged release app using an isolated extracted chooser substitution")
    parser.add_argument("--report", type=Path, default=ROOT / "work/docx-export-integration.json")
    parser.add_argument("--evidence-root", type=Path, help="Directory under which persistent synthetic evidence is created")
    parser.add_argument("--timeout", type=int, default=100)
    args = parser.parse_args()
    if args.rpm and args.binary != ROOT / "target/debug/novel-workspace":
        parser.error("Use either --rpm or a custom --binary, not both")
    run(None if args.rpm else args.binary, args.report.resolve(), args.evidence_root, args.timeout, args.rpm)
