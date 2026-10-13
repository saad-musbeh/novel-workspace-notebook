#!/usr/bin/env python3
"""Generate the narrow storage serde wire contract using only Python's stdlib.

Supports named public structs, camelCase serde names, String/numeric/bool/Value,
Vec/Option and references to the selected structs. Unsupported syntax/serde
behavior fails closed. JSON documents/payloads stay opaque generic bindings;
the frontend DocumentNode validator remains manual. Direct storage RPC signatures
are derived from companion dispatch and storage methods; complex arms stay uncovered.
"""
import argparse
import json
from pathlib import Path
import re
import sys
from workspace_rpc_contract import derive as derive_rpc, render as render_rpc

STRUCTS = {
    "ExportStyle": "crates/storage/src/export_profile.rs",
    "ExportProfile": "crates/storage/src/export_profile.rs",
    "Scene": "crates/storage/src/lib.rs",
    "Snapshot": "crates/storage/src/lib.rs",
    "UniverseSeries": "crates/storage/src/universe.rs",
    "UniverseNovel": "crates/storage/src/universe.rs",
    "UniverseCatalogue": "crates/storage/src/universe.rs",
    "NovelCreated": "crates/storage/src/universe.rs",
    "UniverseNovelCreated": "crates/storage/src/universe.rs",
    "Project": "crates/storage/src/workspace.rs",
    "Chapter": "crates/storage/src/workspace.rs",
    "ChapterEditSnapshot": "crates/storage/src/workspace.rs",
    "SceneRecord": "crates/storage/src/workspace.rs",
    "ProjectBundle": "crates/storage/src/workspace.rs",
    "SceneSummary": "crates/storage/src/workspace.rs",
    "ProjectIndex": "crates/storage/src/workspace.rs",
    "Revision": "crates/storage/src/workspace.rs",
    "ProjectTimePoint": "crates/storage/src/workspace.rs",
    "ProjectTimePoints": "crates/storage/src/workspace.rs",
    "TimeMachineHead": "crates/storage/src/workspace.rs",
    "TimeMachineObject": "crates/storage/src/workspace.rs",
    "ProjectTimePreview": "crates/storage/src/workspace.rs",
    "ProjectDataSnapshot": "crates/storage/src/workspace.rs",
    "ProjectAsset": "crates/storage/src/workspace.rs",
    "BranchMapping": "crates/storage/src/branches.rs",
    "BranchSceneMapping": "crates/storage/src/branches.rs",
    "BranchOrigin": "crates/storage/src/branches.rs",
    "BranchLineage": "crates/storage/src/branches.rs",
    "BranchInfo": "crates/storage/src/branches.rs",
    "BranchPiece": "crates/storage/src/branches.rs",
    "BranchComparison": "crates/storage/src/branches.rs",

}
OUTPUT = "apps/writer/src/generated/workspace-contract.ts"
NUMBERS = {"u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize", "f32", "f64"}


class ContractError(ValueError):
    pass


def uncomment(source):
    # Preserve string contents; do not treat URLs or literal slash pairs as comments.
    token = re.compile(r'"(?:\\.|[^"\\])*"|/\*.*?\*/|//[^\n]*', re.S)
    return token.sub(lambda match: match[0] if match[0].startswith('"') else ' ', source)


def split_fields(body):
    fields, start, depth = [], 0, 0
    for index, char in enumerate(body):
        if char == "<":
            depth += 1
        elif char == ">":
            depth -= 1
            if depth < 0:
                raise ContractError("Unbalanced Rust field type")
        elif char == "," and depth == 0:
            fields.append(body[start:index].strip())
            start = index + 1
    if depth:
        raise ContractError("Unbalanced Rust field type")
    fields.append(body[start:].strip())
    return [field for field in fields if field]


def rust_type(text):
    text = re.sub(r"\s+", "", text)
    if text in {"String", "bool", "Value"} | NUMBERS | STRUCTS.keys():
        return text
    match = re.fullmatch(r"(Vec|Option)<(.+)>", text)
    if match:
        return match[1] + "<" + rust_type(match[2]) + ">"
    raise ContractError(f"Unsupported Rust field type: {text}")


def camel(name):
    pieces = name.split("_")
    if not pieces[0] or any(not piece for piece in pieces):
        raise ContractError(f"Unsupported Rust field identifier: {name}")
    return pieces[0] + "".join(piece[:1].upper() + piece[1:] for piece in pieces[1:])


