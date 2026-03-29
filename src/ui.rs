use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box, DrawingArea, Label, Orientation};

/// How many seconds of history to keep for each graph.
/// Also used by the update loop in main.rs to size the ring buffers.
pub const HISTORY_LEN: usize = 120;

/// Grid line color (faint white).
const GRID_A: f64 = 0.08;

/// Shared history buffer holding 0-1 fractions (CPU, memory).
pub type History = Rc<RefCell<VecDeque<f64>>>;

/// Shared history buffer holding (read_bps, write_bps) pairs (disk).
pub type ThroughputHistory = Rc<RefCell<VecDeque<(f64, f64)>>>;

fn new_history() -> History {
    Rc::new(RefCell::new(VecDeque::with_capacity(HISTORY_LEN + 1)))
}

fn new_throughput_history() -> ThroughputHistory {
    Rc::new(RefCell::new(VecDeque::with_capacity(HISTORY_LEN + 1)))
}

/// Handles to every widget the update loop needs to touch.
pub struct Widgets {
    // CPU
    pub cpu_percent: Label,
    pub cpu_graph: DrawingArea,

    // Memory
    pub mem_total: Label,
    pub mem_used: Label,
    pub mem_free: Label,
    pub mem_swap: Label,
    pub mem_graph: DrawingArea,

    // Disk (up to 3 physical devices)
    pub disk1: Label,
    pub disk2: Label,
    pub disk3: Label,
    pub disk_graphs: Vec<DrawingArea>,
}

/// Build a mountain-style 2D history graph for a 0-1 fraction metric.
/// `fill` is RGBA for the filled area; `line` is RGB for the stroke.
fn make_graph(history: History, fill: (f64, f64, f64, f64), line: (f64, f64, f64)) -> DrawingArea {
    let area = DrawingArea::builder()
        .height_request(80)
        .hexpand(true)
        .build();

    area.set_draw_func(move |_area, cr, width, height| {
        let w = width as f64;
        let h = height as f64;
        let data = history.borrow();
        let n = data.len();

        cr.set_source_rgba(0.17, 0.17, 0.17, 1.0);
        let _ = cr.paint();

        cr.set_source_rgba(1.0, 1.0, 1.0, GRID_A);
        cr.set_line_width(1.0);
        for pct in &[0.25_f64, 0.50, 0.75] {
            let y = h - pct * h;
            cr.move_to(0.0, y);
            cr.line_to(w, y);
            let _ = cr.stroke();
        }

        if n < 2 {
            return;
        }

        let step = w / (HISTORY_LEN as f64 - 1.0);
        let x_offset = (HISTORY_LEN - n) as f64 * step;

        let point = |i: usize| -> (f64, f64) {
            let frac = data[i];
            let x = x_offset + i as f64 * step;
            let y = h - frac * (h - 2.0);
            (x, y)
        };

        let (x0, y0) = point(0);
        cr.move_to(x0, y0);
        for i in 1..n {
            let (xi, yi) = point(i);
            cr.line_to(xi, yi);
        }
        let (xn, _) = point(n - 1);
        cr.line_to(xn, h);
        cr.line_to(x0, h);
        cr.close_path();
        cr.set_source_rgba(fill.0, fill.1, fill.2, fill.3);
        let _ = cr.fill_preserve();

        cr.new_path();
        cr.move_to(x0, y0);
        for i in 1..n {
            let (xi, yi) = point(i);
            cr.line_to(xi, yi);
        }
        cr.set_source_rgb(line.0, line.1, line.2);
        cr.set_line_width(2.0);
        let _ = cr.stroke();
    });

    area
}

