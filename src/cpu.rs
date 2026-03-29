use std::time::Duration;

use sysinfo::System;

/// A simple CPU monitor that returns usage as a **fraction** (0.0 … 1.0).
pub struct CpuMonitor {
    sys: System,
}

impl CpuMonitor {
    /// Create a new monitor – grabs an initial CPU snapshot so the first
    /// `update()` call has a meaningful baseline to diff against.
    pub fn new() -> Self {
        let mut sys = System::new();
        // First refresh to establish a baseline.
        sys.refresh_cpu_all();
        // sysinfo requires at least 200 ms between CPU refreshes to produce
        // accurate usage figures (was System::MINIMUM_CPU_UPDATE_INTERVAL in 0.29).
        std::thread::sleep(Duration::from_millis(200));
        sys.refresh_cpu_all();
        Self { sys }
    }

    /// Return the current overall CPU usage as a fraction in [0.0, 1.0].
    pub fn update(&mut self) -> f64 {
        self.sys.refresh_cpu_all();
        let cpus = self.sys.cpus();
        if cpus.is_empty() {
            return 0.0;
        }
        let avg = cpus.iter().map(|c| c.cpu_usage() as f64).sum::<f64>() / cpus.len() as f64;
        (avg / 100.0).clamp(0.0, 1.0)
    }
}