def read_struct(source, name):
    source = uncomment(source)
    pattern = re.compile(r"(?P<attrs>(?:#\[[^\]]*\]\s*)*)pub\s+struct\s+" + re.escape(name) + r"\b(?P<tail>[^{};]*)(?P<opening>[{;])")
    matches = list(pattern.finditer(source))
    if len(matches) != 1:
        raise ContractError(f"Expected exactly one public Rust struct {name}")
    match = matches[0]
    if match["tail"].strip() or match["opening"] != "{":
        raise ContractError(f"Unsupported tuple/generic Rust struct {name}")
    attrs = re.findall(r"#\[([^\]]*)\]", match["attrs"])
    rename_all, deny_unknown, derives = None, False, set()
    for attr in attrs:
        if attr.startswith("derive(") and attr.endswith(")"):
            derives.update(part.strip() for part in attr[7:-1].split(","))
        elif attr.startswith("serde(") and attr.endswith(")"):
            for part in attr[6:-1].split(","):
                part = part.strip()
                if part == "deny_unknown_fields":
                    deny_unknown = True
                elif re.fullmatch(r'rename_all\s*=\s*"camelCase"', part):
                    rename_all = "camelCase"
                else:
                    raise ContractError(f"Unsupported serde attribute on {name}: {part}")
        else:
            raise ContractError(f"Unsupported Rust attribute on {name}: {attr}")
    if not {"Serialize", "Deserialize"}.issubset(derives) or rename_all != "camelCase":
        raise ContractError(f"{name} must derive Serialize/Deserialize with explicit camelCase")
    end = source.find("}", match.end())
    if end < 0 or "{" in source[match.end():end]:
        raise ContractError(f"Unsupported Rust body for {name}")
    fields, names = [], set()
    for raw in split_fields(source[match.end():end]):
        # Only the new backward-compatible chapter numbering fields support
        # defaults. They remain required in current serialized responses.
        attr = re.match(r'#\[serde\((.*?)\)\]\s*', raw)
        if attr:
            raw = raw[attr.end():]
            allowed = name in {'ProjectIndex', 'ProjectBundle'} and (
                (attr[1] == 'default="default_chapter_number_start"' and re.fullmatch(r'pub\s+chapter_number_start\s*:\s*i64', raw) and re.search(r'fn\s+default_chapter_number_start\(\)\s*->\s*i64\s*\{\s*1\s*\}', source)) or
                (attr[1] == 'default' and re.fullmatch(r'pub\s+(chapter_numbering_revision_id\s*:\s*Option<String>|chapter_numbering_conflicted\s*:\s*bool)', raw)))
            if not allowed:
                raise ContractError(f"Unsupported field serde default in {name}")
        field = re.fullmatch(r"pub\s+([A-Za-z][A-Za-z0-9_]*)\s*:\s*(.+)", raw, re.S)
        if not field:
            raise ContractError(f"Unsupported Rust field/attribute in {name}: {raw}")
        wire = camel(field[1])
        if wire in names:
            raise ContractError(f"Duplicate serialized field in {name}: {wire}")
        names.add(wire)
        fields.append({"rustName": field[1], "name": wire, "rustType": rust_type(field[2])})
    if not fields:
        raise ContractError(f"Empty wire struct {name}")
    return {"fields": fields, "denyUnknownFields": deny_unknown, "declaration": source[match.start():end + 1]}


def declarations(root):
    sources = {path: (root / path).read_text(encoding="utf-8") for path in set(STRUCTS.values())}
    return {name: read_struct(sources[path], name) for name, path in STRUCTS.items()}


def ts_type(kind, owner):
    if kind == "String":
        return "string"
    if kind == "bool":
        return "boolean"
    if kind in NUMBERS:
        return "number"
    if kind == "Value":
        if owner == "Scene":
            return "TDocument"
        if owner in {"Revision", "ProjectDataSnapshot"}:
            return "TPayload"
        return "unknown"
    if kind.startswith("Vec<"):
        return "Array<" + ts_type(kind[4:-1], owner) + ">"
    if kind.startswith("Option<"):
        return "(" + ts_type(kind[7:-1], owner) + " | null)"
    if kind == "Scene" and owner in {"Snapshot", "SceneRecord", "ProjectBundle"}:
        return "TScene"
    if kind == "SceneRecord" and owner in {"ProjectBundle", "UniverseNovelCreated", "NovelCreated"}:
        return "SceneRecord<TScene>"
    if kind in STRUCTS:
        return kind
    raise ContractError(f"Unsupported TypeScript binding for {kind}")


