use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box, DrawingArea, DropDown, Label, Orientation, StringList};

/// How many seconds of history to keep for each graph.
/// Also used by the update loop in main.rs to size the ring buffers.
pub const HISTORY_LEN: usize = 120;

// ── Colours ──────────────────────────────────────────────────────────────────

/// An RGB colour with components in [0.0, 1.0].
#[derive(Clone, Copy)]
struct Color {
    r: f64,
    g: f64,
    b: f64,
}

impl Color {
    /// Construct from the familiar 0-255 component range.
    const fn from_u8(r: u8, g: u8, b: u8) -> Self {
        // Integer-to-float casts in const fn are stable since Rust 1.45.
        Self {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
        }
    }
}

/// Graph background colour.
const BG: Color = Color::from_u8(43, 43, 43);

/// Opacity of the filled area under every graph curve.
const FILL_ALPHA: f64 = 0.35;

/// Faint white guide lines drawn at 25 / 50 / 75 %.
const GRID_ALPHA: f64 = 0.08;

// Per-panel colour pairs (fill = lighter, line = darker stroke).
const CPU_FILL: Color = Color::from_u8(100, 180, 255); // blue
const CPU_LINE: Color = Color::from_u8(30, 100, 200);
const MEM_FILL: Color = Color::from_u8(100, 220, 130); // green
const MEM_LINE: Color = Color::from_u8(30, 160, 60);
const DISK_READ_FILL: Color = Color::from_u8(0, 200, 180); // teal
const DISK_READ_LINE: Color = Color::from_u8(0, 140, 120);
const DISK_WRITE_FILL: Color = Color::from_u8(255, 140, 30); // amber
const DISK_WRITE_LINE: Color = Color::from_u8(200, 80, 0);

// ── History buffer types ──────────────────────────────────────────────────────

/// Shared history buffer holding 0-1 fractions (CPU, memory).
pub type History = Rc<RefCell<VecDeque<f64>>>;

/// Shared history buffer holding (read_bps, write_bps) pairs (disk).
pub type ThroughputHistory = Rc<RefCell<VecDeque<(f64, f64)>>>;

/// Named histories returned by `create_ui`, one field per data source.
/// Keeps `create_ui`'s return type readable and future-proof.
pub struct Histories {
    pub cpu: History,
    pub memory: History,
    pub disks: Vec<ThroughputHistory>,
}

fn new_history() -> History {
    Rc::new(RefCell::new(VecDeque::with_capacity(HISTORY_LEN + 1)))
}

fn new_throughput_history() -> ThroughputHistory {
    Rc::new(RefCell::new(VecDeque::with_capacity(HISTORY_LEN + 1)))
}

/// Push one sample into a history ring buffer, evicting the oldest entry when full.
/// Works for both `History` (f64) and `ThroughputHistory` ((f64, f64)).
pub fn push_history<T>(history: &Rc<RefCell<VecDeque<T>>>, value: T) {
    let mut h = history.borrow_mut();
    if h.len() == HISTORY_LEN {
        h.pop_front();
    }
    h.push_back(value);
}

// ── Widget handles ────────────────────────────────────────────────────────────

