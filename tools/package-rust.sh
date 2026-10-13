#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
task_build_root="${1:-target}"
task_absolute_target=$(realpath "$task_build_root")
task_stage="$task_absolute_target/rust-package"
task_lock_hash=$(sha256sum Cargo.lock | cut -d ' ' -f 1)
task_notice_hash=$(sed -n 's/.*"cargoLockSha256": "\([a-f0-9]*\)".*/\1/p' docs/third-party/inventory.json)
if [[ "$task_lock_hash" != "$task_notice_hash" ]];then
  printf 'Dependency notices are stale. Regenerate with tools/dependency-notices.py before packaging.\n' >&2
  exit 1
fi
mkdir -p "$task_stage/usr/bin" "$task_stage/usr/share/applications" "$task_stage/usr/share/icons/hicolor/128x128/apps" "$task_stage/usr/share/doc/novel-workspace-rust" "$task_absolute_target/rust-bundle"
install -m 755 "$task_absolute_target/release/novel-workspace-rust" "$task_stage/usr/bin/novel-workspace-rust"
install -m 755 "$task_absolute_target/release/novel-companion" "$task_stage/usr/bin/novel-companion-rust"
install -m 755 "$task_absolute_target/release/novel-editor" "$task_stage/usr/bin/novel-editor-rust"
install -m 644 apps/native-writer/icons/notebook.png "$task_stage/usr/share/icons/hicolor/128x128/apps/novel-workspace-rust.png"
install -m 644 README.md LICENSE NOTICE "$task_stage/usr/share/doc/novel-workspace-rust/"
cp -a docs "$task_stage/usr/share/doc/novel-workspace-rust/"
cat > "$task_stage/usr/share/applications/novel-workspace-rust.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Novel Workspace — Notebook
Comment=Local novel writing and planning
Exec=novel-workspace-rust
Icon=novel-workspace-rust
Terminal=false
Categories=Office;WordProcessor;
Keywords=novel;writing;Arabic;
DESKTOP
cat > "$task_stage/run.sh" <<'LAUNCH'
#!/usr/bin/env bash
set -euo pipefail
exec "$(dirname "$(realpath "$0")")/usr/bin/novel-workspace-rust" "$@"
LAUNCH
chmod 755 "$task_stage/run.sh"
tar -czf "$task_absolute_target/rust-bundle/novel-workspace-rust-0.4.0-linux-x86_64.tar.gz" -C "$task_stage" .
if command -v rpmbuild >/dev/null 2>&1;then
  mkdir -p "$task_absolute_target/rust-rpm/"{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
  cat > "$task_absolute_target/rust-rpm/SPECS/novel-workspace-rust.spec" <<SPEC
Name: novel-workspace-rust
Version: 0.4.0
Release: 1
Summary: Native Rust novel writing workspace
License: Apache-2.0 AND MIT AND BSD-3-Clause AND ISC AND Unicode-3.0 AND Zlib AND MPL-2.0
BuildArch: x86_64
Requires: gtk3, pango >= 1.50, cairo, fontconfig
Recommends: libsecret
%description
Native GTK novel writing interface, Unicode editor and local storage, implemented in Rust.
%install
mkdir -p %{buildroot}
cp -a "$task_stage/usr" %{buildroot}/
%files
/usr/bin/novel-workspace-rust
/usr/bin/novel-companion-rust
/usr/bin/novel-editor-rust
/usr/share/applications/novel-workspace-rust.desktop
/usr/share/icons/hicolor/128x128/apps/novel-workspace-rust.png
/usr/share/doc/novel-workspace-rust
%changelog
* Tue Oct 13 2026 Local build - 0.4.0-1
- Add recovered scraps, reading reactions, passage reminders and return bookmarks.
SPEC
  rpmbuild --define "_topdir $task_absolute_target/rust-rpm" --define '_build_id_links none' --define '__os_install_post %{nil}' -bb "$task_absolute_target/rust-rpm/SPECS/novel-workspace-rust.spec"
  cp "$task_absolute_target/rust-rpm/RPMS/x86_64/novel-workspace-rust-0.4.0-1.x86_64.rpm" "$task_absolute_target/rust-bundle/"
fi
