#!/usr/bin/env python3
"""Validate WordCraft DOCX output against a ProjectBundle.

Usage: python3 tools/check-docx-export.py export.docx bundle.json [--libreoffice]
       python3 tools/check-docx-export.py export.docx bundle.json --libreoffice --artifacts DIRECTORY
LibreOffice uses a private temporary profile. QA files are removed by default;
--artifacts preserves them in a unique private subfolder under DIRECTORY.
"""
import argparse
import json
import os
import subprocess
import sys
import tempfile
import unicodedata
import zipfile
import xml.etree.ElementTree as ET
from pathlib import Path

W = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
NS = {"w": W}
Q = lambda name: f"{{{W}}}{name}"


def fail(message):
    raise SystemExit(f"DOCX check failed: {message}")


def visible_text(node):
    out = []
    for child in node.iter():
        if child.tag == Q("t"):
            out.append(child.text or "")
        elif child.tag == Q("tab"):
            out.append("\t")
        elif child.tag == Q("cr"):
            out.append("\n")
        elif child.tag == Q("br") and child.get(Q("type")) != "page":
            out.append("\n")
    return "".join(out)


def toggle_is_on(element):
    if element is None:
        return False
    value = (element.get(Q("val")) or "1").lower()
    return value not in ("0", "false", "off", "no")


def source_text(block):
    out = []
    for child in block.get("content", []):
        if child.get("type") == "text":
            out.append(child.get("text", ""))
        elif child.get("type") == "hardBreak":
            out.append("\n")
    return "".join(out)


def rtl_for(block):
    attrs = block.get("attrs", {})
    if attrs.get("dir") in ("rtl", "ltr"):
        return attrs["dir"] == "rtl"
    for char in source_text(block):
        bidi = unicodedata.bidirectional(char)
        if bidi in ("R", "AL"):
            return True
        if bidi == "L":
            return False
    return False


def ordered_structure(bundle):
    chapters = sorted(bundle["chapters"], key=lambda x: (x["position"], x["id"]))
    scenes_by_chapter = {}
    chapter_ids = {c["id"] for c in chapters}
    for scene in bundle["scenes"]:
        if scene["chapterId"] not in chapter_ids:
            fail(f"scene references missing chapter {scene['chapterId']}")
        scenes_by_chapter.setdefault(scene["chapterId"], []).append(scene)
    for values in scenes_by_chapter.values():
        values.sort(key=lambda x: (x["position"], x["id"]))
    return chapters, scenes_by_chapter


def expected_paragraphs(bundle):
    chapters, scenes_by_chapter = ordered_structure(bundle)
    start = bundle.get("chapterNumberStart", 1)
    expected = [{"text": bundle["project"]["title"], "style": "Title", "kind": "project"}]
    source_blocks = []
    for index, chapter in enumerate(chapters):
        expected.append({"text": f"Chapter {start + index} — {chapter['title']}", "style": "Heading1", "kind": "chapter", "page_break": index > 0})
        for scene in scenes_by_chapter.get(chapter["id"], []):
            expected.append({"text": scene["scene"]["title"], "style": "Heading2", "kind": "scene"})
            for block in scene["scene"].get("document", {}).get("content", []):
                if block.get("type") not in ("paragraph", "heading"):
                    continue
                level = block.get("attrs", {}).get("level", 2) if block["type"] == "heading" else None
                style = f"Heading{level}" if level is not None else None
                item = {"text": source_text(block), "style": style, "kind": "source", "source": block}
                expected.append(item)
                source_blocks.append(item)
    return expected, source_blocks


def read_document(path):
    try:
        with zipfile.ZipFile(path) as archive:
            bad = archive.testzip()
            if bad:
                fail(f"corrupt ZIP member: {bad}")
            names = set(archive.namelist())
            for required in ("[Content_Types].xml", "_rels/.rels", "word/document.xml"):
                if required not in names:
                    fail(f"missing OOXML part {required}")
            for name in names:
                if name.endswith((".xml", ".rels")):
                    try:
                        ET.fromstring(archive.read(name))
                    except ET.ParseError as error:
                        fail(f"invalid XML in {name}: {error}")
            root = ET.fromstring(archive.read("word/document.xml"))
    except (OSError, zipfile.BadZipFile) as error:
        fail(str(error))
    return names, root.findall("./w:body/w:p", NS)


