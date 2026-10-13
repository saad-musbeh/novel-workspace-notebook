"""Exercise the native profile-aware DOCX example with owned synthetic data."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def cargo_binary():
    found = shutil.which("cargo")
    if found:
        return found
    fallback = Path.home() / ".cargo" / "bin" / "cargo"
    if fallback.is_file():
        return str(fallback)
    raise RuntimeError("Cargo was not found on PATH or in ~/.cargo/bin")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--libreoffice", action="store_true", help="Also roundtrip and render using a private LibreOffice profile")
    args = parser.parse_args()
    bundle = ROOT / "tests/fixtures/docx-project.json"
    profile = ROOT / "tests/fixtures/docx-export-profile.json"
    with tempfile.TemporaryDirectory(prefix="novel-export-style-test-") as directory:
        output = Path(directory) / "synthetic.docx"
        command = [cargo_binary(), "run", "-p", "novel-docx", "--example", "export_fixture", "--offline", "--locked", "--", str(bundle), str(output), "--profile", str(profile)]
        completed = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
        if completed.returncode:
            raise SystemExit("Synthetic exporter failed:\n" + completed.stdout + completed.stderr)
        if not output.is_file():
            raise SystemExit("Synthetic exporter did not create its DOCX")
        checker = ["python3", str(ROOT / "tools/check-docx-export.py"), str(output), str(bundle), "--profile", str(profile)]
        if args.libreoffice:
            checker.append("--libreoffice")
        checked = subprocess.run(checker, cwd=ROOT, capture_output=True, text=True, check=False)
        if checked.returncode:
            raise SystemExit("Independent DOCX check failed:\n" + checked.stdout + checked.stderr)
        print(checked.stdout.strip())


if __name__ == "__main__":
    main()
