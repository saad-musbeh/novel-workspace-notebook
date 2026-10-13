#!/usr/bin/env python3
"""Check the declared partial native English label hooks, without running GTK.

The manifest records this slice's original English captions and minimum static
references. It deliberately does not claim all native strings are externalized.
"""
import argparse
from collections import Counter
import importlib.util
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'apps/native-writer/locales/coverage.json'


def verify(root=ROOT):
    manifest = json.loads((root / 'apps/native-writer/locales/coverage.json').read_text(encoding='utf-8'))
    catalog = json.loads((root / 'apps/native-writer/locales/en.json').read_text(encoding='utf-8'))['strings']
    if manifest.get('schemaVersion') != 1 or manifest.get('coverage') != 'partial-shell-and-shared-forms':
        raise ValueError('Unsupported UI hook coverage manifest')
    for key, original in manifest['originalEnglish'].items():
        if catalog.get(key) != original:
            raise ValueError('English caption changed without explicit coverage review: ' + key)
    for file, minimum in manifest['minimumReferences'].items():
        source = (root / file).read_text(encoding='utf-8')
        prefix = r'crate::ui_text::text\s*\('
        calls = re.findall(prefix + r'\s*("[^"\n]+"|[a-zA-Z_][a-zA-Z0-9_]*)\s*\)', source)
        if len(calls) != len(re.findall(prefix, source)):
            raise ValueError('Unreviewed dynamic UI lookup in ' + file)
        keys = []
        for argument in calls:
            if argument == 'key' and file.endswith('/app.rs'):
                continue  # the separately checked area-ID -> label-key table
            if not argument.startswith('"'):
                raise ValueError('Unreviewed dynamic UI lookup in ' + file)
            key = json.loads(argument)
            if key not in catalog:
                raise ValueError('Missing UI catalogue key: ' + key)
            keys.append(key)
        actual = Counter(keys)
        for key, expected in minimum.items():
            if actual[key] < expected:
                raise ValueError('Declared UI hook disappeared: ' + file + ' ' + key)
    for file, patterns in manifest['stableIdentifierPatterns'].items():
        source = (root / file).read_text(encoding='utf-8')
        for pattern in patterns:
            if not re.search(pattern, source):
                raise ValueError('A declared route/format identifier hook changed in ' + file)
    return len(manifest['originalEnglish'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    try:
        count = verify()
    except (ValueError, OSError) as error:
        parser.exit(1, str(error) + '\n')
    print(f'Partial UI hook coverage passed: {count} original English captions; route IDs stay separate.')


if __name__ == '__main__':
    main()
