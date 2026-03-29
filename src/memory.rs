use sysinfo::{System, SystemExt};

/// Holds the current memory statistics (all values in bytes).
pub struct MemoryMonitor {
    sys: System,
}

impl MemoryMonitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        Self { sys }
    }

    /// Return a struct with the *current* memory usage.
    pub fn update(&mut self) -> MemoryStats {
        self.sys.refresh_memory();
        MemoryStats {
            total:      self.sys.total_memory(),
            used:       self.sys.used_memory(),
            free:       self.sys.free_memory(),
            swap_total: self.sys.total_swap(),
            swap_used:  self.sys.used_swap(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct MemoryStats {
    pub total:      u64, // bytes
    pub used:       u64, // bytes
    pub free:       u64, // bytes
    pub swap_total: u64,
    pub swap_used:  u64,
}
