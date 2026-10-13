#!/usr/bin/env python3
"""Isolated synthetic desktop-scale benchmark. Uses only the stdio companion and installed JS tools.
No personal database, network service, dependency download, or model is used.
Run: python3 tests/desktop_scale_test.py --binary target/debug/novel-companion --report work/desktop-scale-results.json
"""
from __future__ import annotations
import argparse, hashlib, json, math, os, platform, re, statistics, subprocess, sys, tempfile, time, uuid
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

def percentile(values, fraction):
    return sorted(values)[max(0, math.ceil(len(values) * fraction) - 1)]

def distribution(values):
    return {"count": len(values), "median_ms": round(statistics.median(values), 3), "p95_ms": round(percentile(values, .95), 3), "max_ms": round(max(values), 3)}

class Companion:
    def __init__(self, binary, directory):
        self.stderr = open(directory / "companion-stderr.log", "w", encoding="utf-8")
        self.process = subprocess.Popen([str(binary), str(directory / "data")], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr, text=True, encoding="utf-8", bufsize=1)
        self.sequence = 0
    def rpc(self, operation, **args):
        self.sequence += 1
        request = {"id": self.sequence, "op": operation, "args": args}
        started = time.perf_counter()
        self.process.stdin.write(json.dumps(request, ensure_ascii=False, separators=(",", ":")) + "\n")
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError("Companion exited before acknowledging " + operation)
        reply = json.loads(line)
        assert reply["id"] == self.sequence, "Unexpected response identity"
        if not reply["ok"]:
            raise RuntimeError(operation + ": " + str(reply.get("error", "Unknown error")))
        return reply["value"], (time.perf_counter() - started) * 1000
    def memory(self):
        try:
            text = Path(f"/proc/{self.process.pid}/status").read_text()
            return {key: int(re.search(r"^" + key + r":\s+(\d+)", text, re.M).group(1)) for key in ["VmRSS", "VmHWM"]}
        except (OSError, AttributeError):
            return {}
    def stop(self):
        if self.process.stdin:
            self.process.stdin.close()
        try:
            self.process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
            raise RuntimeError("Companion did not finish its local writes during shutdown")
        self.stderr.close()
        assert self.process.returncode == 0, "Companion shutdown failed"


def identifier(kind, index):
    return str(uuid.uuid5(uuid.NAMESPACE_URL, f"novel-synthetic-scale/{kind}/{index}"))


