#!/bin/sh

set -eu

if ! command -v curl >/dev/null; then
  echo "Error: \`curl\` is required to download the \`appsignal-cli\` binary"
  exit 1
fi

if ! command -v tar >/dev/null; then
  echo "Error: \`tar\` is required to extract the \`appsignal-cli\` binary"
  exit 1
fi

# This value is automatically updated during the release process;
# see `script/write_version`.
LAST_RELEASE="0.1.0"

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
  VERSION_FRIENDLY="latest version"
else
  URL="https://github.com/appsignal/appsignal-cli/releases/download/v$VERSION/$TRIPLE.tar.gz"
  VERSION_FRIENDLY="version $VERSION"
fi

echo "Downloading $VERSION_FRIENDLY of the \`appsignal-cli\` binary for $TRIPLE_FRIENDLY..."

curl --progress-bar -SL "$URL" | tar -C "$INSTALL_FOLDER" -xz

# Remove the old `appsignal-run` binary or symlink if it exists.
rm -f "$INSTALL_FOLDER/appsignal-run" || true
# Create a new `appsignal-run` symlink to the `appsignal-cli` binary.
# This is done to maintain backwards compatibility with the previous name.
ln -s "$INSTALL_FOLDER/appsignal-cli" "$INSTALL_FOLDER/appsignal-run"

echo "Done! Installed \`appsignal-cli\` binary at \`$INSTALL_FOLDER/appsignal-cli\`."
