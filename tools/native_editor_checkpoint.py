"""Private, transient editor checkpoints; no executable serialization.

Shared paragraph objects are stored once, retaining independent scene Undo while
offscreen scenes release their Pango layouts and helper processes.
"""
import json
import os
from pathlib import Path
from native_text_model import TextModel, validate_document, MARKS, text_of

MAX_BYTES = 128 * 1024 * 1024

def write_checkpoint(directory, editor, opening):
    model = editor.model
    blocks = []
    identities = {}
    def state(value):
        nodes, cursor, anchor, marks = value
        refs = []
        for block in nodes:
            key = id(block)
            if key not in identities:
                identities[key] = len(blocks)
                blocks.append(block)
            refs.append(identities[key])
        return [refs, list(cursor), list(anchor), None if marks is None else sorted(marks)]
    payload = {'schemaVersion': 1, 'opening': opening, 'original': model._original,
               'state': state(model._state()),
               'undo': [[state(value), cost] for value, cost in model.undo],
               'redo': [[state(value), cost] for value, cost in model.redo],
               'blocks': blocks, 'trimmed': model.undo_trimmed,
               'version': model.version, 'top': editor.top, 'height': editor.height,
               'width': editor.layout.width, 'document': model.document_snapshot()}
    data = json.dumps(payload, ensure_ascii=False, separators=(',', ':')).encode('utf-8')
    if len(data) > MAX_BYTES:
        raise ValueError('This scene’s Undo checkpoint is too large. The open editor is retained.')
    directory = Path(directory)
    temporary = directory / 'checkpoint.pending'
    descriptor = os.open(temporary, os.O_CREAT | os.O_TRUNC | os.O_WRONLY, 0o600)
    with os.fdopen(descriptor, 'wb') as output:
        output.write(data)
        output.flush()
        os.fsync(output.fileno())
    os.replace(temporary, directory / 'checkpoint.json')

def read_checkpoint(directory):
    path = Path(directory) / 'checkpoint.json'
    with path.open('rb') as source:
        data = source.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES:
        raise ValueError('The private editor checkpoint exceeds its limit.')
    value = json.loads(data)
    if not isinstance(value, dict) or value.get('schemaVersion') != 1:
        raise ValueError('Unsupported private editor checkpoint.')
    opening = value.get('opening')
    if not isinstance(opening, dict) or opening.get('type') != 'open':
        raise ValueError('Invalid private editor configuration.')
    validate_document(value.get('original'))
    validate_document(value.get('document'))
    nodes = value.get('blocks')
    if not isinstance(nodes, list) or len(nodes) > 3_000_000:
        raise ValueError('Invalid private paragraph table.')
    for block in nodes:
        validate_document({'type': 'doc', 'content': [block]})
    lengths = [len(text_of(block)) for block in nodes]
    def state(raw):
        if not isinstance(raw, list) or len(raw) != 4 or not isinstance(raw[0], list) or not raw[0] or len(raw[0]) > 20_000:
            raise ValueError('Invalid private editor state.')
        if any(type(ref) is not int or not 0 <= ref < len(nodes) for ref in raw[0]):
            raise ValueError('Invalid private paragraph reference.')
        for position in raw[1:3]:
            if not isinstance(position, list) or len(position) != 2 or any(type(part) is not int for part in position) or not 0 <= position[0] < len(raw[0]) or not 0 <= position[1] <= lengths[raw[0][position[0]]]:
                raise ValueError('Invalid private cursor position.')
        marks = raw[3]
        if marks is not None and (not isinstance(marks, list) or any(mark not in MARKS for mark in marks)):
            raise ValueError('Invalid private typing marks.')
        return (tuple(nodes[ref] for ref in raw[0]), tuple(raw[1]), tuple(raw[2]), None if marks is None else frozenset(marks))
    def history(raw):
        if not isinstance(raw, list) or len(raw) > 512:
            raise ValueError('Invalid private Undo history.')
        result = []
        for entry in raw:
            if not isinstance(entry, list) or len(entry) != 2 or type(entry[1]) is not int or not 0 <= entry[1] <= 64 * 1024 * 1024:
                raise ValueError('Invalid private Undo entry.')
            result.append((state(entry[0]), entry[1]))
        return result
    restored = TextModel(value['original'])
    restored._restore(state(value.get('state')))
    restored.undo = history(value.get('undo'))
    restored.redo = history(value.get('redo'))
    restored.undo_bytes = sum(cost for _, cost in restored.undo)
    if restored.undo_bytes > 64 * 1024 * 1024 or type(value.get('trimmed')) is not bool or type(value.get('version')) is not int or value['version'] < 0:
        raise ValueError('Invalid private Undo accounting.')
    restored.undo_trimmed = value['trimmed']
    restored.version = value['version']
    if restored.document_snapshot() != value['document']:
        raise ValueError('The private editor checkpoint does not match its document.')
    if any(type(value.get(field)) is not int for field in ('top', 'height', 'width')) or not 0 <= value['top'] <= 100_000_000 or not 120 <= value['height'] <= 2160 or not 240 <= value['width'] <= 4096:
        raise ValueError('Invalid private editor viewport.')
    return value, restored