/// Handles to every widget the update loop needs to touch.
pub struct Widgets {
    // Toolbar
    pub poll_dropdown: DropDown,

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

// ── Graph constructors ────────────────────────────────────────────────────────

/// Build a mountain-style 2D history graph for a 0-1 fraction metric.
fn make_graph(history: History, fill: Color, line: Color) -> DrawingArea {
    let area = DrawingArea::builder()
        .height_request(80)
        .hexpand(true)
        .build();

    area.set_draw_func(move |_area, cr, width, height| {
        let w = width as f64;
        let h = height as f64;
        let data = history.borrow();
        let n = data.len();

        cr.set_source_rgb(BG.r, BG.g, BG.b);
        let _ = cr.paint();

        cr.set_source_rgba(1.0, 1.0, 1.0, GRID_ALPHA);
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
            let x = x_offset + i as f64 * step;
            let y = h - data[i] * (h - 2.0);
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
        cr.set_source_rgba(fill.r, fill.g, fill.b, FILL_ALPHA);
        let _ = cr.fill_preserve();

        cr.new_path();
        cr.move_to(x0, y0);
        for i in 1..n {
            let (xi, yi) = point(i);
            cr.line_to(xi, yi);
        }
        cr.set_source_rgb(line.r, line.g, line.b);
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

        cr.set_source_rgb(BG.r, BG.g, BG.b);
        let _ = cr.paint();

        cr.set_source_rgba(1.0, 1.0, 1.0, GRID_ALPHA);
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

        // Auto-scale to the peak across both traces; minimum 1.0 avoids
        // division by zero when the disk is idle.
        let max_val = data
            .iter()
            .flat_map(|&(r, w)| [r, w])
            .fold(1.0_f64, f64::max);

        let step = w / (HISTORY_LEN as f64 - 1.0);
        let x_offset = (HISTORY_LEN - n) as f64 * step;

        let xy = |i: usize, val: f64| -> (f64, f64) {
            let frac = (val / max_val).clamp(0.0, 1.0);
            (x_offset + i as f64 * step, h - frac * (h - 2.0))
        };

        // Each trace: (sample extractor, fill colour, stroke colour).
        type TraceDef = (fn(&(f64, f64)) -> f64, Color, Color);
        let traces: [TraceDef; 2] = [
            (|s: &(f64, f64)| s.0, DISK_READ_FILL, DISK_READ_LINE), // teal  – read
            (|s: &(f64, f64)| s.1, DISK_WRITE_FILL, DISK_WRITE_LINE), // amber – write
        ];

        for (get, fill, line) in traces {
            let (x0, y0) = xy(0, get(&data[0]));
            cr.move_to(x0, y0);
            for i in 1..n {
                let (xi, yi) = xy(i, get(&data[i]));
                cr.line_to(xi, yi);
            }
            let (xn, _) = xy(n - 1, get(&data[n - 1]));
            cr.line_to(xn, h);
            cr.line_to(x0, h);
            cr.close_path();
            cr.set_source_rgba(fill.r, fill.g, fill.b, FILL_ALPHA);
            let _ = cr.fill_preserve();

            cr.new_path();
            cr.move_to(x0, y0);
            for i in 1..n {
                let (xi, yi) = xy(i, get(&data[i]));
                cr.line_to(xi, yi);
            }
            cr.set_source_rgb(line.r, line.g, line.b);
            cr.set_line_width(2.0);
            let _ = cr.stroke();
        }
    });

    area
}

// ── UI assembly ───────────────────────────────────────────────────────────────

/// Build the entire UI and return widget handles plus the shared history buffers.
pub fn create_ui() -> (gtk4::Box, Widgets, Histories) {
    let main_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    // ── Toolbar ──────────────────────────────────────────────────────────────
    let poll_dropdown = DropDown::builder()
        .model(&StringList::new(&["0.5 s", "1 s", "2 s"]))
        .selected(1) // default: 1 s
        .build();
    let toolbar = Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(6)
        .build();
    toolbar.append(&Label::new(Some("Poll interval:")));
    toolbar.append(&poll_dropdown);

    // ── CPU panel ────────────────────────────────────────────────────────────
    let cpu_history = new_history();
    let cpu_graph = make_graph(Rc::clone(&cpu_history), CPU_FILL, CPU_LINE);
    let cpu_percent = Label::new(Some("0.0%"));
    let cpu_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    cpu_box.append(&Label::new(Some("CPU Usage")));
    cpu_box.append(&cpu_percent);
    cpu_box.append(&cpu_graph);

    // ── Memory panel ─────────────────────────────────────────────────────────
    let mem_history = new_history();
    let mem_graph = make_graph(Rc::clone(&mem_history), MEM_FILL, MEM_LINE);
    let mem_total = Label::new(Some("Total: —"));
    let mem_used = Label::new(Some("Used:  —"));
    let mem_free = Label::new(Some("Avail: —"));
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

    // ── Disk panel ───────────────────────────────────────────────────────────
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

    // ── Assemble ──────────────────────────────────────────────────────────────
    main_box.append(&toolbar);
    main_box.append(&cpu_box);
    main_box.append(&mem_box);
    main_box.append(&disk_box);

    let widgets = Widgets {
        poll_dropdown,
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

    let histories = Histories {
        cpu: cpu_history,
        memory: mem_history,
        disks: disk_histories,
    };

    (main_box, widgets, histories)
}
