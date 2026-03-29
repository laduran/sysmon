use sysinfo::{DiskExt, System, SystemExt};

/// Tracks free / total space on each mounted partition.
pub struct DiskMonitor {
    sys: System,
}

impl DiskMonitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_disks_list();
        sys.refresh_disks();
        Self { sys }
    }

    /// Return a vector of `(partition_name, usage_fraction, used_bytes)`.
    /// At most three entries are returned.
    pub fn update(&mut self) -> Vec<(String, f64, u64)> {
        self.sys.refresh_disks();

        let mut out: Vec<(String, f64, u64)> = self
            .sys
            .disks()
            .iter()
            .map(|disk| {
                let total = disk.total_space();
                let available = disk.available_space();
                let used = total.saturating_sub(available);
                let fraction = if total > 0 {
                    used as f64 / total as f64
                } else {
                    0.0
                };
                let name = disk.mount_point().to_string_lossy().into_owned();
                (name, fraction, used)
            })
            .collect();

        out.truncate(3);
        out
    }
}
