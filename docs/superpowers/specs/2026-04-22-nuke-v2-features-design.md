# nuke v2 — Feature Design Spec

**Date:** 2026-04-22
**Status:** Approved

---

## Goal

Evolve `nuke` from a dev-focused directory cleaner into a general-purpose `rm` replacement. Three capability gaps to close: trash management (list/restore/empty), selective nuking (pattern/exclude filtering), and multi-target support. Dry-run added for safety.

---

## CLI Surface

### Default command (extended)

```
nuke [OPTIONS]

  -a, --all                    Nuke files + subdirectories
      --files-only             Nuke files only (default)
  -t, --target <PATH>          Target directory (repeatable; default: cwd)
      --force                  Skip confirmation prompt
      --dry-run                Show what would be moved; do nothing
      --pattern <GLOB>         Only nuke items matching glob (e.g. "*.log")
      --exclude <GLOB>         Skip items matching glob (repeatable)
```

`-t` becomes `Vec<PathBuf>`. Multiple targets share one trash session per invocation.

### New subcommands

```
nuke list               Tabular view of all trash sessions
nuke restore            Interactive picker: choose session → choose destination
nuke empty [--all]      Pick session(s) to permanently delete; --all skips picker
```

---

## Architecture

```
src/
  main.rs
  cli.rs            — clap enum: Nuke(NukeArgs) | List | Restore | Empty(EmptyArgs)
  nuke.rs           — core pipeline: validate → collect → preview → confirm → trash
  trash.rs          — Trash struct + list_sessions(), restore_session(), empty_session(), empty_all()
  commands/
    list.rs         — formats session table (timestamp, item count, size)
    restore.rs      — dialoguer picker: session → destination → trash.restore_session()
    empty.rs        — dialoguer multi-picker or --all → trash.empty_*()
```

### New dependencies

| Crate | Purpose |
|-------|---------|
| `glob` | Pattern and exclude matching |
| `dialoguer` | Interactive pickers for restore and empty |
| `bytesize` | Human-readable sizes in `nuke list` |

---

## Data Flow

### Nuke pipeline (updated)

1. Validate each target — existing guards (rejects `/`, `$HOME`) + warn if target is already empty
2. Collect items per target; apply `--pattern` include filter, then `--exclude` filter
3. Preview — per-target item counts; if `--dry-run`, print full item list and exit cleanly (exit 0)
4. Single confirm prompt covers all targets combined (skipped with `--force`)
5. Items moved into one shared trash session, namespaced by source dir name to avoid collisions:
   `~/.nuke-trash/<timestamp>/<target-dir-name>/<item>`

### Restore flow

1. Scan `~/.nuke-trash/` — exit with clear message if empty: `"No sessions found."`
2. `dialoguer::Select` — sessions sorted newest-first, each line shows: timestamp, item count, total size
3. Prompt for destination path (default: cwd); validate it exists and is a directory
4. Move items from session dir to destination
5. On name collision at destination — **abort with error listing conflicting names**; do not silently overwrite
6. Remove now-empty session dir on success

### Empty flow

- `nuke empty` — `dialoguer::MultiSelect` to pick one or more sessions; confirm once before deletion
- `nuke empty --all` — skip picker; confirm once: `"Permanently delete N sessions? [y/N]"`; remove all

### Error handling

- All errors via `anyhow` with context strings, consistent with current code
- No rollback on partial restore failure — items already moved to destination stay there (safer than attempting reverse moves)
- Partial nuke failure (one target fails mid-run) — report error, continue remaining targets, summarize at end

---

## Key Invariants

- Nothing is ever permanently deleted by the default `nuke` command — always goes to trash first
- `--dry-run` never touches disk (no trash dir created, no files moved)
- `--pattern` and `--exclude` both use glob syntax; exclude takes precedence over pattern
- Multi-target nukes produce one trash session total, not one per target
- Restore aborts on conflict rather than overwriting — user must resolve manually

---

## Out of Scope

- Age-based auto-purge of old trash sessions
- Named/labeled sessions (`--label`)
- Config file (`~/.config/nuke/config.toml`)
- `nuke restore` partial item selection within a session (all-or-nothing per session)
