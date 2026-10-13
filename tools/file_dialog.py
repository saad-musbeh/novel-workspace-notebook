#!/usr/bin/python3
"""User-triggered GTK chooser. The UI supplies formats/names, never destination paths."""
import json
from pathlib import Path
import sys

FORMATS = {
    'docx': ('Word manuscript (.docx)', ['*.docx']),
    'text': ('Plain UTF-8 text', ['*.txt']),
    'markdown': ('Markdown', ['*.md', '*.markdown']),
    'html': ('Self-contained HTML', ['*.html', '*.htm']),
    'project': ('Novel Workspace project archive', ['*.novel.json', '*.json']),
    'alphabet': ('Alphabet definitions', ['*.alphabet.json', '*.json']),
    'dictionary': ('Local dictionary data', ['*.json']),
}

def choose(request):
    if not isinstance(request, dict) or request.get('mode') not in ['open', 'save'] or request.get('format') not in FORMATS:
        raise ValueError('Unsupported file selection.')
    suggested = request.get('suggestedName', 'document')
    if not isinstance(suggested, str) or not suggested or len(suggested) > 200 or suggested in ['.', '..'] or any(c in suggested for c in '/\\\0'):
        raise ValueError('Invalid suggested filename.')
    import gi
    gi.require_version('Gtk', '3.0')
    from gi.repository import Gtk
    initialized, _ = Gtk.init_check(None)
    if not initialized:
        raise RuntimeError('The desktop file chooser is unavailable. Keep the document open and try again in your desktop session.')
    saving = request['mode'] == 'save'
    action = Gtk.FileChooserAction.SAVE if saving else Gtk.FileChooserAction.OPEN
    dialog = Gtk.FileChooserNative.new('Save a local file' if saving else 'Open a local file', None, action, 'Save' if saving else 'Open', 'Cancel')
    dialog.set_modal(True)
    dialog.set_local_only(True)
    if saving:
        dialog.set_current_name(suggested)
        dialog.set_do_overwrite_confirmation(True)
    name, patterns = FORMATS[request['format']]
    file_filter = Gtk.FileFilter()
    file_filter.set_name(name)
    for pattern in patterns:
        file_filter.add_pattern(pattern)
    dialog.add_filter(file_filter)
    try:
        response = dialog.run()
        filename = dialog.get_filename() if response == Gtk.ResponseType.ACCEPT else None
        return {'cancelled': filename is None, **({'path': str(Path(filename))} if filename else {})}
    finally:
        dialog.destroy()

def main():
    try:
        line = sys.stdin.buffer.readline(4097)
        if len(line) > 4096:
            raise ValueError('File chooser request is too large.')
        print(json.dumps({'ok': True, **choose(json.loads(line))}), flush=True)
    except Exception as error:
        print(json.dumps({'ok': False, 'error': str(error)}), flush=True)
        return 1
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
