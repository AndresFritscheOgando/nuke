# nuke

A Rust CLI tool that safely cleans directories by moving files to a timestamped
trash folder instead of permanently deleting them.

```
nuke -t ./build --force
```

---

## Install

### Via cargo (recommended)

```bash
cargo install --path .
```

### Via make

```bash
make install
```

Requires Rust 1.75+. The binary is installed to `~/.cargo/bin/nuke`.

---

## Build

```bash
# Debug
cargo build

# Optimized release binary
cargo build --release
```

---

## Usage

```
nuke [OPTIONS]

Options:
  -a, --all              Nuke files + subdirectories
      --files-only       Nuke files only (default behavior)
  -t, --target <PATH>    Target directory (default: current directory)
      --force            Skip confirmation prompt
  -h, --help             Print help
```

### Flag reference

| Flag | Description |
|------|-------------|
| `-a` / `--all` | Nuke files **and** subdirectories |
| `--files-only` | Nuke files only, preserve subdirectories (default) |
| `-t <PATH>` / `--target <PATH>` | Target directory (defaults to cwd) |
| `--force` | Skip the `[y/N]` confirmation prompt |

> **Note:** The shell version used `-fo` as a short flag. The Rust binary uses
> `--files-only` (clap only supports single-character short flags).

---

## Examples

```bash
# Nuke files in the current directory (prompts for confirmation)
nuke

# Nuke files only in ./dist, skip confirmation
nuke -t ./dist --force

# Nuke files + subdirectories in ./build
nuke -a -t ./build

# Nuke everything in cwd without a prompt
nuke -a --force
```

---

## Trash & recovery

Every nuke operation moves items to a unique timestamped folder:

```
~/.nuke-trash/YYYY-MM-DD_HH-MM-SS/
```

Each run gets its own folder — no collisions between operations.

**To recover files:**

```bash
mv ~/.nuke-trash/2026-04-22_10-00-00/myfile.txt ./
```

**To permanently clear all trash:**

```bash
rm -rf ~/.nuke-trash
```

---

## Safeties

- Refuses to nuke `/` (root filesystem)
- Refuses to nuke `$HOME`
- Validates that the target exists and is a directory
- Nothing is ever permanently deleted — items always go to trash first
- Confirmation prompt shown by default (bypass with `--force`)
