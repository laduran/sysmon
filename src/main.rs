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
use ui::{create_ui, HISTORY_LEN};

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

        let (main_box, widgets, cpu_history, mem_history, disk_histories) = create_ui();

        // Wrap widgets in Rc<RefCell<>> so we can share them with the timeout closure.
        let widgets = std::rc::Rc::new(std::cell::RefCell::new(widgets));

        // Quit the application when the window's close button is pressed.
        // Without this, the glib timeout source keeps the process alive indefinitely.
        let app_handle = app.clone();
        win.connect_close_request(move |_| {
            app_handle.quit();
            gtk4::glib::Propagation::Proceed
        });

        // Initialise monitors on the main thread (they're not Send).
        let mut cpu_monitor = CpuMonitor::new();
        let mut mem_monitor = MemoryMonitor::new();
        let mut disk_monitor = DiskMonitor::new();

        // Keep a weak reference to the window so the timeout can stop itself
        // cleanly once the window has been destroyed.
        let win_weak = win.downgrade();

        // Use a GLib timeout to refresh every second (no extra threads needed).
        glib::timeout_add_local(Duration::from_secs(1), move || {
            // Stop the timer if the window no longer exists.
            if win_weak.upgrade().is_none() {
                return glib::ControlFlow::Break;
            }
            let cpu_frac = cpu_monitor.update();
            let mem_stats = mem_monitor.update();
            let disk_stats = disk_monitor.update();

            let w = widgets.borrow();

            // ── CPU ────────────────────────────────────────────────────────
            w.cpu_percent.set_text(&format!("{:.1}%", cpu_frac * 100.0));
            {
                let mut hist = cpu_history.borrow_mut();
                if hist.len() == HISTORY_LEN {
                    hist.pop_front();
                }
                hist.push_back(cpu_frac);
            }
            w.cpu_graph.queue_draw();

            // ── Memory ──────────────────────────────────────────────────────
            let to_gb = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
            let to_mb = |b: u64| b as f64 / (1024.0 * 1024.0);
            w.mem_total
                .set_text(&format!("Total: {:.2} GB", to_gb(mem_stats.total)));
            w.mem_used
                .set_text(&format!("Used:  {:.2} GB", to_gb(mem_stats.used)));
            w.mem_free
                .set_text(&format!("Free:  {:.2} GB", to_gb(mem_stats.free)));
            w.mem_swap.set_text(&format!(
                "Swap:  {:.0} MB used / {:.0} MB total",
                to_mb(mem_stats.swap_used),
                to_mb(mem_stats.swap_total)
            ));
            if mem_stats.total > 0 {
                let frac = (mem_stats.used as f64 / mem_stats.total as f64).clamp(0.0, 1.0);
                let mut hist = mem_history.borrow_mut();
                if hist.len() == HISTORY_LEN {
                    hist.pop_front();
                }
                hist.push_back(frac);
            }
            w.mem_graph.queue_draw();

            // ── Disk ────────────────────────────────────────────────────────
            let disk_labels = [&w.disk1, &w.disk2, &w.disk3];
            for (i, (name, read_bps, write_bps)) in disk_stats.iter().enumerate().take(3) {
                disk_labels[i].set_text(&format!(
                    "{}  R: {}  W: {}",
                    name,
                    fmt_rate(*read_bps),
                    fmt_rate(*write_bps),
                ));
                disk_labels[i].set_visible(true);
                {
                    let mut hist = disk_histories[i].borrow_mut();
                    if hist.len() == HISTORY_LEN {
                        hist.pop_front();
                    }
                    hist.push_back((*read_bps, *write_bps));
                }
                w.disk_graphs[i].set_visible(true);
                w.disk_graphs[i].queue_draw();
            }
            // Hide slots for which no physical device was found.
            for i in disk_stats.len()..3 {
                disk_labels[i].set_visible(false);
                w.disk_graphs[i].set_visible(false);
            }

            glib::ControlFlow::Continue
        });

        win.set_child(Some(&main_box));
        win.present();
    });

    app.run();
}

/// Format a byte-per-second rate as a human-readable string.
fn fmt_rate(bps: f64) -> String {
    if bps >= 1024.0 * 1024.0 {
        format!("{:.1} MB/s", bps / (1024.0 * 1024.0))
    } else if bps >= 1024.0 {
        format!("{:.0} KB/s", bps / 1024.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}
