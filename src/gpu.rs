use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Snapshot of GPU metrics for one poll cycle.
pub struct GpuStats {
    /// GPU engine utilisation, 0.0–1.0.
    pub util_frac: f64,
    /// Total VRAM allocated across all DRM clients, in bytes.
    pub vram_used_bytes: f64,
}

enum Backend {
    /// Intel Arc / xe driver — uses sysfs idle-residency counters.
    IntelXe {
        idle_path: PathBuf,
        render_device: String,
        last_idle_ms: u64,
        first: bool,
    },
    /// NVIDIA — a background thread polls `nvidia-smi`; main thread reads cache.
    Nvidia {
        /// Latest (util_frac, vram_used_bytes) from the background thread.
        cache: Arc<Mutex<Option<(f64, f64)>>>,
    },
}

pub struct GpuMonitor {
    backend: Backend,
}

impl GpuMonitor {
    /// Detect a supported GPU and return a monitor, or `None` if none found.
    /// Tries Intel Arc (xe driver) first, then NVIDIA via `nvidia-smi`.
    pub fn new() -> Option<Self> {
        // ── Intel Arc / xe driver ────────────────────────────────────────────
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
                backend: Backend::IntelXe {
                    idle_path,
                    render_device,
                    last_idle_ms,
                    first: true,
                },
            });
        }

        // ── NVIDIA ───────────────────────────────────────────────────────────
        if nvidia_smi_available() {
            let cache: Arc<Mutex<Option<(f64, f64)>>> = Arc::new(Mutex::new(None));
            let cache_bg = Arc::clone(&cache);
            thread::spawn(move || loop {
                if let Some(stats) = query_nvidia_smi()
                    && let Ok(mut g) = cache_bg.lock()
                {
                    *g = Some((stats.util_frac, stats.vram_used_bytes));
                }
                thread::sleep(Duration::from_millis(1000));
            });
            return Some(GpuMonitor {
                backend: Backend::Nvidia { cache },
            });
        }

        None
    }

    /// Sample GPU utilisation and VRAM usage.
    /// `elapsed_ms` is the wall-clock time since the last call (used by Intel backend).
    /// Returns `None` if the underlying counters become unreadable.
    pub fn update(&mut self, elapsed_ms: u64) -> Option<GpuStats> {
        match &mut self.backend {
            Backend::IntelXe {
                idle_path,
                render_device,
                last_idle_ms,
                first,
            } => {
                let current_idle_ms = read_u64(idle_path)?;

                let util_frac = if *first || elapsed_ms == 0 {
                    *first = false;
                    0.0
                } else {
                    let idle_delta = current_idle_ms.saturating_sub(*last_idle_ms);
                    (1.0 - idle_delta as f64 / elapsed_ms as f64).clamp(0.0, 1.0)
                };
                *last_idle_ms = current_idle_ms;

                let vram_used_bytes = scan_vram_used(render_device);

                Some(GpuStats {
                    util_frac,
                    vram_used_bytes,
                })
            }

            Backend::Nvidia { cache } => {
                let guard = cache.lock().ok()?;
                let (util_frac, vram_used_bytes) = (*guard)?;
                Some(GpuStats { util_frac, vram_used_bytes })
            }
        }
    }
}

// ── NVIDIA helpers ────────────────────────────────────────────────────────────

/// Return true if `nvidia-smi` is present and responds successfully.
fn nvidia_smi_available() -> bool {
    Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Query `nvidia-smi` for utilisation and VRAM of GPU index 0.
///
/// Runs: `nvidia-smi --query-gpu=utilization.gpu,memory.used --format=csv,noheader,nounits`
/// Output example: `45, 4096`
/// Units: utilization in %, memory in MiB.
fn query_nvidia_smi() -> Option<GpuStats> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=utilization.gpu,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let line = stdout.lines().next()?;
    let mut parts = line.splitn(2, ',');

    let util_pct: f64 = parts.next()?.trim().parse().ok()?;
    let vram_mib: f64 = parts.next()?.trim().parse().ok()?;

    Some(GpuStats {
        util_frac: (util_pct / 100.0).clamp(0.0, 1.0),
        vram_used_bytes: vram_mib * 1024.0 * 1024.0,
    })
}

// ── Intel / xe helpers ────────────────────────────────────────────────────────

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
            let Ok(link) = fs::read_link(fd_entry.path()) else {
                continue;
            };
            if link.to_string_lossy() != target {
                continue;
            }

            let fd_name = fd_entry.file_name();
            let fdinfo_path = format!("/proc/{}/fdinfo/{}", pid_str, fd_name.to_string_lossy());
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
