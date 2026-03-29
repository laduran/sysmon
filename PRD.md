# sysmon — Product Requirements Document

## Purpose

A lightweight, native Linux system monitor built in Rust with a GTK4 UI.
This project is also a deliberate experiment in AI-assisted software development,
used to explore effective human–AI collaboration workflows.

## Current Feature Set

### CPU Panel
- Usage percentage label (updates every second)
- Scrolling 2D mountain graph — 120 seconds of history, blue colour scheme

### Memory Panel
- Labels: Total, Used, Available, Swap (formatted in GB / MB)
- Scrolling 2D mountain graph — 120 seconds of history, green colour scheme
- Uses `MemAvailable` (not raw `MemFree`) so the figure reflects actually usable memory

### Disk Panel
- Enumerates physical block devices via `/sys/block/`, excluding loop/ram/dm/zram
- Per-device read and write throughput in bytes/sec, derived by diffing `/proc/diskstats`
- Dual-trace auto-scaling graph — teal = read, amber = write
- Device model names sourced from `/sys/class/nvme/<ctrl>/model` (NVMe) or
  `/sys/block/<dev>/device/model` (SATA); cached after first read
- Unused disk slots are hidden rather than shown as "Disk N: —"

## Quality Bar

Every session must leave the codebase in a state where:

1. `cargo build` completes without errors
2. `cargo clippy` produces no warnings
3. The application window close button terminates the process cleanly
4. No magic number colour values in draw code — use named `Color` constants
5. No duplicated Cairo draw blocks — shared logic must be factored out
6. Commit messages describe *why*, not just *what*

## Planned Features (Backlog)

- **Network throughput** — per-interface send/receive graphs (similar pattern to disk)
- **Per-core CPU breakdown** — individual core graphs or a heatmap strip
- **GPU monitoring** — utilisation and VRAM (scope TBD; depends on vendor API availability)

## Out of Scope (for now)

- Process list / top-style view
- Alerts or notifications
- Configuration file / settings UI
- Multi-monitor or tray icon support

## Tech Stack

- **Language:** Rust (edition 2024)
- **UI:** GTK4 via `gtk4-rs` 0.11
- **Drawing:** Cairo (via GTK4's draw functions)
- **System data:** `sysinfo` crate for CPU and memory; `/proc` and `/sys` directly for disk I/O
- **Platform:** Linux only

## Development Workflow

This project uses a variant of the Ralph Loop:
- State is persisted in Git history and `PROGRESS.md`
- Each Claude session begins by reading `PRD.md`, `PROGRESS.md`, and recent `git log`
- Each session ends with an updated `PROGRESS.md` entry committed to Git
- Completion criteria are machine-verifiable (`cargo build`, `cargo clippy`)
- Devin serves as a secondary code reviewer; feedback is addressed promptly