def generate(structs, contracts=None):
    lines = ["// Generated by tools/generate-workspace-contract.py; do not edit.",
             "// Coverage: named storage wire structs and a direct-storage RPC subset; runtime validators remain separate.",
             "// Rust Value is opaque. i64/u64/usize use JSON numbers; callers must enforce safe integer bounds.", ""]
    for name, model in structs.items():
        generic = ""
        if name == "Scene":
            generic = "<TDocument = unknown>"
        elif name in {"Revision", "ProjectDataSnapshot"}:
            generic = "<TPayload = unknown>"
        elif name in {"Snapshot", "SceneRecord", "ProjectBundle", "UniverseNovelCreated", "NovelCreated"}:
            generic = "<TScene extends Scene = Scene>"
        lines.append(f"export interface {name}{generic} {{")
        for field in model["fields"]:
            lines.append(f"  {field['name']}: {ts_type(field['rustType'], name)};")
        lines.extend(["}", ""])
    metadata = {name: {"source": STRUCTS[name], "denyUnknownFields": model["denyUnknownFields"], "fields": model["fields"]} for name, model in structs.items()}
    lines.extend(["export const workspaceWireSchema = " + json.dumps(metadata, ensure_ascii=False, indent=2) + " as const;", ""])
    if contracts is not None:
        lines.append(render_rpc(contracts, ts_type))
    return "\n".join(lines)


def fixture_value(kind, owner, field):
    if kind == "String":
        return json.dumps(f"{owner}.{field} مِ 中文 😀", ensure_ascii=False) + ".into()"
    if kind == "bool":
        return "true"
    if kind in NUMBERS:
        return "1.0" if kind.startswith("f") else "1"
    if kind == "Value":
        return 'serde_json::json!({"type":"doc","content":[]})' if owner == "Scene" else 'serde_json::json!({"note":"Synthetic مِ 中文 😀"})'
    if kind.startswith("Vec<"):
        inner = kind[4:-1]
        return "vec![0,128,255]" if inner == "u8" else "vec![" + fixture_value(inner, owner, field) + "]"
    if kind.startswith("Option<"):
        return "Some(" + fixture_value(kind[7:-1], owner, field) + ")"
    if kind in STRUCTS:
        return f"sample_{kind}()"
    raise ContractError(f"Unsupported serialization fixture type: {kind}")


def rust_fixture(structs):
    lines = ["// Synthetic serde fixture using the exact selected Rust declarations.", "use serde::{Serialize, Deserialize};", "use serde_json::Value;", "fn default_chapter_number_start()->i64{1}"]
    lines.extend(model["declaration"] for model in structs.values())
    for name, model in structs.items():
        fields = ",".join(field["rustName"] + ":" + fixture_value(field["rustType"], name, field["rustName"]) for field in model["fields"])
        lines.append(f"#[allow(non_snake_case)] fn sample_{name}()->{name} {{{name}{{{fields}}}}}")
    values = '"__rpcUnit":serde_json::to_value(()).unwrap(),' + ",".join(json.dumps(name) + f":serde_json::to_value(sample_{name}()).unwrap()" for name in structs)
    lines.append('fn main(){println!("{}",serde_json::json!({' + values + '}));}')
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--check", action="store_true", help="fail when the committed generated file differs")
    parser.add_argument("--rust-fixture", action="store_true", help="emit a synthetic actual-serde serialization fixture to stdout")
    args = parser.parse_args()
    try:
        structs = declarations(args.root)
        if args.rust_fixture:
            if args.check:
                raise ContractError("--rust-fixture cannot be combined with --check")
            sys.stdout.write(rust_fixture(structs))
            return 0
        contracts = derive_rpc(args.root, rust_type, uncomment)
        output, expected = args.root / OUTPUT, generate(structs, contracts)
        if args.check:
            if not output.exists() or output.read_text(encoding="utf-8") != expected:
                print("Workspace contract drift: run python3 tools/generate-workspace-contract.py", file=sys.stderr)
                return 1
        else:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(expected, encoding="utf-8", newline="\n")
        return 0
    except (ValueError, OSError) as error:
        print(f"Workspace contract generation refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
