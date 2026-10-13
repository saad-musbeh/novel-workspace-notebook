"""Check the actual Rust release payload in disposable stores, without installation."""
import hashlib
import json
import os
from pathlib import Path
import selectors
import signal
import sqlite3
import subprocess
import tarfile
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / 'target/rust-bundle'
NAMES = ['novel-workspace-rust', 'novel-companion-rust', 'novel-editor-rust']


class Worker:
    def __init__(self, executable, directory, companion=False):
        directory.mkdir(parents=True, exist_ok=True)
        self.process = subprocess.Popen([str(executable), str(directory)], stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.companion = companion
        self.serial = 0

    def request(self, value):
        self.serial += 1
        if self.companion:
            value = dict(value, id=self.serial)
        self.process.stdin.write(json.dumps(value, ensure_ascii=False) + '\n')
        self.process.stdin.flush()
        with selectors.DefaultSelector() as selector:
            selector.register(self.process.stdout, selectors.EVENT_READ)
            assert selector.select(15), 'Worker reply timed out'
        reply = json.loads(self.process.stdout.readline())
        return reply

    def rpc(self, operation, args):
        reply = self.request({'op': operation, 'args': args})
        assert reply['ok'], reply
        return reply['value']

    def close(self):
        self.process.stdin.close()
        assert self.process.wait(timeout=10) == 0
        self.process.stdout.close()
        self.process.stderr.close()


def checksum(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def children(pid):
    path = Path(f'/proc/{pid}/task/{pid}/children')
    try:
        direct = [int(value) for value in path.read_text().split()]
    except FileNotFoundError:
        return []
    return direct + [child for parent in direct for child in children(parent)]


def main():
    archive = BUNDLE / 'novel-workspace-rust-0.4.0-linux-x86_64.tar.gz'
    rpm = BUNDLE / 'novel-workspace-rust-0.4.0-1.x86_64.rpm'
    checks = []
    with tempfile.TemporaryDirectory(prefix='novel-rust-release-') as folder:
        folder = Path(folder)
        payload = folder / 'payload'
        payload.mkdir()
        with tarfile.open(archive) as package:
            package.extractall(payload, filter='data')
        binaries = payload / 'usr/bin'
        for name, built in zip(NAMES, ['novel-workspace-rust', 'novel-companion', 'novel-editor']):
            assert checksum(binaries / name) == checksum(ROOT / 'target/release' / built)
            assert os.access(binaries / name, os.X_OK)
            linked = subprocess.check_output(['ldd', str(binaries / name)], text=True)
            assert 'not found' not in linked, linked
        checks.append('All three extracted executable hashes, modes and runtime libraries')
        assert sorted(p.name for p in binaries.iterdir()) == sorted(NAMES)
        assert not list(payload.rglob('*.py')) and not list(payload.rglob('*.js'))
        docs = payload / 'usr/share/doc/novel-workspace-rust'
        for path in [ROOT / 'README.md', ROOT / 'LICENSE', ROOT / 'NOTICE', *sorted((ROOT / 'docs').rglob('*'))]:
            if path.is_file():
                relative = path.relative_to(ROOT)
                assert checksum(docs / relative) == checksum(path), str(relative)
        subprocess.run(['desktop-file-validate', str(payload / 'usr/share/applications/novel-workspace-rust.desktop')], check=True)
        notices = json.loads((docs / 'docs/third-party/inventory.json').read_text())
        assert not notices['missingNoticeFiles']
        assert notices['cargoLockSha256'] == checksum(ROOT / 'Cargo.lock')
        checks.append('Documentation, notices, desktop launcher and Rust-only payload')

        rpm_payload = folder / 'rpm'
        rpm_payload.mkdir()
        source = subprocess.Popen(['rpm2cpio', str(rpm)], stdout=subprocess.PIPE)
        extracted = subprocess.run(['cpio', '-id', '--quiet'], stdin=source.stdout,
                                   cwd=rpm_payload, capture_output=True)
        source.stdout.close()
        assert extracted.returncode == 0 and source.wait(timeout=10) == 0
        for path in sorted((payload / 'usr').rglob('*')):
            if path.is_file():
                assert checksum(rpm_payload / path.relative_to(payload)) == checksum(path)
        subprocess.run(['rpm', '-K', '--nosignature', str(rpm)], check=True, capture_output=True)
        requirements = subprocess.check_output(['rpm', '-qp', '--requires', str(rpm)], text=True)
        assert 'python' not in requirements.lower() and 'webkit' not in requirements.lower()
        checks.append('RPM/archive payload equality, RPM digests and runtime requirements')

        editor = Worker(binaries / 'novel-editor-rust', folder / 'editor')
        document = {'type': 'doc', 'content': [{'type': 'paragraph', 'attrs': {'lang': 'ar', 'dir': 'rtl'},
                    'content': [{'type': 'text', 'text': 'مِ 中文 e\u0301 👩🏽‍🚀', 'marks': [{'type': 'bold'}]}]}]}
        scene = {'schemaVersion': 1, 'title': 'Synthetic release scene', 'document': document}
        assert editor.request({'type': 'open', 'scene': scene, 'width': 480})['frame']['document'] == document
        changed = editor.request({'type': 'text', 'text': 'Rust 繁體 عَ '})['frame']['document']
        assert changed != document
        assert editor.request({'type': 'command', 'command': 'undo'})['frame']['document'] == document
        assert editor.request({'type': 'command', 'command': 'redo'})['frame']['document'] == changed
        assert editor.request({'type': 'command', 'command': 'park'})['ok']
        assert editor.request({'type': 'resume'})['frame']['document'] == changed
        editor.close()
        checks.append('Actual release editor: exact rich Unicode, Undo/Redo and checkpoint resume')

        xdg = folder / 'xdg'
        data = xdg / 'data/local.novel.workspace.spike'
        companion = Worker(binaries / 'novel-companion-rust', data, True)
        assert companion.rpc('runtime_info', {})['storageSchema'] == 3
        project = companion.rpc('create_project', {'title': 'Disposable Rust package'})
        chapter = companion.rpc('create_chapter', {'projectId': project['id'], 'title': 'Synthetic chapter'})
        original = companion.rpc('create_scene', {'chapterId': chapter['id'], 'scene': scene})
        scene['document'] = changed
        saved = companion.rpc('save_scene', {'sceneId': original['id'], 'scene': scene, 'parent': original['revisionId']})
        refused = companion.request({'op': 'save_scene', 'args': {'sceneId': original['id'], 'scene': scene,
                                      'parent': original['revisionId']}})
        assert not refused['ok']
        assert companion.rpc('load_scene', {'sceneId': original['id']})['scene']['document'] == changed
        archive_text = companion.rpc('export_archive', {'projectId': project['id']})
        imported = companion.rpc('import_archive', {'archive': archive_text})
        assert imported['project']['id'] != project['id']
        assert imported['scenes'][0]['scene']['document'] == changed
        companion.close()
        checks.append('Actual release companion: schema, exact saves, stale refusal and archive/import')

        env = dict(os.environ, XDG_DATA_HOME=str(xdg / 'data'), XDG_CACHE_HOME=str(xdg / 'cache'),
                   XDG_CONFIG_HOME=str(xdg / 'config'))
        launched = subprocess.Popen([str(binaries / 'novel-workspace-rust'), '--no-focus'], env=env,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        try:
            deadline = time.monotonic() + 15
            names = set()
            while time.monotonic() < deadline:
                assert launched.poll() is None, 'Release desktop exited during startup'
                names = set()
                for pid in children(launched.pid):
                    try:
                        names.add(Path(os.readlink(f'/proc/{pid}/exe')).name)
                    except FileNotFoundError:
                        pass
                if {'novel-editor-rust', 'novel-companion-rust'} <= names:
                    break
                time.sleep(0.05)
            assert {'novel-editor-rust', 'novel-companion-rust'} <= names, names
        finally:
            os.killpg(launched.pid, signal.SIGTERM)
            launched.communicate(timeout=10)
        database = data / 'editor-spike.sqlite3'
        with sqlite3.connect(database) as connection:
            assert connection.execute('PRAGMA quick_check').fetchone() == ('ok',)
        checks.append('Actual release GTK main: isolated startup, scene-worker creation and store integrity')

    report = {'archive': str(archive), 'archiveBytes': archive.stat().st_size, 'archiveSha256': checksum(archive),
              'rpm': str(rpm), 'rpmBytes': rpm.stat().st_size, 'rpmSha256': checksum(rpm),
              'checks': checks, 'noticePackages': len(notices['packages']),
              'limitations': ['Release-main check covers startup; GTK workflow is a separate compiled native test.',
                              'No install/shortcut change, physical IME, accessibility or remote network qualification.',
                              'RPM digests verified without a publisher signature.']}
    (ROOT / 'work/rust-package-checks.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
