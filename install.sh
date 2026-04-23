#!/bin/sh
set -e

REPO="AndresFritscheOgando/nuke"
BIN="nuke"
INSTALL_DIR="/usr/local/bin"
MAN_DIR="/usr/local/share/man/man1"

# Detect OS
OS=$(uname -s)
case "$OS" in
  Linux)  OS="linux"  ;;
  Darwin) OS="macos"  ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
  x86_64 | amd64)   ARCH="x86_64"  ;;
  arm64 | aarch64)  ARCH="aarch64" ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

# Resolve latest tag
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' \
  | cut -d'"' -f4)

if [ -z "$LATEST" ]; then
  echo "Could not resolve latest release. Check your internet connection." >&2
  exit 1
fi

URL="https://github.com/$REPO/releases/download/$LATEST/${BIN}-${OS}-${ARCH}"

echo "Downloading nuke $LATEST ($OS/$ARCH)..."
curl -fsSL "$URL" -o /tmp/nuke
chmod +x /tmp/nuke

# Install — try without sudo first
if [ -w "$INSTALL_DIR" ]; then
  mv /tmp/nuke "$INSTALL_DIR/$BIN"
else
  echo "Writing to $INSTALL_DIR requires sudo..."
  sudo mv /tmp/nuke "$INSTALL_DIR/$BIN"
fi

MAN_URL="https://github.com/$REPO/releases/download/$LATEST/nuke.1"
echo "Downloading man page..."
curl -fsSL "$MAN_URL" -o /tmp/nuke.1

if [ -w "$MAN_DIR" ]; then
  mkdir -p "$MAN_DIR"
  mv /tmp/nuke.1 "$MAN_DIR/nuke.1"
else
  sudo mkdir -p "$MAN_DIR"
  sudo mv /tmp/nuke.1 "$MAN_DIR/nuke.1"
fi

# Rebuild the man database if mandb is available
if command -v mandb > /dev/null 2>&1; then
  sudo mandb -q 2>/dev/null || true
fi

echo ""
echo "nuke $LATEST installed to $INSTALL_DIR/$BIN"
echo "Man page installed — run 'man nuke' to read it."
echo "Run 'nuke --help' to get started."
