use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box, DrawingArea, Label, Orientation, ProgressBar};

/// How many seconds of CPU history to keep.
const HISTORY_LEN: usize = 120;

/// Fill color for the mountain graph area (light blue, semi-transparent).
const FILL_R: f64 = 100.0 / 255.0;
const FILL_G: f64 = 180.0 / 255.0;
const FILL_B: f64 = 1.0;
const FILL_A: f64 = 0.35;

/// Stroke color for the mountain graph line (darker blue).
const LINE_R: f64 = 30.0 / 255.0;
const LINE_G: f64 = 100.0 / 255.0;
const LINE_B: f64 = 200.0 / 255.0;

/// Grid line color (faint white).
const GRID_A: f64 = 0.08;

/// Shared, reference-counted history buffer type.
pub type CpuHistory = Rc<RefCell<VecDeque<f64>>>;

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
    pub mem_progress: ProgressBar,

    // Disk (up to 3 partitions)
    pub disk1: Label,
    pub disk2: Label,
    pub disk3: Label,
    pub disk_progresses: Vec<ProgressBar>,
}

/// Build the entire UI.
/// Returns the root widget, widget handles, and the shared CPU history buffer.
pub fn create_ui() -> (gtk4::Box, Widgets, CpuHistory) {
    // ── Root container ──────────────────────────────────────────────────────
    let main_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    // ── CPU history buffer ───────────────────────────────────────────────────
    let history: CpuHistory = Rc::new(RefCell::new(VecDeque::with_capacity(HISTORY_LEN + 1)));

    // ── CPU DrawingArea ──────────────────────────────────────────────────────
    let cpu_graph = DrawingArea::builder()
        .height_request(80)
        .hexpand(true)
        .build();

    // Clone the Rc so the draw closure can borrow it each frame.
    let history_for_draw = Rc::clone(&history);

    cpu_graph.set_draw_func(move |_area, cr, width, height| {
        let w = width as f64;
        let h = height as f64;
        let data = history_for_draw.borrow();
        let n = data.len();

        // ── Background ──────────────────────────────────────────────────────
        // Use the dark background color from the GTK theme by clearing to
        // transparent (the window/box background shows through).
        cr.set_source_rgba(0.17, 0.17, 0.17, 1.0);
        let _ = cr.paint();

        // ── Guide lines at 25 / 50 / 75 % ───────────────────────────────────
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

        // ── Build the curve path ─────────────────────────────────────────────
        // x spacing between samples
        let step = w / (HISTORY_LEN as f64 - 1.0);

        // Start x offset so the newest sample is always at the right edge.
        let x_offset = (HISTORY_LEN - n) as f64 * step;

        // Helper: sample index → (x, y) in widget coords.
        let point = |i: usize| -> (f64, f64) {
            let frac = data[i];
            let x = x_offset + i as f64 * step;
            let y = h - frac * (h - 2.0); // leave 2 px headroom at top
            (x, y)
        };

        // Move to the first point.
        let (x0, y0) = point(0);
        cr.move_to(x0, y0);
        for i in 1..n {
            let (xi, yi) = point(i);
            cr.line_to(xi, yi);
        }

        // ── Filled area ──────────────────────────────────────────────────────
        let (xn, _) = point(n - 1);
        cr.line_to(xn, h);
        cr.line_to(x0, h);
        cr.close_path();
        cr.set_source_rgba(FILL_R, FILL_G, FILL_B, FILL_A);
        let _ = cr.fill_preserve();

        // ── Stroke line ──────────────────────────────────────────────────────
        // Re-draw just the curve (not the closing baseline) as a stroke.
        cr.new_path();
        cr.move_to(x0, y0);
        for i in 1..n {
            let (xi, yi) = point(i);
            cr.line_to(xi, yi);
        }
        cr.set_source_rgb(LINE_R, LINE_G, LINE_B);
        cr.set_line_width(2.0);
        let _ = cr.stroke();
    });

    let cpu_title = Label::new(Some("CPU Usage"));
    let cpu_percent = Label::new(Some("0.0%"));

    let cpu_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    cpu_box.append(&cpu_title);
    cpu_box.append(&cpu_percent);
    cpu_box.append(&cpu_graph);

    // ── Memory panel ────────────────────────────────────────────────────────
    let mem_total = Label::new(Some("Total: —"));
    let mem_used = Label::new(Some("Used:  —"));
    let mem_free = Label::new(Some("Avail: —"));
    let mem_swap = Label::new(Some("Swap:  —"));
    let mem_progress = ProgressBar::new();
    mem_progress.set_fraction(0.0);
    mem_progress.set_show_text(true);

    let mem_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    mem_box.append(&Label::new(Some("Memory")));
    mem_box.append(&mem_total);
    mem_box.append(&mem_used);
    mem_box.append(&mem_free);
    mem_box.append(&mem_swap);
    mem_box.append(&mem_progress);

    // ── Disk panel ──────────────────────────────────────────────────────────
    let disk1 = Label::new(Some("Disk 1: —"));
    let disk2 = Label::new(Some("Disk 2: —"));
    let disk3 = Label::new(Some("Disk 3: —"));
    let disk_prog1 = ProgressBar::new();
    let disk_prog2 = ProgressBar::new();
    let disk_prog3 = ProgressBar::new();
    for p in &[&disk_prog1, &disk_prog2, &disk_prog3] {
        p.set_fraction(0.0);
        p.set_show_text(true);
    }

    let disk_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    disk_box.append(&Label::new(Some("Disks")));
    disk_box.append(&disk1);
    disk_box.append(&disk_prog1);
    disk_box.append(&disk2);
    disk_box.append(&disk_prog2);
    disk_box.append(&disk3);
    disk_box.append(&disk_prog3);

    // ── Assemble ─────────────────────────────────────────────────────────────
    main_box.append(&cpu_box);
    main_box.append(&mem_box);
    main_box.append(&disk_box);

    let widgets = Widgets {
        cpu_percent,
        cpu_graph: cpu_graph.clone(),
        mem_total,
        mem_used,
        mem_free,
        mem_swap,
        mem_progress,
        disk1,
        disk2,
        disk3,
        disk_progresses: vec![disk_prog1, disk_prog2, disk_prog3],
    };

    (main_box, widgets, history)
}
