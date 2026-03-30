use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Snapshot of GPU metrics for one poll cycle.
pub struct GpuStats {
    /// GPU engine utilisation, 0.0–1.0.
    pub util_frac: f64,
    /// Total VRAM allocated across all DRM clients, in bytes.
    pub vram_used_bytes: f64,
}

pub struct GpuMonitor {
    /// Cumulative idle-residency counter (xe driver, GT0).
    idle_path: PathBuf,
    /// DRM render node (e.g. "renderD128") used to match fdinfo entries.
    render_device: String,
    /// Last sampled idle_residency_ms value.
    last_idle_ms: u64,
    /// Skip utilisation calculation on the very first sample.
    first: bool,
}

impl GpuMonitor {
    /// Detect an Intel Arc / xe-driver GPU and return a monitor, or `None` if
    /// no supported GPU is found.
    pub fn new() -> Option<Self> {
        for card_index in 0..8u32 {
            let idle_path = PathBuf::from(format!(
                "/sys/class/drm/card{}/device/tile0/gt0/gtidle/idle_residency_ms",
                card_index
            ));
            if !idle_path.exists() {
                continue;
            }
            let render_device =
                find_render_device(card_index).unwrap_or_else(|| "renderD128".to_string());
            let last_idle_ms = read_u64(&idle_path).unwrap_or(0);
            return Some(GpuMonitor {
                idle_path,
                render_device,
                last_idle_ms,
                first: true,
            });
        }
        None
    }

    /// Sample GPU utilisation and VRAM usage.
    /// `elapsed_ms` is the wall-clock time in milliseconds since the last call.
    /// Returns `None` if the underlying sysfs counters become unreadable.
    pub fn update(&mut self, elapsed_ms: u64) -> Option<GpuStats> {
        let current_idle_ms = read_u64(&self.idle_path)?;

        let util_frac = if self.first || elapsed_ms == 0 {
            self.first = false;
            0.0
        } else {
            let idle_delta = current_idle_ms.saturating_sub(self.last_idle_ms);
            (1.0 - idle_delta as f64 / elapsed_ms as f64).clamp(0.0, 1.0)
        };
        self.last_idle_ms = current_idle_ms;

        let vram_used_bytes = scan_vram_used(&self.render_device);

        Some(GpuStats {
            util_frac,
            vram_used_bytes,
        })
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn read_u64(path: &Path) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// Find the renderD* node name for a given DRM card index by inspecting the
/// card's `device/drm/` sysfs directory.
fn find_render_device(card_index: u32) -> Option<String> {
    let drm_path = format!("/sys/class/drm/card{}/device/drm", card_index);
    for entry in fs::read_dir(&drm_path).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("render") {
            return Some(name);
        }
    }
    None
}

/// Sum VRAM allocated by all unique DRM clients that have `render_device` open,
/// by scanning every process's fdinfo entries.
///
/// The xe driver reports per-client VRAM under `drm-total-vram0`.  Each DRM
/// client can hold multiple file descriptors that all show the same allocation,
/// so we deduplicate by `drm-client-id` (first occurrence wins).
fn scan_vram_used(render_device: &str) -> f64 {
    let target = format!("/dev/dri/{}", render_device);
    let mut seen: HashMap<u64, u64> = HashMap::new(); // client-id → bytes

    let Ok(procs) = fs::read_dir("/proc") else {
        return 0.0;
    };

    for proc_entry in procs.flatten() {
        let pid = proc_entry.file_name();
        let pid_str = pid.to_string_lossy();
        if !pid_str.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }

        let fd_dir = format!("/proc/{}/fd", pid_str);
        let Ok(fds) = fs::read_dir(&fd_dir) else {
            continue;
        };

        for fd_entry in fds.flatten() {
            // Only bother reading fdinfo if the fd actually points at our GPU.
            let Ok(link) = fs::read_link(fd_entry.path()) else {
                continue;
            };
            if link.to_string_lossy() != target {
                continue;
            }

            let fd_name = fd_entry.file_name();
            let fdinfo_path =
                format!("/proc/{}/fdinfo/{}", pid_str, fd_name.to_string_lossy());
            let Ok(content) = fs::read_to_string(&fdinfo_path) else {
                continue;
            };

            let mut client_id: Option<u64> = None;
            let mut vram_bytes: u64 = 0;

            for line in content.lines() {
                if let Some(v) = line.strip_prefix("drm-client-id:\t") {
                    client_id = v.trim().parse().ok();
                } else if let Some(v) = line.strip_prefix("drm-total-vram0:\t") {
                    vram_bytes = parse_drm_memory(v);
                }
            }

            if let Some(id) = client_id {
                seen.entry(id).or_insert(vram_bytes);
            }
        }
    }

    seen.values().sum::<u64>() as f64
}

/// Parse a DRM memory field value such as "1234 KiB", "5 MiB", "2 GiB", or
/// a bare byte count.
fn parse_drm_memory(s: &str) -> u64 {
    let mut parts = s.split_whitespace();
    let value: u64 = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    match parts.next().unwrap_or("") {
        "GiB" => value * 1024 * 1024 * 1024,
        "MiB" => value * 1024 * 1024,
        "KiB" => value * 1024,
        _ => value,
    }
}