/// Build a dual-trace throughput graph for (read_bps, write_bps) history.
/// The Y-axis auto-scales to the maximum value seen in the current window.
/// Read trace: teal.  Write trace: amber.
fn make_throughput_graph(history: ThroughputHistory) -> DrawingArea {
    let area = DrawingArea::builder()
        .height_request(80)
        .hexpand(true)
        .build();

    area.set_draw_func(move |_area, cr, width, height| {
        let w = width as f64;
        let h = height as f64;
        let data = history.borrow();
        let n = data.len();

        cr.set_source_rgba(0.17, 0.17, 0.17, 1.0);
        let _ = cr.paint();

        cr.set_source_rgba(1.0, 1.0, 1.0, GRID_A);
        cr.set_line_width(1.0);
        for pct in &[0.25_f64, 0.50, 0.75] {
            let y = h - pct * h;
            cr.move_to(0.0, y);
            cr.line_to(w, y);
            let _ = cr.stroke();
        }

        if n < 2 {
            return;
        }

        // Auto-scale: find the peak across both traces in the visible window.
        // Use a minimum of 1.0 to avoid division by zero when the disk is idle.
        let max_val = data
            .iter()
            .flat_map(|&(r, wr)| [r, wr])
            .fold(1.0_f64, f64::max);

        let step = w / (HISTORY_LEN as f64 - 1.0);
        let x_offset = (HISTORY_LEN - n) as f64 * step;

        let xy = |i: usize, val: f64| -> (f64, f64) {
            let frac = (val / max_val).clamp(0.0, 1.0);
            (x_offset + i as f64 * step, h - frac * (h - 2.0))
        };

        // ── Read trace (teal) ────────────────────────────────────────────────
        {
            let (x0, y0) = xy(0, data[0].0);
            cr.move_to(x0, y0);
            for i in 1..n {
                let (xi, yi) = xy(i, data[i].0);
                cr.line_to(xi, yi);
            }
            let (xn, _) = xy(n - 1, data[n - 1].0);
            cr.line_to(xn, h);
            cr.line_to(x0, h);
            cr.close_path();
            cr.set_source_rgba(0.0, 200.0 / 255.0, 180.0 / 255.0, 0.35);
            let _ = cr.fill_preserve();

            cr.new_path();
            cr.move_to(x0, y0);
            for i in 1..n {
                let (xi, yi) = xy(i, data[i].0);
                cr.line_to(xi, yi);
            }
            cr.set_source_rgb(0.0, 140.0 / 255.0, 120.0 / 255.0);
            cr.set_line_width(2.0);
            let _ = cr.stroke();
        }

        // ── Write trace (amber) ──────────────────────────────────────────────
        {
            let (x0, y0) = xy(0, data[0].1);
            cr.move_to(x0, y0);
            for i in 1..n {
                let (xi, yi) = xy(i, data[i].1);
                cr.line_to(xi, yi);
            }
            let (xn, _) = xy(n - 1, data[n - 1].1);
            cr.line_to(xn, h);
            cr.line_to(x0, h);
            cr.close_path();
            cr.set_source_rgba(1.0, 140.0 / 255.0, 30.0 / 255.0, 0.35);
            let _ = cr.fill_preserve();

            cr.new_path();
            cr.move_to(x0, y0);
            for i in 1..n {
                let (xi, yi) = xy(i, data[i].1);
                cr.line_to(xi, yi);
            }
            cr.set_source_rgb(200.0 / 255.0, 80.0 / 255.0, 0.0);
            cr.set_line_width(2.0);
            let _ = cr.stroke();
        }
    });

    area
}

/// Build the entire UI.
/// Returns: root widget, widget handles, CPU history, memory history,
/// and per-disk throughput histories (up to 3).
pub fn create_ui() -> (gtk4::Box, Widgets, History, History, Vec<ThroughputHistory>) {
    // ── Root container ──────────────────────────────────────────────────────
    let main_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    // ── CPU panel (blue) ─────────────────────────────────────────────────────
    let cpu_history = new_history();
    let cpu_graph = make_graph(
        Rc::clone(&cpu_history),
        (100.0 / 255.0, 180.0 / 255.0, 255.0 / 255.0, 0.35),
        (30.0 / 255.0, 100.0 / 255.0, 200.0 / 255.0),
    );
    let cpu_percent = Label::new(Some("0.0%"));
    let cpu_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    cpu_box.append(&Label::new(Some("CPU Usage")));
    cpu_box.append(&cpu_percent);
    cpu_box.append(&cpu_graph);

    // ── Memory panel (green) ─────────────────────────────────────────────────
    let mem_history = new_history();
    let mem_graph = make_graph(
        Rc::clone(&mem_history),
        (100.0 / 255.0, 220.0 / 255.0, 130.0 / 255.0, 0.35),
        (30.0 / 255.0, 160.0 / 255.0, 60.0 / 255.0),
    );
    let mem_total = Label::new(Some("Total: —"));
    let mem_used = Label::new(Some("Used:  —"));
    let mem_free = Label::new(Some("Free:  —"));
    let mem_swap = Label::new(Some("Swap:  —"));
    let mem_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    mem_box.append(&Label::new(Some("Memory")));
    mem_box.append(&mem_total);
    mem_box.append(&mem_used);
    mem_box.append(&mem_free);
    mem_box.append(&mem_swap);
    mem_box.append(&mem_graph);

    // ── Disk panel (teal reads / amber writes, auto-scaled) ──────────────────
    let disk_histories: Vec<ThroughputHistory> = (0..3).map(|_| new_throughput_history()).collect();
    let disk_graphs: Vec<DrawingArea> = disk_histories
        .iter()
        .map(|h| make_throughput_graph(Rc::clone(h)))
        .collect();

    let disk1 = Label::new(Some("Disk 1: —"));
    let disk2 = Label::new(Some("Disk 2: —"));
    let disk3 = Label::new(Some("Disk 3: —"));

    let disk_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    disk_box.append(&Label::new(Some("Disks  (teal = read, amber = write)")));
    disk_box.append(&disk1);
    disk_box.append(&disk_graphs[0]);
    disk_box.append(&disk2);
    disk_box.append(&disk_graphs[1]);
    disk_box.append(&disk3);
    disk_box.append(&disk_graphs[2]);

    // ── Assemble ─────────────────────────────────────────────────────────────
    main_box.append(&cpu_box);
    main_box.append(&mem_box);
    main_box.append(&disk_box);

    let widgets = Widgets {
        cpu_percent,
        cpu_graph,
        mem_total,
        mem_used,
        mem_free,
        mem_swap,
        mem_graph,
        disk1,
        disk2,
        disk3,
        disk_graphs,
    };

    (main_box, widgets, cpu_history, mem_history, disk_histories)
}
