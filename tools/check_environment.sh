#!/usr/bin/env bash
# Read-only Linux inventory for the Offline Novel Workspace planning project.
# Prints to stdout only. No sudo, installations, network calls, or configuration changes.
# Optional tools are detected, never installed. Tool shims are not executed.
set -u
set -o pipefail
export LC_ALL=C

section() { printf '\n## %s\n' "$1"; }
has() { command -v "$1" >/dev/null 2>&1; }

if ! has uname || [[ "$(uname -s)" != "Linux" ]]; then
  printf 'This report is intended for a Linux development machine.\n' >&2
  exit 2
fi

printf '# Local development environment report\n'
printf 'This describes the machine executing the script, not a remotely inspected PC.\n'
printf 'Review the output before sharing it. No account names, IP addresses, or serial numbers are requested.\n'

section 'Operating system and session'
if [[ -r /etc/os-release ]]; then
  awk -F= '$1 == "PRETTY_NAME" || $1 == "VERSION_ID" || $1 == "VARIANT_ID" {print}' /etc/os-release
else
  printf 'OS release file: unavailable\n'
fi
printf 'Architecture: %s\n' "$(uname -m)"
printf 'Kernel: %s\n' "$(uname -r)"
printf 'Session type: %s\n' "${XDG_SESSION_TYPE:-not reported}"
printf 'Desktop: %s\n' "${XDG_CURRENT_DESKTOP:-not reported}"
if [[ -e /run/ostree-booted ]]; then
  printf 'OSTree boot marker: present (check Atomic/immutable setup instructions)\n'
else
  printf 'OSTree boot marker: absent (not conclusive on every distribution)\n'
fi
if has getenforce; then
  printf 'SELinux mode: '
  getenforce 2>/dev/null || printf 'unavailable\n'
fi

section 'CPU'
if has lscpu; then
  lscpu 2>/dev/null | awk -F: '
    $1 ~ /^(Architecture|CPU\(s\)|Vendor ID|Model name|Thread\(s\) per core|Core\(s\) per socket|Socket\(s\))$/ {print}'
else
  printf 'lscpu: not installed; skipping CPU model details\n'
fi

section 'Memory'
if [[ -r /proc/meminfo ]]; then
  awk '
    /^(MemTotal|MemAvailable|SwapTotal):/ {
      label=$1; sub(/:$/, "", label);
      printf "%s: %.2f GiB\n", label, $2/1048576
    }' /proc/meminfo
else
  printf 'Memory information: unavailable\n'
fi

section 'Graphics devices'
if has lspci; then
  # Standard PCI device descriptions: no serial numbers or process inspection.
  gpu_lines="$(lspci 2>/dev/null | grep -Ei 'VGA compatible controller|3D controller|Display controller' || true)"
  if [[ -n "$gpu_lines" ]]; then
    printf '%s\n' "$gpu_lines"
  else
    printf 'No GPU description reported by lspci\n'
  fi
else
  printf 'lspci: not installed; skipping PCI graphics details\n'
fi
if has nvidia-smi; then
  printf '\nNVIDIA name, total VRAM, driver version:\n'
  nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader 2>/dev/null \
    || printf 'NVIDIA query unavailable (driver/tool may not be usable)\n'
fi
shopt -s nullglob
vram_files=(/sys/class/drm/card[0-9]*/device/mem_info_vram_total)
if ((${#vram_files[@]} > 0)); then
  index=0
  for file in "${vram_files[@]}"; do
    [[ -r "$file" ]] || continue
    index=$((index + 1))
    awk -v i="$index" '{printf "DRM reported dedicated memory entry %d: %.2f GiB\n", i, $1/1073741824}' "$file" 2>/dev/null
  done
fi
printf 'Missing VRAM figures do not establish that a GPU is unsupported. Shared memory is not dedicated VRAM.\n'

section 'Available storage'
if has df; then
  # Deliberately print only available space, not paths or volume identifiers.
  df -h --output=avail "${HOME:-/}" 2>/dev/null \
    || printf 'Available-space query unavailable\n'
fi

section 'Development tool presence'
printf 'Presence does not establish version compatibility. Shims are not executed, avoiding implicit toolchain downloads.\n'
for tool in git rustup rustc cargo node npm pnpm java javac adb cmake clang gcc pkg-config sqlite3 tailscale; do
  if has "$tool"; then
    printf '%-12s detected\n' "$tool"
  else
    printf '%-12s not found in PATH\n' "$tool"
  fi
done

section 'Desktop editing runtime'
printf 'The default desktop interface, text engine and local workers are Rust.\n'
if has pkg-config; then
  for library in gtk+-3.0 pango pangocairo cairo fontconfig; do
    if pkg-config --exists "$library"; then
      printf '%s: %s\n' "$library" "$(pkg-config --modversion "$library")"
    else
      printf '%s: development metadata not found\n' "$library"
    fi
  done
  pkg-config --atleast-version=1.50 pango || printf 'Pango >= 1.50 is required.\n'
else
  printf 'pkg-config not found; native development versions were not inspected.\n'
fi
printf 'Python, Node and WebKit are used only by retained development/reference workflows.\n'

section 'Android SDK configuration'
for variable in JAVA_HOME ANDROID_HOME ANDROID_SDK_ROOT NDK_HOME; do
  if [[ -n "${!variable:-}" ]]; then
    printf '%s: set (path omitted)\n' "$variable"
  else
    printf '%s: not set\n' "$variable"
  fi
done
if [[ -n "${HOME:-}" && -d "$HOME/Android/Sdk" ]]; then
  printf 'Default per-user Android SDK directory: present\n'
else
  printf 'Default per-user Android SDK directory: not detected\n'
fi

section 'Remaining manual verification'
printf '%s\n' \
  'Record the actual Android version, keyboard/input methods, and Android System WebView version on the phone.' \
  'Verify installed tool versions and SDK/NDK compatibility during setup, not from command presence alone.' \
  'Model performance, graphics backend compatibility, disk encryption, and application behavior remain untested.' \
  'No phone connection, manuscript access, model loading, or remote service query was performed.'
