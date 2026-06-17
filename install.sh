#!/bin/sh

set -eu

fail() {
  echo "Error: $*" >&2
  exit 1
}

need_cmd() {
  if ! command -v "$1" >/dev/null; then
    fail "\`$1\` is required to install \`appsignal-cli\`"
  fi
}

sha256_file() {
  file_path="$1"

  if command -v sha256sum >/dev/null; then
    sha256sum "$file_path" | cut -d' ' -f1
    return
  fi

  if command -v shasum >/dev/null; then
    shasum -a 256 "$file_path" | cut -d' ' -f1
    return
  fi

  if command -v openssl >/dev/null; then
    openssl dgst -sha256 "$file_path" | sed 's/^.*= //'
    return
  fi

  fail "\`sha256sum\`, \`shasum\`, or \`openssl\` is required to verify the download"
}

need_cmd curl
need_cmd cut
need_cmd grep
need_cmd mktemp
need_cmd sed
need_cmd tar
need_cmd tr

# This value is automatically updated during the release process;
# see `script/write_version`.
LAST_RELEASE="1.0.1"

VERSION="${APPSIGNAL_RUN_VERSION:-"$LAST_RELEASE"}"
INSTALL_FOLDER="${APPSIGNAL_RUN_INSTALL_FOLDER:-"/usr/local/bin"}"

# Expected values are "linux" or "darwin".
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"

if [ "$OS" = "linux" ]; then
  OS_FRIENDLY="Linux"
  VENDOR="unknown"
elif [ "$OS" = "darwin" ]; then
  OS_FRIENDLY="macOS"
  VENDOR="apple"
else
  echo "Error: Unsupported OS: $OS"
  exit 1
fi

# Expected values are "x86_64", "aarch64" or "arm64".
ARCH="$(uname -m)"
ARCH_FRIENDLY="$ARCH"

# Rename "arm64" to "aarch64" to match the naming convention used by the Rust
# toolchain target triples.
if [ "$ARCH" = "arm64" ]; then
  ARCH="aarch64"
fi

# Rename "aarch64" to "arm64" for the user-friendly architecture name.
if [ "$ARCH" = "aarch64" ]; then
  ARCH_FRIENDLY="arm64"
fi

if [ "$ARCH" != "x86_64" ] && [ "$ARCH" != "aarch64" ]; then
  echo "Error: Unsupported architecture: $ARCH"
  exit 1
fi

EXTRA=""
EXTRA_FRIENDLY=""

# Only check for musl with ldd on Linux; macOS doesn't have either.
if [ "$OS" = "linux" ]; then
  ldd_has_musl() {
    ldd --version 2>&1 | grep -q musl
  }

  if ldd_has_musl; then
    EXTRA="musl"
    EXTRA_FRIENDLY="musl"
  else
    # Do not add "gnu" to the friendly triple, as it is the assumed default.
    EXTRA="gnu"
  fi
fi

if [ -z "$EXTRA" ]; then
  TRIPLE="${ARCH}-${VENDOR}-${OS}"
else
  TRIPLE="${ARCH}-${VENDOR}-${OS}-${EXTRA}"
fi

if [ -z "$EXTRA_FRIENDLY" ]; then
  TRIPLE_FRIENDLY="${OS_FRIENDLY} (${ARCH_FRIENDLY})"
else
  TRIPLE_FRIENDLY="${OS_FRIENDLY} (${ARCH_FRIENDLY}, ${EXTRA_FRIENDLY})"
fi

if [ "$VERSION" = "latest" ]; then
  URL="https://github.com/appsignal/appsignal-cli/releases/latest/download/$TRIPLE.tar.gz"
  URL_FALLBACK="https://github.com/appsignal/homebrew-appsignal-cli/releases/latest/download/$TRIPLE.tar.gz"
  CHECKSUMS_URL="https://github.com/appsignal/appsignal-cli/releases/latest/download/SHA256SUMS"
  CHECKSUMS_FALLBACK_URL="https://raw.githubusercontent.com/appsignal/homebrew-appsignal-cli/main/checksums/v$LAST_RELEASE.txt"
  VERSION_FRIENDLY="latest version"
else
  URL="https://github.com/appsignal/appsignal-cli/releases/download/v$VERSION/$TRIPLE.tar.gz"
  URL_FALLBACK="https://github.com/appsignal/homebrew-appsignal-cli/releases/download/v$VERSION/$TRIPLE.tar.gz"
  CHECKSUMS_URL="https://github.com/appsignal/appsignal-cli/releases/download/v$VERSION/SHA256SUMS"
  CHECKSUMS_FALLBACK_URL="https://raw.githubusercontent.com/appsignal/homebrew-appsignal-cli/main/checksums/v$VERSION.txt"
  VERSION_FRIENDLY="version $VERSION"
fi

echo "Downloading $VERSION_FRIENDLY of the \`appsignal-cli\` binary for $TRIPLE_FRIENDLY..."

ARCHIVE_NAME="$TRIPLE.tar.gz"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}

trap cleanup EXIT INT TERM

if ! curl -fsSL "$CHECKSUMS_URL" -o "$TMP_DIR/SHA256SUMS" 2>/dev/null; then
  echo "Checksum manifest was not found on the appsignal-cli release; trying the legacy Homebrew tap manifest..."
  curl -fsSL "$CHECKSUMS_FALLBACK_URL" -o "$TMP_DIR/SHA256SUMS"
  URL="$URL_FALLBACK"
fi

EXPECTED_SHA="$(grep "  $ARCHIVE_NAME$" "$TMP_DIR/SHA256SUMS" | cut -d' ' -f1)"

if [ -z "$EXPECTED_SHA" ]; then
  fail "Could not find checksum for $ARCHIVE_NAME in the downloaded manifest"
fi

curl --progress-bar -fSL "$URL" -o "$TMP_DIR/$ARCHIVE_NAME"
ACTUAL_SHA="$(sha256_file "$TMP_DIR/$ARCHIVE_NAME")"

if [ "$ACTUAL_SHA" != "$EXPECTED_SHA" ]; then
  fail "Checksum verification failed for $ARCHIVE_NAME"
fi

echo "Verified SHA256 checksum for $ARCHIVE_NAME."

tar -C "$INSTALL_FOLDER" -xzf "$TMP_DIR/$ARCHIVE_NAME"

echo "Done! Installed \`appsignal-cli\` binary at \`$INSTALL_FOLDER/appsignal-cli\`."
