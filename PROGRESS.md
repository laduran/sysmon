# Progress Log

Append-only session log. Each entry records what was done, decisions made,
and what to work on next. A new Claude session should read this file (plus
`PRD.md` and `git log --oneline -10`) before touching any code.

---

## Session — 2026-03-29 (continued)

### Completed

- **Addressed second round of Devin code quality feedback:**
  - Cached physical device list in `DiskMonitor::new()` — `physical_devices()`
    was re-running `readdir("/sys/block")` + sort every second; devices are
    stable at runtime so enumerate once into `self.devices`
  - Extracted `push_history<T>()` generic helper in `ui.rs` — the
    borrow/len-check/pop/push block was copy-pasted three times in `main.rs`;
    single function now owns the invariant, preventing omission errors when
    adding future panels
  - Upgraded `sysinfo` 0.29 → 0.38 — old trait-based API (`CpuExt`,
    `SystemExt`) was removed in 0.30; on Arch (rolling release) a `cargo update`
    could have silently broken the build. Updated `cpu.rs` and `memory.rs` to
    current direct-method API; `refresh_cpu()` → `refresh_cpu_all()`,
    `System::MINIMUM_CPU_UPDATE_INTERVAL` replaced with `Duration::from_millis(200)`
  - Added `const BG: Color = Color::from_u8(43, 43, 43)` — last inline magic
    number in the draw closures; now consistent with the named colour constants

- **Bumped Cargo.toml to `edition = "2024"`** — Rust 2024 stabilised in 1.85
  (Feb 2025); no code changes required, builds cleanly on Rust 1.94.1

- **Established Ralph Loop workflow.** Created `PRD.md` and `PROGRESS.md`;
  introduced `Histories` struct to replace positionally-ambiguous 5-tuple
  return from `create_ui()`

### Decisions

- Agreed to upgrade sysinfo proactively rather than wait for breakage — Arch
  Linux's rolling release means `cargo update` could pull in a breaking version
  at any time.
- `Duration::from_millis(200)` is the documented minimum CPU refresh interval
  in sysinfo (was the value of the now-removed `MINIMUM_CPU_UPDATE_INTERVAL`).

### Known Issues / Notes

- The `glib = "0.18"` direct dependency and `gtk4 = "0.11"` (which pulls in
  `glib 0.22`) coexist with minor friction — `gtk4::glib::` prefix needed for
  GTK signal return types. Consider dropping the direct `glib` dependency and
  using `gtk4::glib` throughout in a future cleanup.

### Next Session Should

- Consider adding `cargo clippy --deny warnings` as a pre-commit hook or CI check.
- Begin scoping the network throughput panel (see PRD backlog).
- Explore Ralph Loop scripting: a shell script to end and restart Claude
  sessions automatically, with machine-verifiable completion criteria.

---

## Session — 2026-03-29

### Completed

- **Upgraded memory and disk panels to 2D graphs.** Replaced GTK `ProgressBar`
  widgets with Cairo `DrawingArea` graphs using the same mountain-graph style
  as the CPU panel. Extracted a reusable `make_graph()` helper to avoid
  duplicating the drawing logic.

- **Replaced disk usage % with read/write throughput.** Rewrote `disk.rs` to
  read `/proc/diskstats` and enumerate physical devices via `/sys/block/`.
  Throughput is computed by diffing sector counts each second (× 512 bytes).
  Unused disk slots are hidden when fewer than 3 physical drives are present.

- **Added device model names to disk labels.** NVMe model strings come from
  `/sys/class/nvme/<ctrl>/model`; SATA from `/sys/block/<dev>/device/model`.
  Cached after first read to avoid hitting the filesystem every second.

- **Fixed process not exiting on window close.** Added `connect_close_request`
  to explicitly call `app.quit()`, and a weak window reference in the timeout
  closure so it returns `ControlFlow::Break` once the window is gone.

- **Updated README.** Replaced the old "I vibe-coded this" opening with an
  accurate description of the project and an honest account of the AI-assisted
  development experiment. Added a screenshot captured after 18s of graph data.

- **Applied Devin's memory display fix.** Changed `free_memory()` to
  `available_memory()` so the "Avail" label reflects `MemAvailable` (includes
  reclaimable cache), not just raw `MemFree`. Updated label text accordingly.

- **Addressed Devin's code quality feedback:**
  - Added `Color` struct with `const fn from_u8()` and named per-panel
    constants (`CPU_FILL`, `MEM_LINE`, `DISK_READ_FILL`, etc.)
  - Replaced two identical 25-line Cairo trace blocks with a single loop
    over a `[(extractor, fill, line); 2]` array
  - Replaced the 5-tuple return from `create_ui()` with a `Histories` struct
    (`cpu`, `memory`, `disks` fields) to avoid positional ambiguity

- **Established Ralph Loop workflow.** Created `PRD.md` and this `PROGRESS.md`
  as the external state that new sessions read to orient themselves.

### Decisions

- Disk I/O reads directly from `/proc/diskstats` rather than using the
  `sysinfo` crate, which does not expose per-device throughput in v0.29.
- `MemAvailable` is the correct metric for "how much memory can I use" on
  Linux; `MemFree` is almost always lower and misleading.
- `Color::from_u8` uses `const fn` — integer-to-float casts in const context
  are stable since Rust 1.45; float division in const is stable since 1.82
  (Arch Linux ships a recent enough toolchain).

### Known Issues / Notes

- The `glib = "0.18"` direct dependency and `gtk4 = "0.11"` (which uses
  `glib 0.22` internally) coexist with minor friction — `gtk4::glib::` prefix
  needed for GTK signal return types. Consider dropping the direct `glib`
  dependency and using `gtk4::glib` exclusively in a future cleanup.

### Next Session Should

- Consider adding `cargo clippy --deny warnings` as a pre-commit hook or CI check.
- Begin scoping the network throughput panel (see PRD backlog).
