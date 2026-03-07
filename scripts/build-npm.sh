#!/usr/bin/env bash
set -euo pipefail

# Build pick binaries for all platforms and stage them into npm packages.
# Usage: ./scripts/build-npm.sh [--local-only]

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NPM_DIR="$ROOT/npm"
VERSION=$(grep '^version' "$ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)"/\1/')

echo "Building pick v${VERSION}"

# Platform -> Rust target mapping (using parallel arrays for bash 3.x compat)
PLATFORMS="darwin-arm64 darwin-x64 linux-x64 linux-arm64 win32-x64"
get_target() {
  case "$1" in
    darwin-arm64) echo "aarch64-apple-darwin" ;;
    darwin-x64)   echo "x86_64-apple-darwin" ;;
    linux-x64)    echo "x86_64-unknown-linux-gnu" ;;
    linux-arm64)  echo "aarch64-unknown-linux-gnu" ;;
    win32-x64)    echo "x86_64-pc-windows-gnu" ;;
  esac
}

# Update version in all package.json files
for pkg_dir in "$NPM_DIR"/pick-cli*/; do
  if [ -f "$pkg_dir/package.json" ]; then
    sed -i.bak "s/\"version\": \".*\"/\"version\": \"${VERSION}\"/" "$pkg_dir/package.json"
    rm -f "$pkg_dir/package.json.bak"
  fi
done

# Also update optionalDependencies versions in main package
sed -i.bak "s/\"@aryanbhosale\/pick-\([^\"]*\)\": \"[^\"]*\"/\"@aryanbhosale\/pick-\1\": \"${VERSION}\"/g" "$NPM_DIR/pick-cli/package.json"
rm -f "$NPM_DIR/pick-cli/package.json.bak"

build_platform() {
  local platform="$1"
  local target
  target=$(get_target "$platform")
  local bin_name="pick"

  case "$platform" in
    win32-*) bin_name="pick.exe" ;;
  esac

  echo "  Compiling for $target..."

  if cargo build --release --target "$target" 2>/dev/null; then
    true
  else
    echo "  Native cargo failed, trying cross..."
    cross build --release --target "$target"
  fi

  local src="$ROOT/target/$target/release/$bin_name"
  local dst="$NPM_DIR/pick-cli-$platform/bin/$bin_name"

  cp "$src" "$dst"
  chmod +x "$dst"
  echo "  Staged: $dst ($(du -h "$dst" | cut -f1))"
}

if [ "${1:-}" = "--local-only" ]; then
  echo "Building for local platform only..."
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)  LOCAL_PLATFORM="darwin-arm64" ;;
    Darwin-x86_64) LOCAL_PLATFORM="darwin-x64" ;;
    Linux-x86_64)  LOCAL_PLATFORM="linux-x64" ;;
    Linux-aarch64) LOCAL_PLATFORM="linux-arm64" ;;
    *) echo "Unknown local platform: $(uname -s)-$(uname -m)"; exit 1 ;;
  esac
  build_platform "$LOCAL_PLATFORM"
else
  echo "Cross-compiling for all platforms..."
  for platform in $PLATFORMS; do
    build_platform "$platform"
  done
fi

echo ""
echo "Done. To publish:"
echo "  cd npm/pick-cli-darwin-arm64 && npm publish --access public"
echo "  cd npm/pick-cli-darwin-x64   && npm publish --access public"
echo "  cd npm/pick-cli-linux-x64    && npm publish --access public"
echo "  cd npm/pick-cli-linux-arm64  && npm publish --access public"
echo "  cd npm/pick-cli-win32-x64    && npm publish --access public"
echo "  cd npm/pick-cli              && npm publish --access public"
echo ""
echo "IMPORTANT: Publish platform packages BEFORE the main package."
