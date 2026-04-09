use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

mod cpu;
mod disk;
mod gpu;
mod memory;
mod ui;

use cpu::CpuMonitor;
use disk::DiskMonitor;
use gpu::GpuMonitor;
use memory::MemoryMonitor;
use ui::{Histories, create_ui, push_history};

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

        let (main_box, widgets, hist) = create_ui();
        let Histories {
            cpu: cpu_history,
            memory: mem_history,
            disks: disk_histories,
            mem_total_gb,
            gpu_util: gpu_util_history,
            gpu_vram: gpu_vram_history,
        } = hist;

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
        let mut gpu_monitor = GpuMonitor::new();

        // Keep a weak reference to the window so the timeout can stop itself
        // cleanly once the window has been destroyed.
        let win_weak = win.downgrade();

        // Tracks how many milliseconds have elapsed since the last data update.
        // The base tick is 500 ms (the minimum selectable interval); for longer
        // intervals the callback simply skips the update until enough ticks have
        // accumulated.
        let elapsed_ms: Rc<Cell<u64>> = Rc::new(Cell::new(0));

        glib::timeout_add_local(Duration::from_millis(500), move || {
            // Stop the timer if the window no longer exists.
            if win_weak.upgrade().is_none() {
                return glib::ControlFlow::Break;
            }

            let w = widgets.borrow();

            // Determine the selected polling interval from the dropdown.
            let interval_ms: u64 = match w.poll_dropdown.selected() {
                0 => 500,
                2 => 2000,
                _ => 1000, // default: 1 s
            };

            // Accumulate elapsed time and skip this tick if the interval hasn't
            // been reached yet.
            let new_elapsed = elapsed_ms.get() + 500;
            if new_elapsed < interval_ms {
                elapsed_ms.set(new_elapsed);
                return glib::ControlFlow::Continue;
            }
            elapsed_ms.set(0);

            drop(w);
            let cpu_frac = cpu_monitor.update();
            let mem_stats = mem_monitor.update();
            let disk_stats = disk_monitor.update(new_elapsed as f64 / 1000.0);

            let w = widgets.borrow();

            // ── CPU ────────────────────────────────────────────────────────
            w.cpu_percent.set_text(&format!("{:.1}%", cpu_frac * 100.0));
            push_history(&cpu_history, cpu_frac);
            w.cpu_graph.queue_draw();

            // ── Memory ──────────────────────────────────────────────────────
            let to_gb = |b: u64| b as f64 / (1024.0 * 1024.0 * 1024.0);
            mem_total_gb.set(to_gb(mem_stats.total));
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
                let frac = (mem_stats.used as f64 / mem_stats.total as f64).clamp(0.0, 1.0);
                push_history(&mem_history, frac);
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
                push_history(&disk_histories[i], (*read_bps, *write_bps));
                w.disk_graphs[i].set_visible(true);
                w.disk_graphs[i].queue_draw();
            }
            // Hide slots for which no physical device was found.
            for (label, graph) in disk_labels
                .iter()
                .zip(w.disk_graphs.iter())
                .skip(disk_stats.len())
            {
                label.set_visible(false);
                graph.set_visible(false);
            }

            // ── GPU ─────────────────────────────────────────────────────────
            if let Some(ref mut mon) = gpu_monitor
                && let Some(stats) = mon.update(new_elapsed)
            {
                w.gpu_panel.set_visible(true);
                w.gpu_util_label
                    .set_text(&format!("{:.1}%", stats.util_frac * 100.0));
                w.gpu_vram_label.set_text(&format!(
                    "VRAM: {:.2} GB",
                    stats.vram_used_bytes / (1024.0 * 1024.0 * 1024.0)
                ));
                push_history(&gpu_util_history, stats.util_frac);
                push_history(&gpu_vram_history, stats.vram_used_bytes);
                w.gpu_util_graph.queue_draw();
                w.gpu_vram_graph.queue_draw();
            }

            glib::ControlFlow::Continue
        });

        // Wrap content in a ScrolledWindow so the window can always be
        // resized narrower than the content's natural width.  Without this,
        // GTK4/Wayland locks the resize floor to the high-water-mark natural
        // width, preventing the FlowBox from wrapping.
        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .propagate_natural_width(false)
            .child(&main_box)
            .build();
        win.set_child(Some(&scrolled));
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
