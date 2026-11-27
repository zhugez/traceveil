#!/bin/sh
set -e
umask 077
fail() { printf 'Traceveil: %s\n' "$*" >&2; exit 2; }
prefix="$TRACEVEIL_HOME"
version=
archive=
sha=
force=0
uninstall=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --prefix|--version|--archive|--sha256)
      [ "$#" -ge 2 ] || fail "Missing option value."
      case "$1" in --prefix) prefix=$2;; --version) version=$2;; --archive) archive=$2;; --sha256) sha=$2;; esac
      shift 2;;
    --force) force=1; shift;;
    --uninstall) uninstall=1; shift;;
    --help) printf '%s\n' 'Install: sh install.sh --version VERSION --sha256 SHA256 [--archive FILE] [--prefix DIR] [--force]' 'Uninstall: sh install.sh --uninstall [--prefix DIR]' 'No public download exists until a release is authorized. Prefer a locally prepared archive.'; exit 0;;
    *) fail "Unknown option. Use --help.";;
  esac
done
if [ -z "$prefix" ]; then [ -n "$HOME" ] || fail "Set HOME or --prefix."; prefix="$HOME/.local"; fi
case "$prefix" in /*) ;; *) fail "--prefix must be absolute.";; esac
if printf '%s' "$prefix" | LC_ALL=C grep '[[:cntrl:]]' >/dev/null; then fail "Control characters are not permitted in the prefix."; fi
check_path() {
  item=$1
  while :; do
    [ ! -L "$item" ] || fail "The installation path must not contain symlinks."
    [ "$item" != / ] || break
    item=$(dirname "$item")
  done
}
check_path "$prefix/bin/traceveil"
check_path "$prefix/share/traceveil/install-receipt"
if command -v sha256sum >/dev/null 2>&1; then hash() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then hash() { shasum -a 256 "$1" | awk '{print $1}'; }
else fail "Install sha256sum or shasum before continuing."; fi
tmpbase="$TMPDIR"
[ -n "$tmpbase" ] || tmpbase=/tmp
stage=$(mktemp -d "$tmpbase/traceveil-install.XXXXXXXX") || fail "Cannot create a private temporary directory."
binary_tmp=
receipt_tmp=
cleanup() { [ -z "$binary_tmp" ] || rm -f "$binary_tmp"; [ -z "$receipt_tmp" ] || rm -f "$receipt_tmp"; [ -z "$stage" ] || rm -rf "$stage"; }
trap cleanup 0
trap 'exit 130' INT
trap 'exit 1' HUP TERM
binary="$prefix/bin/traceveil"
receipt="$prefix/share/traceveil/install-receipt"
owned=0
if [ -e "$binary" ] || [ -e "$receipt" ]; then
  [ -f "$binary" ] && [ -f "$receipt" ] || fail "Existing files have no complete Traceveil installation receipt. Preserve them."
  [ -z "$(find "$binary" -prune -size +134217728c -print)" ] || fail "Existing binary exceeds the size limit."
  head -c 1025 "$receipt" > "$stage/old-receipt"
  [ "$(wc -c < "$stage/old-receipt")" -le 1024 ] || fail "Installation receipt exceeds its size limit."
  stored=$(sed -n '2p' "$stage/old-receipt")
  printf '%s\n' "$stored" | grep -Eq '^[a-f0-9]{64}$' || fail "Invalid installation receipt."
  [ "$(hash "$binary")" = "$stored" ] || fail "Installed binary was modified. Preserve it and review manually."
  owned=1
fi
if [ "$uninstall" = 1 ]; then
  [ "$owned" = 1 ] || fail "No owned Traceveil installation at this prefix."
  rm "$binary" "$receipt"
  rmdir "$prefix/share/traceveil" 2>/dev/null || true
  printf '%s\n' 'Traceveil uninstalled. User configuration, agent skills and unrelated files were preserved.'
  exit 0
fi
printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$' || fail "Pass an explicit --version. No latest-version lookup occurs."
printf '%s\n' "$sha" | grep -Eq '^[0-9A-Fa-f]{64}$' || fail "Pass the archive SHA256 from a trusted source."
sha=$(printf '%s' "$sha" | tr '[:upper:]' '[:lower:]')
case "$(uname -s)" in Linux) system=linux;; Darwin) system=macos;; *) fail "Use the PowerShell installer on Windows.";; esac
case "$(uname -m)" in x86_64|amd64) arch=x86_64;; arm64|aarch64) arch=arm64;; *) fail "Unsupported CPU. Build from source.";; esac
if [ -z "$archive" ]; then
  command -v curl >/dev/null 2>&1 || fail "curl is required for an explicit HTTPS download."
  archive="$stage/archive.tar.gz"
  curl --fail --location --proto '=https' --proto-redir '=https' --tlsv1.2 --max-time 120 --max-filesize 104857600 --output "$archive" "https://github.com/zhugez/traceveil/releases/download/v$version/traceveil-$version-$system-$arch.tar.gz" || fail "Release download failed. Nothing was installed."
fi
case "$archive" in /*) ;; *) archive="$(pwd -P)/$archive";; esac
[ -f "$archive" ] && [ ! -L "$archive" ] || fail "Archive must be a regular file."
[ -z "$(find "$archive" -prune -size +104857600c -print)" ] || fail "Archive exceeds 100 MiB."
head -c 104857601 "$archive" > "$stage/verified.tar.gz"
[ "$(wc -c < "$stage/verified.tar.gz")" -le 104857600 ] || fail "Archive exceeds 100 MiB."
archive="$stage/verified.tar.gz"
[ "$(hash "$archive")" = "$sha" ] || fail "Archive checksum mismatch. Nothing was installed."
(ulimit -f 131072; tar -xOf "$archive" "traceveil-$version/traceveil" > "$stage/traceveil") || fail "Cannot read the expected native binary."
chmod 755 "$stage/traceveil"
[ "$("$stage/traceveil" --version)" = "traceveil $version" ] || fail "Archive version does not match."
[ "$owned" = 0 ] || [ "$force" = 1 ] || fail "A receipted installation exists. Use --force to replace it."
check_path "$binary"
check_path "$receipt"
mkdir -p "$prefix/bin" "$prefix/share/traceveil"
binary_tmp=$(mktemp "$prefix/bin/.traceveil-XXXXXXXX")
receipt_tmp=$(mktemp "$prefix/share/traceveil/.receipt-XXXXXXXX")
cp "$stage/traceveil" "$binary_tmp"
chmod 755 "$binary_tmp"
printf '%s\n%s\n' "$version" "$(hash "$stage/traceveil")" > "$receipt_tmp"
mv -f "$binary_tmp" "$binary"
binary_tmp=
mv -f "$receipt_tmp" "$receipt"
receipt_tmp=
printf 'Installed Traceveil %s. Add this directory to PATH if needed:\n  %s/bin\n' "$version" "$prefix"
printf '%s\n' 'Next: traceveil demo'
