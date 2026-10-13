"""Exercise only an isolated debug app's save-before-close path on Fedora."""
import json
import os
from pathlib import Path
import signal
import sqlite3
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class NativeCloseTest(unittest.TestCase):
    def test_queued_multilingual_input_commits_before_native_close(self):
        executable = ROOT / 'target/debug/novel-workspace'
        self.assertTrue(executable.is_file(), 'Build the native debug executable first.')
        with tempfile.TemporaryDirectory(prefix='novel-close-test-') as temporary:
            directory = Path(temporary)
            environment = dict(os.environ, NOVEL_PERFORMANCE_PROBE=temporary, NOVEL_CLOSE_PROBE='1')
            process = subprocess.Popen([str(executable)], cwd=ROOT, env=environment,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
            try:
                output, errors = process.communicate(timeout=30)
            except subprocess.TimeoutExpired:
                # This group contains only the synthetic test instance and its helper.
                os.killpg(process.pid, signal.SIGKILL)
                output, errors = process.communicate()
                self.fail('Isolated close test timed out: ' + errors.decode(errors='replace')[-2000:])
            self.assertEqual(process.returncode, 0, errors.decode(errors='replace')[-2000:])
            report = directory / 'closed-cleanly.json'
            self.assertTrue(report.exists(), 'Close did not pass through the native save guard.')
            self.assertEqual(json.loads(report.read_text()), {'closeIntercepted': True, 'saveDrained': True})
            with sqlite3.connect(f'file:{directory / "editor-spike.sqlite3"}?mode=ro', uri=True) as database:
                row = database.execute('SELECT payload FROM scene_revision JOIN scene_head ON scene_revision.id = scene_head.revision_id').fetchone()
                self.assertIsNotNone(row, 'Close occurred before the first commit.')
                scene = json.loads(row[0])
                expected = [('سلام محمد مِمِمِ ' + str(i) + ' ') * 20 for i in range(20)]
                expected[-1] += ' CLOSE سلام مِمِ中文👩🏽‍🚀'
                actual = [''.join(node.get('text', '') for node in block.get('content', []))
                          for block in scene['document']['content']]
                self.assertEqual(actual, expected)
                self.assertEqual(database.execute('PRAGMA integrity_check').fetchone(), ('ok',))
                self.assertEqual(database.execute('PRAGMA foreign_key_check').fetchall(), [])


if __name__ == '__main__':
    unittest.main()