def validate(path, bundle, expected, profile=None):
    names, paragraphs = read_document(path)
    actual = [visible_text(p) for p in paragraphs]
    texts = [item["text"] for item in expected]
    if actual != texts:
        limit = min(len(actual), len(texts))
        mismatch = next((i for i in range(limit) if actual[i] != texts[i]), limit)
        fail(f"paragraph order/text differs at {mismatch}; expected {len(texts)}, got {len(actual)}; expected={texts[mismatch:mismatch + 1]!r}, actual={actual[mismatch:mismatch + 1]!r}")

    for index, (p, item) in enumerate(zip(paragraphs, expected)):
        props = p.find("w:pPr", NS)
        style = props.find("w:pStyle", NS) if props is not None else None
        if item["style"] and (style is None or style.get(Q("val")) != item["style"]):
            fail(f"paragraph {index} should use style {item['style']}")
        if item["kind"] == "chapter":
            has_break = props is not None and props.find("w:pageBreakBefore", NS) is not None
            if has_break != item["page_break"]:
                fail(f"chapter paragraph {index} has incorrect pageBreakBefore")
        if item["kind"] == "source":
            source = item["source"]
            attrs = source.get("attrs", {})
            language = attrs.get("lang", "en")
            p_langs = [node.get(Q("val")) for node in p.findall(".//w:pPr//w:lang", NS)]
            run_langs = [node.get(Q("val")) for node in p.findall(".//w:rPr/w:lang", NS)]
            if language not in (p_langs + run_langs):
                fail(f"paragraph {index} lost language {language!r}")
            bidi = toggle_is_on(props.find("w:bidi", NS) if props is not None else None)
            if bidi != rtl_for(source):
                fail(f"paragraph {index} has incorrect RTL bidi setting")
            for run in p.findall("w:r", NS):
                if not run.findall("w:t", NS):
                    continue
                run_lang = run.find("w:rPr/w:lang", NS)
                if run_lang is None or run_lang.get(Q("val")) != language:
                    fail(f"paragraph {index} contains a text run without language {language!r}")
        spacing = props.find("w:spacing", NS) if props is not None else None
        if spacing is None or spacing.get(Q("before")) != "0" or spacing.get(Q("after")) != "0":
            fail(f"paragraph {index} must have zero before/after spacing")
        source = item.get("source")
        profile_spacing = profile.get("lineSpacing", "source") if profile else "source"
        body_paragraph = source is not None and source.get("type") == "paragraph"
        ratio = float(profile_spacing) if profile_spacing != "source" and body_paragraph else (source.get("attrs", {}).get("lineSpacing", 1.15) if source else 1.15)
        line = {1: "240", 1.15: "276", 1.5: "360", 2: "480"}[ratio]
        if spacing.get(Q("line")) != line or spacing.get(Q("lineRule")) != "auto":
            fail(f"paragraph {index} line spacing should be {ratio} (w:line={line}, auto)")

        if source:
            runs = p.findall("w:r", NS)
            run_cursor = 0
            for node in source.get("content", []):
                if node.get("type") != "text":
                    continue
                marks = {mark.get("type") if isinstance(mark, dict) else mark for mark in node.get("marks", [])}
                target = node.get("text", "")
                if not target:
                    continue
                matching = None
                matched_end = run_cursor
                for start in range(run_cursor, len(runs)):
                    collected = ""
                    for end in range(start, len(runs)):
                        collected += visible_text(runs[end])
                        if collected == target:
                            matching = runs[start:end + 1]
                            matched_end = end + 1
                            break
                        if len(collected) >= len(target):
                            break
                    if matching:
                        break
                if not matching:
                    if marks:
                        fail(f"formatted source run disappeared or was split: {target!r}")
                    continue
                # Advance in source order for plain and marked text so a later
                # repeated run cannot match an earlier occurrence.
                run_cursor = matched_end
                if not marks:
                    continue
                complex_script = any(unicodedata.bidirectional(char) in ("R", "AL") for char in target)
                for run in matching:
                    rpr = run.find("w:rPr", NS)
                    for mark, tag, complex_tag in (("bold", "b", "bCs"), ("italic", "i", "iCs"), ("underline", "u", "u")):
                        if mark in marks:
                            regular = rpr.find(f"w:{tag}", NS) if rpr is not None else None
                            complex_prop = rpr.find(f"w:{complex_tag}", NS) if rpr is not None else None
                            if not toggle_is_on(complex_prop if complex_script else regular):
                                fail(f"{mark} formatting lost for run {target!r}")
    return len(names), len(paragraphs)


