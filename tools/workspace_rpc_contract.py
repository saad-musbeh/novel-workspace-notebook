"""Narrow source-derived contract for direct storage RPC dispatch arms.

This is not a Rust request enum or a general Rust parser. Covered operations
must retain this exact supported dispatch shape; changed syntax fails closed.
"""
import re
import json

# Coverage selection only: field names and types are derived, never hand copied.
OPERATIONS = (
    'create_universe','create_universe_novel','create_novel','load', 'save', 'list_projects', 'create_project', 'open_project',
    'metadata_resolution_snapshot', 'resolve_metadata_checked', 'scene_arrangement_snapshot', 'arrange_scene', 'chapter_edit_snapshot', 'edit_chapter', 'rename_project', 'create_chapter', 'rename_chapter', 'reorder_chapters',
    'open_project_index', 'load_project_scene', 'create_scene', 'load_scene', 'rename_scene', 'reorder_scenes', 'history',
    'restore_scene', 'export_archive', 'import_archive', 'load_project_data',
    'review_sources_check', 'planning_read_snapshot', 'alphabet_export_snapshot', 'load_project_data_unambiguous', 'load_entity_revision', 'project_data_history', 'load_project_data_revision',
    'save_project_data', 'move_scene', 'load_revision', 'snapshot_scene',
    'scene_conflict_index', 'scene_conflict_snapshot', 'resolve_scene_conflict_checked', 'conflicts', 'resolve_scene', 'get_asset', 'put_asset', 'list_assets', 'deleted_scenes',
    'project_time_points', 'project_time_preview', 'recover_time_piece',
    'create_project_branch', 'list_project_branches', 'compare_project_branch', 'bring_back_branch_piece',
)


def comma_parts(text):
    parts, start, stack = [], 0, []
    pairs = {')': '(', '>': '<', ']': '['}
    for index, char in enumerate(text):
        if char in '(<[':
            stack.append(char)
        elif char in ')>]':
            if not stack or stack.pop() != pairs[char]:
                raise ValueError('Unbalanced direct RPC expression')
        elif char == ',' and not stack:
            parts.append(text[start:index].strip())
            start = index + 1
    if stack:
        raise ValueError('Unbalanced direct RPC expression')
    if text[start:].strip():
        parts.append(text[start:].strip())
    return parts


def derive(root, rust_type, uncomment):
    source = uncomment((root / 'crates/companion/src/main.rs').read_text(encoding='utf-8'))
    storage = uncomment('\n'.join((root / path).read_text(encoding='utf-8') for path in ['crates/storage/src/workspace.rs','crates/storage/src/time_machine.rs','crates/storage/src/branches.rs','crates/storage/src/universe.rs','crates/storage/src/scene_arrangement.rs','crates/storage/src/sync.rs','crates/storage/src/planning_read.rs','crates/storage/src/review_sources.rs','crates/storage/src/scene_conflict_snapshot.rs','crates/storage/src/alphabet_export.rs']))
    contracts = {}
    for operation in OPERATIONS:
        arms = re.findall(r'"' + operation + r'"\s*=>\s*encode\(storage\.([a-z_]+)\((.*?)\)\.await\?\),', source, re.S)
        if len(arms) != 1:
            raise ValueError(f'Unsupported or missing covered RPC arm: {operation}')
        method, arguments = arms[0]
        signatures = re.findall(r'pub\s+async\s+fn\s+' + method + r'\s*\((.*?)\)\s*->\s*Result\s*<\s*(.*?)\s*,\s*String\s*>\s*\{', storage, re.S)
        if len(signatures) != 1:
            raise ValueError(f'Unsupported or missing storage method: {method}')
        parameters, result = signatures[0]
        parameters = comma_parts(parameters)
        if not parameters or parameters.pop(0) != '&self':
            raise ValueError(f'Unsupported method receiver: {method}')
        arguments = comma_parts(arguments)
        if len(arguments) != len(parameters):
            raise ValueError(f'RPC arity differs from storage method: {operation}')
        fields = {}
        for argument, parameter in zip(arguments, parameters):
            match = re.fullmatch(r'&?argument(?:\s*::<\s*(.*?)\s*>)?\(\s*&args\s*,\s*"([A-Za-z][A-Za-z0-9]*)"\s*\)\?', argument, re.S)
            declaration = re.fullmatch(r'[a-z_]+\s*:\s*(.+)', parameter, re.S)
            if not match or not declaration:
                raise ValueError(f'Unsupported direct argument in {operation}')
            kind = re.sub(r'\s+', '', declaration[1])
            if kind == '&str':
                kind = 'String'
            elif argument.strip().startswith('&') and re.fullmatch(r'&\[[A-Za-z][A-Za-z0-9]*\]', kind):
                kind = 'Vec<' + kind[2:-1] + '>'
            elif kind.startswith('&') and argument.strip().startswith('&') and re.fullmatch(r'&[A-Za-z][A-Za-z0-9]*', kind):
                # A borrowed direct RPC value is serialized as its selected owned type.
                # Borrowed serde fields remain unsupported in the struct parser.
                kind = kind[1:]
            kind = rust_type(kind)
            if match[1] and rust_type(match[1]) != kind:
                raise ValueError(f'Explicit argument type differs from method in {operation}')
            if match[2] in fields:
                raise ValueError(f'Duplicate RPC key: {operation}.{match[2]}')
            fields[match[2]] = kind
        result = re.sub(r'\s+', '', result)
        contracts[operation] = {'method': method, 'args': fields, 'result': '()' if result == '()' else rust_type(result)}
    return contracts, sorted(set(re.findall(r'"([a-z_]+)"(?=\s*(?:\|\s*"[a-z_]+"\s*)*=>)', source.split('#[tokio::main]')[0])) - set(contracts))


def render(model, ts_type):
    contracts, uncovered = model
    def bind(kind):
        if kind == '()':
            return 'null'
        # All opaque serde Values deliberately remain unknown.
        if kind == 'Scene':
            return 'TScene'
        if kind in {'Snapshot', 'SceneRecord', 'ProjectBundle', 'UniverseNovelCreated', 'NovelCreated', 'SceneConflictRevision', 'SceneConflictSnapshot'}:
            return kind + '<TScene>'
        if kind.startswith('Vec<'):
            return 'Array<' + bind(kind[4:-1]) + '>'
        if kind.startswith('Option<'):
            return '(' + bind(kind[7:-1]) + ' | null)'
        return ts_type(kind, 'Rpc')
    lines = [
        '// Source-derived subset of direct storage RPC arms and method signatures.',
        '// Not a Rust typed request enum. Complex/dynamic operations are uncovered.',
        '// Opaque document and payload Values retain their existing external validation.',
        'export interface StorageRpcContract<TScene extends Scene = Scene> {',
    ]
    for operation, contract in contracts.items():
        fields = ' '.join(name + ': ' + bind(kind) + ';' for name, kind in contract['args'].items())
        lines.append("  " + operation + ": {args: {" + fields + "}; result: " + bind(contract["result"]) + "};")
    lines.extend(['}', '', 'export const storageRpcCoverage = ' + json.dumps({'covered': list(contracts), 'uncovered': uncovered}, indent=2) + ' as const;', ''])
    return '\n'.join(lines)
