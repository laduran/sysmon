use std::collections::HashMap;
use std::fs;

/// Tracks per-physical-disk I/O throughput by diffing `/proc/diskstats`.
pub struct DiskMonitor {
    /// Previous cumulative sector counts: device -> (read_sectors, write_sectors).
    prev: HashMap<String, (u64, u64)>,
    /// Cached display names: device -> "nvme0n1 · WD_BLACK SN770 1TB".
    display: HashMap<String, String>,
}

impl DiskMonitor {
    pub fn new() -> Self {
        Self {
            prev: HashMap::new(),
            display: HashMap::new(),
        }
    }

    /// Return a vector of `(display_name, read_bytes_per_sec, write_bytes_per_sec)`.
    /// At most three physical devices are returned.
    /// On the very first call the throughput values are 0 (no previous baseline).
    pub fn update(&mut self) -> Vec<(String, f64, f64)> {
        let devices = physical_devices();
        let stats = read_diskstats();

        let mut out = Vec::new();
        for dev in &devices {
            if let Some(&(reads, writes)) = stats.get(dev) {
                let (prev_r, prev_w) = self.prev.get(dev).copied().unwrap_or((reads, writes));
                let read_bps = reads.saturating_sub(prev_r) as f64 * 512.0;
                let write_bps = writes.saturating_sub(prev_w) as f64 * 512.0;
                self.prev.insert(dev.clone(), (reads, writes));

                // Build and cache the display name (device + model) on first sight.
                let display = self
                    .display
                    .entry(dev.clone())
                    .or_insert_with(|| {
                        let model = device_model(dev)
                            .map(|m| format!(" · {}", m))
                            .unwrap_or_default();
                        format!("{}{}", dev, model)
                    })
                    .clone();

                out.push((display, read_bps, write_bps));
                if out.len() == 3 {
                    break;
                }
            }
        }
        out
    }
}

/// Enumerate physical block devices from `/sys/block/`, filtering out
/// loop devices, RAM disks, device-mapper volumes, and zram.
fn physical_devices() -> Vec<String> {
    let mut devs = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("loop")
                || name.starts_with("ram")
                || name.starts_with("dm-")
                || name.starts_with("zram")
            {
                continue;
            }
            devs.push(name);
        }
    }
    devs.sort();
    devs
}

/// Parse `/proc/diskstats` into a map of device name -> (read_sectors, write_sectors).
fn read_diskstats() -> HashMap<String, (u64, u64)> {
    let mut map = HashMap::new();
    let Ok(content) = fs::read_to_string("/proc/diskstats") else {
        return map;
    };
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Fields: major minor name reads_completed reads_merged sectors_read ...
        //         writes_completed writes_merged sectors_written ...
        // Indices:  0     1     2         3              4            5
        //                                7              8            9
        if parts.len() < 10 {
            continue;
        }
        if let (Ok(r), Ok(w)) = (parts[5].parse::<u64>(), parts[9].parse::<u64>()) {
            map.insert(parts[2].to_string(), (r, w));
        }
    }
    map
}

/// Look up a human-readable model string for a block device.
///
/// NVMe drives: reads `/sys/class/nvme/nvme<X>/model`
///              where the controller name is derived by stripping the trailing `n<N>` namespace suffix.
/// SATA/SCSI:   reads `/sys/block/<dev>/device/model`.
fn device_model(dev: &str) -> Option<String> {
    if dev.starts_with("nvme") {
        // "nvme0n1" -> controller "nvme0": strip trailing n<digits>
        if let Some(n_pos) = dev.rfind('n') {
            let after_n = &dev[n_pos + 1..];
            if !after_n.is_empty() && after_n.chars().all(|c| c.is_ascii_digit()) {
                let controller = &dev[..n_pos];
                let path = format!("/sys/class/nvme/{}/model", controller);
                if let Ok(s) = fs::read_to_string(&path) {
                    return Some(s.trim().to_string());
                }
            }
        }
    }
    // SATA / SCSI
    let path = format!("/sys/block/{}/device/model", dev);
    fs::read_to_string(&path).ok().map(|s| s.trim().to_string())
}
