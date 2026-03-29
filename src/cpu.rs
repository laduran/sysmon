use sysinfo::{CpuExt, System, SystemExt};

/// A simple CPU monitor that returns usage as a **fraction** (0.0 … 1.0).
pub struct CpuMonitor {
    sys: System,
}

impl CpuMonitor {
    /// Create a new monitor – grabs an initial CPU snapshot so the first
    /// `update()` call has a meaningful baseline to diff against.
    pub fn new() -> Self {
        let mut sys = System::new();
        // First refresh to establish a baseline
        sys.refresh_cpu();
        // Sleep the minimum required interval then refresh again so the
        // very first call to `update()` returns a real value.
        std::thread::sleep(System::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_cpu();
        Self { sys }
    }

    /// Return the current overall CPU usage as a fraction in [0.0, 1.0].
    pub fn update(&mut self) -> f64 {
        self.sys.refresh_cpu();
        let cpus = self.sys.cpus();
        if cpus.is_empty() {
            return 0.0;
        }
        let avg = cpus.iter().map(|c| c.cpu_usage() as f64).sum::<f64>() / cpus.len() as f64;
        (avg / 100.0).clamp(0.0, 1.0)
    }
}
