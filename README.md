# sysmon

A lightweight system monitor for Linux, written in Rust with a GTK4 UI.

![System Monitor screenshot](screenshots/sysmon_window.png)

## What it monitors

- **CPU** — usage percentage with a scrolling history graph
- **Memory** — total, used, free, and swap with a scrolling history graph
- **Disks** — real-time read/write throughput per physical drive, with model names pulled from the kernel (e.g. `nvme0n1 · WD_BLACK SN770 1TB`)
- **GPU** — engine utilisation and VRAM usage; supports Intel Arc (xe driver via sysfs) and NVIDIA (via `nvidia-smi`)

All graphs show 120 seconds of history and auto-scale to the data.

## About this project

This is an intentional experiment in using AI coding assistants to develop a non-trivial program in a language I have only basic knowledge of. The goal was to explore how far AI agents can carry a real project: architecture decisions, GTK4/Cairo rendering, Linux kernel interfaces (`/proc/diskstats`, `/sys/block`), and bug fixes, with a human providing direction and feedback.

- **[Claude](https://claude.ai)** — primary coding assistant for implementation and architecture
- **[Devin.AI](https://devin.ai)** — automated code review and bug finding on pull requests

The result is a functional, reasonably well-structured Rust application. The experiment is ongoing.

Inspired by [Mission Center](https://missioncenter.io/).

## Building

```sh
cargo build --release
./target/release/system-monitor
```

Requires GTK4 development libraries (`gtk4` / `libgtk-4-dev`).

## Planned

- Network throughput graphs
- Per-core CPU breakdown
