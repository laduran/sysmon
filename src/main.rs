use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use std::time::Duration;

mod cpu;
mod disk;
mod memory;
mod ui;

use cpu::CpuMonitor;
use disk::DiskMonitor;
use memory::MemoryMonitor;
use ui::create_ui;

/// Maximum history samples to keep (matches HISTORY_LEN in ui.rs).
const HISTORY_LEN: usize = 120;

fn main() {
    let app = Application::builder()
        .application_id("com.example.systemmonitor")
        .build();

    app.connect_activate(|app| {
        let win = ApplicationWindow::builder()
            .application(app)
            .title("System Monitor")
            .default_width(500)
            .default_height(500)
            .build();

        let (main_box, widgets, cpu_history) = create_ui();

        // Wrap widgets in Rc<RefCell<>> so we can share them with the timeout closure.
        let widgets = std::rc::Rc::new(std::cell::RefCell::new(widgets));

        // Initialise monitors on the main thread (they're not Send).
        let mut cpu_monitor = CpuMonitor::new();
        let mut mem_monitor = MemoryMonitor::new();
        let mut disk_monitor = DiskMonitor::new();

        // Use a GLib timeout to refresh every second (no extra threads needed).
        glib::timeout_add_local(Duration::from_secs(1), move || {
            let cpu_frac = cpu_monitor.update();
            let mem_stats = mem_monitor.update();
            let disk_stats = disk_monitor.update();

            let w = widgets.borrow();

            // ── CPU ────────────────────────────────────────────────────────
            w.cpu_percent.set_text(&format!("{:.1}%", cpu_frac * 100.0));

            // Push new value into the history ring buffer, trim if full.
            {
                let mut hist = cpu_history.borrow_mut();
                if hist.len() == HISTORY_LEN {
                    hist.pop_front();
                }
                hist.push_back(cpu_frac);
            }
            // Ask GTK to redraw the graph on the next frame.
            w.cpu_graph.queue_draw();

            // ── Memory ──────────────────────────────────────────────────────
            let to_gb = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
            let to_mb = |b: u64| b as f64 / (1024.0 * 1024.0);
            w.mem_total
                .set_text(&format!("Total: {:.2} GB", to_gb(mem_stats.total)));
            w.mem_used
                .set_text(&format!("Used:  {:.2} GB", to_gb(mem_stats.used)));
            w.mem_free
                .set_text(&format!("Avail: {:.2} GB", to_gb(mem_stats.free)));
            w.mem_swap.set_text(&format!(
                "Swap:  {:.0} MB used / {:.0} MB total",
                to_mb(mem_stats.swap_used),
                to_mb(mem_stats.swap_total)
            ));
            if mem_stats.total > 0 {
                w.mem_progress
                    .set_fraction((mem_stats.used as f64 / mem_stats.total as f64).clamp(0.0, 1.0));
            }

            // ── Disk ────────────────────────────────────────────────────────
            let disk_labels = [&w.disk1, &w.disk2, &w.disk3];
            for (i, (name, frac, _)) in disk_stats.iter().enumerate().take(3) {
                disk_labels[i].set_text(&format!("{} – {:.1}%", name, frac * 100.0));
                if i < w.disk_progresses.len() {
                    w.disk_progresses[i].set_fraction(*frac);
                }
            }

            glib::ControlFlow::Continue
        });

        win.set_child(Some(&main_box));
        win.present();
    });

    app.run();
}