def planning_document(scenes):
    lane = identifier("lane", 0)
    events = [{"id": identifier("event", i), "title": f"Synthetic event {i:04}", "notes": "Synthetic chronology only.", "laneIds": [lane], "sceneIds": [scenes[i % len(scenes)]["id"]], "time": {"kind": "fixed", "day": i, "label": f"Day {i}"}} for i in range(2000)]
    return {"schemaVersion": 1, "fields": [], "records": [{"id": identifier("record", i), "kind": "character", "name": f"Synthetic character {i:03}", "aliases": [], "notes": "Synthetic test data.", "values": {}, "links": []} for i in range(500)], "facts": [], "plotPoints": [], "lanes": [{"id": lane, "name": "Synthetic main plot", "kind": "main"}], "events": events, "constraints": [], "issues": [], "boards": [{"id": identifier("board", 0), "name": "Synthetic scale board", "groups": [], "edges": [], "cards": [{"id": identifier("card", i), "title": f"Synthetic card {i:04}", "body": "Synthetic board text.", "kind": "event", "reference": {"kind": "event", "id": events[i]["id"]}, "x": (i % 10) * 280, "y": (i // 10) * 200, "groupId": None, "labels": ["synthetic"]} for i in range(1000)]}]}


def frontend_measurements(directory, document, bundle):
    entry = directory / "frontend-scale.tsx"
    fixture = directory / "frontend-fixture.json"
    fixture.write_text(json.dumps({"document": document, "scenes": [{"id": scene["id"], "title": scene["scene"]["title"], "text": scene["scene"]["document"]["content"][0]["content"][0]["text"]} for scene in bundle["scenes"]]}, ensure_ascii=False), encoding="utf-8")
    board_path = json.dumps(str(REPO / "apps/writer/src/planning/Board.tsx"))
    entry.write_text("""import React from 'react';
import {renderToStaticMarkup} from 'react-dom/server';
import {readFileSync} from 'node:fs';
import {performance} from 'node:perf_hooks';
import {webcrypto} from 'node:crypto';
import {Board} from """ + board_path + """;
const fixture=JSON.parse(readFileSync(process.argv[2],'utf8'));
const measurements=[];let html='';
for(let i=0;i<10;i++){const start=performance.now();html=renderToStaticMarkup(<Board document={fixture.document} scenes={fixture.scenes} change={()=>{}}/>);measurements.push(performance.now()-start);}
const start=performance.now();await Promise.all(fixture.scenes.map(scene=>webcrypto.subtle.digest('SHA-256',new TextEncoder().encode(scene.text))));const hashesMs=performance.now()-start;
process.stdout.write(JSON.stringify({board_ssr_ms:measurements,board_rendered_cards:(html.match(/class=\"planning-board-card/g)||[]).length,board_total_cards:fixture.document.boards[0].cards.length,board_html_bytes:Buffer.byteLength(html),source_hash_ms:hashesMs,node_peak_rss_kib:process.resourceUsage().maxRSS}));
""", encoding="utf-8")
    out = directory / "frontend-bundle"
    build = "import {build} from 'vite';await build({configFile:false,logLevel:'silent',build:{ssr:" + json.dumps(str(entry)) + ",outDir:" + json.dumps(str(out)) + ",emptyOutDir:true,rollupOptions:{output:{entryFileNames:'scale.mjs'}}}});"
    subprocess.run(["node", "--input-type=module", "-e", build], cwd=REPO, check=True, capture_output=True, text=True, timeout=60)
    result = subprocess.run(["node", str(out / "scale.mjs"), str(fixture)], cwd=REPO, check=True, capture_output=True, text=True, timeout=60)
    data = json.loads(result.stdout)
    data["board_ssr"] = distribution(data.pop("board_ssr_ms"))
    return data


def run(binary, report_path, language="zh-Hans"):
    binary = binary.resolve()
    if not binary.is_file():
        raise RuntimeError("Build the installed offline companion before running this benchmark")
    scratch = REPO / "work"
    scratch.mkdir(exist_ok=True)
    result = {"schemaVersion": 1, "measuredAt": datetime.now(timezone.utc).isoformat(), "host": {"os": platform.system(), "release": platform.release(), "architecture": platform.machine(), "logical_cpus": os.cpu_count()}, "binary": {"profile": "release" if "/release/" in str(binary) else "debug", "sha256": hashlib.sha256(binary.read_bytes()).hexdigest()}, "fixture": {"scenes": 1000, "records": 500, "events": 2000, "board_cards": 1000, "language": language, "chinese_characters": 750000 if language=="zh-Hans" else 0, "english_whitespace_words":250000 if language=="en" else 0}, "timings": {}, "checks": {}, "limitations": ["Synthetic stdio/storage measurements; no desktop UI frame rate, keystroke latency, actual IME, font rendering, disk-full fault, sync-over-mobile, or real model inference is measured.", "Reopening uses a warm OS filesystem cache; it is not a cold boot benchmark.", "Frontend render is React SSR with a default viewport, not an interactive browser benchmark."]}
    with tempfile.TemporaryDirectory(prefix="desktop-scale-", dir=scratch) as name:
        directory = Path(name)
        companion = Companion(binary, directory)
        try:
            project, startup = companion.rpc("create_project", title="Synthetic desktop scale fixture")
            chapter, _ = companion.rpc("create_chapter", projectId=project["id"], title="Synthetic chapter")
            result["timings"]["startup_and_create_project_ms"] = round(startup, 3)
            prefix = "规模样本" if language=="zh-Hans" else "scale"
            if language=="zh-Hans":
                pattern = "故事从这里开始每一个字通向新的世界"
                text = prefix + (pattern * 50)[:750 - len(prefix)]
                assert len(text)==750
            else:
                text = "scale " + "synthetic " * 248 + "ending"
                assert len(text.split())==250
            expected_characters=len(text)*1000
            scenes, create_times = [], []
            for i in range(1000):
                scene = {"schemaVersion": 1, "title": f"Synthetic scene {i:04}", "document": {"type": "doc", "content": [{"type": "paragraph", "attrs": {"id": identifier("block", i), "dir": "auto", "lang": language}, "content": [{"type": "text", "text": text}]}]}}
                saved, elapsed = companion.rpc("create_scene", chapterId=chapter["id"], scene=scene)
                scenes.append(saved); create_times.append(elapsed)
            result["timings"]["create_scene"] = distribution(create_times)
            doc = planning_document(scenes)
            planning, elapsed = companion.rpc("save_project_data", projectId=project["id"], kind="planning", payload=doc, parent=None)
            result["timings"]["save_planning_ms"] = round(elapsed, 3)
            first = scenes[0]
            save_times = []
            parent = first["revisionId"]
            for i in range(10):
                edited = json.loads(json.dumps(first["scene"], ensure_ascii=False))
                edited["document"]["content"][0]["content"][0]["text"] = (("更新样本" if language=="zh-Hans" else "renew") if i % 2 == 0 else prefix) + text[len(prefix):]
                snapshot, elapsed = companion.rpc("save_scene", sceneId=first["id"], scene=edited, parent=parent, requestId=str(uuid.uuid4()))
                parent = snapshot["revisionId"]; save_times.append(elapsed)
            result["timings"]["save_scene"] = distribution(save_times)
            opened, elapsed = companion.rpc("open_project", projectId=project["id"])
            result["timings"]["open_project_ms"] = round(elapsed, 3)
            assert len(opened["scenes"]) == 1000
            hits, elapsed = companion.rpc("search", projectId=project["id"], query=prefix)
            result["timings"]["search_exact_ms"] = round(elapsed, 3)
            assert len(hits) == 1000
            stats, elapsed = companion.rpc("statistics", projectId=project["id"])
            result["timings"]["statistics_ms"] = round(elapsed, 3)
            assert stats["cjkCharacters"] == (750000 if language=="zh-Hans" else 0)
            if language=="en":assert stats["whitespaceWords"]==250000
            archive, elapsed = companion.rpc("export_archive", projectId=project["id"])
            result["timings"]["export_archive_ms"] = round(elapsed, 3)
            result["archive_bytes"] = len(archive.encode("utf-8"))
            imported, elapsed = companion.rpc("import_archive", archive=archive)
            result["timings"]["import_archive_ms"] = round(elapsed, 3)
            assert imported["project"]["id"] != project["id"]
            assert len(imported["scenes"]) == 1000
            imported_data, elapsed = companion.rpc("load_project_data", projectId=imported["project"]["id"], kind="planning")
            result["timings"]["load_imported_planning_ms"] = round(elapsed, 3)
            for field, count in [("records", 500), ("events", 2000)]:
                assert len(imported_data["payload"][field]) == count
            assert len(imported_data["payload"]["boards"][0]["cards"]) == 1000
            assert sum(len(scene["scene"]["document"]["content"][0]["content"][0]["text"]) for scene in imported["scenes"]) == expected_characters
            def manuscript(bundle):
                return {scene["scene"]["title"]: scene["scene"]["document"] for scene in bundle["scenes"]}
            assert manuscript(imported) == manuscript(opened), "Archive changed rich manuscript documents"
            assert imported_data["payload"]["records"] == doc["records"], "Archive changed story records"
            result["checks"] = {"scene_count": True, "exact_language_statistics": True, "search_hit_count": True, "separate_imported_project": True, "planning_counts_after_import": True, "text_count_after_import": True, "exact_rich_manuscript_after_import": True, "exact_story_records_after_import": True}
            result["companion_memory_kib"] = companion.memory()
            result["storage_bytes_before_close"] = {path.name: path.stat().st_size for path in (directory / "data").iterdir() if path.is_file()}
            try:
                result["frontend"] = frontend_measurements(directory, doc, opened)
            except (subprocess.SubprocessError, OSError, ValueError) as error:
                result["frontend_unavailable"] = str(error)[:1500]
            companion.stop()
            reopened = Companion(binary, directory)
            try:
                recovered, elapsed = reopened.rpc("open_project", projectId=project["id"])
                result["timings"]["process_reopen_project_ms"] = round(elapsed, 3)
                assert len(recovered["scenes"]) == 1000
                assert next(scene for scene in recovered["scenes"] if scene["id"] == first["id"])["revisionId"] == parent
                result["checks"]["last_acknowledged_revision_after_reopen"] = True
                result["reopened_companion_memory_kib"] = reopened.memory()
            finally:
                reopened.stop()
            result["storage_bytes_after_close"] = {path.name: path.stat().st_size for path in (directory / "data").iterdir() if path.is_file()}
        finally:
            if companion.process.poll() is None:
                companion.stop()
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return result

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=REPO / "target/debug/novel-companion")
    parser.add_argument("--report", type=Path, default=REPO / "work/desktop-scale-results.json")
    parser.add_argument("--language",choices=["zh-Hans","en"],default="zh-Hans")
    options = parser.parse_args()
    run(options.binary, options.report,options.language)
