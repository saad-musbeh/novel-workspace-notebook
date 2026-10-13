#!/usr/bin/env python3
"""Focused standard-library tests for the native English catalogue compiler."""
import importlib.util
import json
from pathlib import Path
import unittest

path = Path(__file__).with_name('generate-native-ui-text.py')
spec = importlib.util.spec_from_file_location('native_ui_text', path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def catalogue(strings):
    return json.dumps({'schemaVersion': 1, 'locale': 'en', 'strings': strings}, ensure_ascii=False)


class CatalogueTests(unittest.TestCase):
    def test_unicode_and_code_looking_literals_are_escaped_without_rewriting(self):
        text = 'نور 明 e\u0301 \ue000 👩🏽‍🚀 "quote" \\ literal'
        output = module.render(module.load(catalogue({'label.example': text})))
        self.assertIn(json.dumps(text, ensure_ascii=False), output)
        self.assertNotIn('\\u0301', output)
        self.assertEqual(module.load(catalogue({'label.example': text}))['label.example'], text)

    def test_duplicate_keys_and_locale_shape_fail_closed(self):
        with self.assertRaises(ValueError):
            module.load('{"schemaVersion":1,"locale":"en","strings":{"label.x":"One","label.x":"Two"}}')
        for item in [{'schemaVersion': True, 'locale': 'en', 'strings': {'x': 'X'}},
                     {'schemaVersion': 1, 'locale': 'ar', 'strings': {'x': 'X'}},
                     {'schemaVersion': 1, 'locale': 'en', 'strings': {'x': 'X'}, 'extra': True}]:
            with self.assertRaises(ValueError):
                module.load(json.dumps(item))

    def test_invalid_keys_controls_templates_and_bounds_are_rejected(self):
        for strings in [{'Bad Key': 'Label'}, {'label.x': ''}, {'label.x': 'A\nB'},
                        {'label.x': '\ud800'}, {'label.x': 'Hello {author}'},
                        {'label.x': 'x' * 4001}, {f'label.k{i}': 'X' for i in range(2001)}]:
            with self.assertRaises(ValueError):
                module.load(catalogue(strings))

    def test_generation_is_deterministic_and_matches_selected_source(self):
        strings = module.load(module.SOURCE.read_text(encoding='utf-8'))
        self.assertEqual(module.render(strings), module.render(dict(reversed(list(strings.items())))))
        self.assertEqual(module.TARGET.read_text(encoding='utf-8'), module.render(strings))


if __name__ == '__main__':
    unittest.main()
