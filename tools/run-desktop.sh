#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if ! command -v cargo >/dev/null 2>&1 && [[ -x "${CARGO_HOME:-$HOME/.cargo}/bin/cargo" ]]; then
  export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
fi
# Native GTK interface and all application runtime workers are Rust.
mode="${1:-run}"
case "$mode" in
  --prepare-build)
    if [[ "${TAURI_ENV_DEBUG:-}" == "true" ]]; then exec "$0" --prepare; else exec "$0" --prepare-release; fi
    ;;
  --prepare|--prepare-release|--build|--package)
    profile="debug"
    task_cargo_args=(build --offline --locked -p novel-editor -p novel-companion -p novel-native-writer)
    if [[ "$mode" == "--prepare-release" || "$mode" == "--package" ]]; then profile="release";task_cargo_args+=(--release);fi
    cargo "${task_cargo_args[@]}" -j "${CARGO_BUILD_JOBS:-2}"
    task_target=$(rustc -vV | sed -n 's/^host: //p')
    task_build_root="${CARGO_TARGET_DIR:-target}"
    mkdir -p target/desktop-runtime
    for task_binary in novel-companion novel-editor;do
      cp "$task_build_root/$profile/$task_binary" "target/desktop-runtime/$task_binary-$task_target"
      chmod 755 "target/desktop-runtime/$task_binary-$task_target"
    done
    if [[ "$mode" == "--package" ]];then tools/package-rust.sh "$task_build_root";fi
    ;;
  --release|run)
    profile="debug"
    if [[ "$mode" == "--release" ]];then profile="release";fi
    task_build_root="${CARGO_TARGET_DIR:-target}"
    if [[ ! -x "$task_build_root/$profile/novel-workspace-rust" ]];then
      printf 'Build the Rust desktop first: tools/run-desktop.sh --build (or --package for release).\n' >&2
      exit 1
    fi
    exec "$task_build_root/$profile/novel-workspace-rust"
    ;;
  *)
    printf 'Usage: tools/run-desktop.sh [--prepare|--prepare-release|--build|--package|--release]\n' >&2
    exit 2
    ;;
esac