def validate_profile(path, bundle, profile):
    try:
        with zipfile.ZipFile(path) as archive:
            styles_root = ET.fromstring(archive.read("word/styles.xml"))
            document_root = ET.fromstring(archive.read("word/document.xml"))
    except (OSError, KeyError, zipfile.BadZipFile, ET.ParseError) as error:
        fail(f"could not read DOCX style/page properties: {error}")

    styles = {style.get(Q("styleId")): style for style in styles_root.findall("w:style", NS)}

    def check_rpr(rpr, style_name, expected):
        if rpr is None:
            fail(f"{style_name} is missing run properties")
        fonts = rpr.find("w:rFonts", NS)
        for attr in ("ascii", "hAnsi", "eastAsia", "cs"):
            value = fonts.get(Q(attr)) if fonts is not None else None
            if value != expected["font"]:
                fail(f"{style_name} {attr} font should be {expected['font']!r}, got {value!r}")
        half_points = str(round(float(expected["size"]) * 2))
        for tag in ("sz", "szCs"):
            elem = rpr.find(f"w:{tag}", NS)
            if elem is None or elem.get(Q("val")) != half_points:
                fail(f"{style_name} {tag} should be {half_points} half-points")
        for attr, regular, complex_script in (("bold", "b", "bCs"), ("italic", "i", "iCs")):
            want = bool(expected[attr])
            for tag in (regular, complex_script):
                element = rpr.find(f"w:{tag}", NS)
                if toggle_is_on(element) != want:
                    fail(f"{style_name} {tag} should be {str(want).lower()}")

    style_map = (("title", "Title"), ("chapter", "Heading1"), ("scene", "Heading2"), ("body", "Normal"))
    for profile_key, style_id in style_map:
        style = styles.get(style_id)
        if style is None:
            fail(f"styles.xml is missing {style_id}")
        check_rpr(style.find("w:rPr", NS), style_id, profile[profile_key])
    defaults = styles_root.find("w:docDefaults/w:rPrDefault/w:rPr", NS)
    check_rpr(defaults, "defaultBody", profile["body"])

    section = document_root.find("./w:body/w:sectPr", NS)
    page_size = section.find("w:pgSz", NS) if section is not None else None
    page_dims = {"letter": (12240, 15840), "a4": (11906, 16838)}
    page_key = profile["pageSize"].lower()
    if page_key not in page_dims:
        fail(f"unsupported expected page size {profile['pageSize']!r}")
    if page_size is None:
        fail("document section is missing w:pgSz")
    actual_dims = (int(page_size.get(Q("w"), "0")), int(page_size.get(Q("h"), "0")))
    if actual_dims != page_dims[page_key]:
        fail(f"section page size should be {page_key}: {page_dims[page_key]}, got {actual_dims}")
    margins = section.find("w:pgMar", NS) if section is not None else None
    if margins is None:
        fail("document section is missing w:pgMar")
    expected_twips = round(float(profile["marginMm"]) * 1440 / 25.4)
    for side in ("top", "bottom", "left", "right"):
        if abs(int(margins.get(Q(side), "-100000")) - expected_twips) > 1:
            fail(f"section {side} margin should be {expected_twips} twips")

    # A non-source body spacing override applies only to ordinary manuscript
    # paragraphs. Generated headings and source headings retain their existing
    # 1.15 default or source-specific spacing.
    paragraphs = document_root.findall("./w:body/w:p", NS)
    expected, _ = expected_paragraphs(bundle)
    spacing_name = profile["lineSpacing"]
    if spacing_name == "source":
        return
    ratio = float(spacing_name)
    line_value = {1.0: "240", 1.15: "276", 1.5: "360", 2.0: "480"}[ratio]
    for index, (paragraph, item) in enumerate(zip(paragraphs, expected)):
        source = item.get("source")
        ppr = paragraph.find("w:pPr", NS)
        spacing = ppr.find("w:spacing", NS) if ppr is not None else None
        line = spacing.get(Q("line")) if spacing is not None else None
        is_body_paragraph = source is not None and source.get("type") == "paragraph"
        ratio = ratio if is_body_paragraph else (source.get("attrs", {}).get("lineSpacing", 1.15) if source else 1.15)
        expected_line = line_value if is_body_paragraph else {1: "240", 1.15: "276", 1.5: "360", 2: "480"}[ratio]
        if line != expected_line or spacing is None or spacing.get(Q("lineRule")) != "auto":
            kind = "body paragraph" if is_body_paragraph else "heading paragraph"
            fail(f"{kind} {index} should use expected line spacing (w:line={expected_line})")


def libreoffice_check(docx_path, expected, artifacts=None):
    preserve_dir = None
    if artifacts is None:
        output_context = tempfile.TemporaryDirectory(prefix="novel-docx-interop-")
        output_root = Path(output_context.name)
    else:
        base = Path(artifacts)
        base.mkdir(parents=True, exist_ok=True, mode=0o700)
        preserve_dir = Path(tempfile.mkdtemp(prefix="docx-interop-", dir=base))
        os.chmod(preserve_dir, 0o700)
        output_context = None
        output_root = preserve_dir

    # Keep LibreOffice's profile private and ephemeral regardless of whether
    # synthetic QA output is intentionally preserved.
    profile_context = tempfile.TemporaryDirectory(prefix="novel-docx-lo-profile-")
    input_dir = output_root / "input"
    roundtrip_dir = output_root / "roundtrip"
    pdf_dir = output_root / "pdf"
    for folder in (input_dir, roundtrip_dir, pdf_dir):
        folder.mkdir(mode=0o700)
    source_docx = input_dir / Path(docx_path).name
    roundtrip_docx = roundtrip_dir / source_docx.name
    pdf = pdf_dir / (source_docx.stem + ".pdf")
    source_docx.write_bytes(Path(docx_path).read_bytes())
    try:
        profile = Path(profile_context.name)
        result = subprocess.run(
            ["libreoffice", "-env:UserInstallation=" + profile.as_uri(), "--headless", "--convert-to", "docx", "--outdir", str(roundtrip_dir), str(source_docx)],
            capture_output=True, text=True, timeout=60,
        )
        if result.returncode or not roundtrip_docx.is_file():
            fail("LibreOffice DOCX conversion failed: " + (result.stderr or result.stdout).strip()[-1000:])
        _, roundtrip_paras = read_document(roundtrip_docx)
        wanted = [item["text"] for item in expected]
        got = [visible_text(p) for p in roundtrip_paras]
        if got != wanted:
            limit = min(len(got), len(wanted))
            mismatch = next((i for i in range(limit) if got[i] != wanted[i]), limit)
            fail(f"LibreOffice DOCX roundtrip changed visible paragraph text/order at index {mismatch}; expected {len(wanted)} paragraphs, got {len(got)}; expected={wanted[mismatch:mismatch + 1]!r}, actual={got[mismatch:mismatch + 1]!r}. Artifacts: {roundtrip_docx}")
        result = subprocess.run(
            ["libreoffice", "-env:UserInstallation=" + profile.as_uri(), "--headless", "--convert-to", "pdf", "--outdir", str(pdf_dir), str(roundtrip_docx)],
            capture_output=True, text=True, timeout=60,
        )
        if result.returncode or not pdf.is_file() or not pdf.read_bytes().startswith(b"%PDF-"):
            fail("LibreOffice PDF render failed: " + (result.stderr or result.stdout).strip()[-1000:])
        return roundtrip_docx, pdf, preserve_dir, pdf.stat().st_size
    except subprocess.TimeoutExpired as error:
        fail(f"LibreOffice conversion timed out: {error}")
    except OSError as error:
        fail(f"LibreOffice conversion unavailable: {error}")
    finally:
        profile_context.cleanup()
        if output_context is not None:
            output_context.cleanup()


def run(docx_path, bundle_path, use_libreoffice=False, artifacts=None, profile_path=None):
    bundle = json.loads(Path(bundle_path).read_text(encoding="utf-8"))
    expected, _ = expected_paragraphs(bundle)
    profile = json.loads(Path(profile_path).read_text(encoding="utf-8")) if profile_path else None
    parts, count = validate(docx_path, bundle, expected, profile)
    if profile is not None:
        validate_profile(docx_path, bundle, profile)
    extra = ""
    if use_libreoffice:
        roundtrip, pdf, preserved, pdf_size = libreoffice_check(docx_path, expected, artifacts)
        extra = f"; LibreOffice DOCX roundtrip text passed and PDF rendered ({pdf_size} bytes)"
        if preserved:
            extra += f"; artifacts preserved in {preserved}"
    print(f"DOCX valid: {parts} ZIP parts, {count} ordered paragraphs with contract attributes{extra}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("docx")
    parser.add_argument("bundle")
    parser.add_argument("--libreoffice", action="store_true")
    parser.add_argument("--artifacts", type=Path, help="Preserve LibreOffice QA files in a unique private subfolder under this directory")
    parser.add_argument("--profile", type=Path, help="Expected Word export profile JSON to verify styles, page geometry, and spacing")
    args = parser.parse_args()
    if args.artifacts and not args.libreoffice:
        parser.error("--artifacts requires --libreoffice")
    run(args.docx, args.bundle, args.libreoffice, args.artifacts, args.profile)
