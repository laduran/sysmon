use gtk4::prelude::*;
use gtk4::{Box, Label, ProgressBar, Orientation};

/// Handles to every widget the update loop needs to touch.
pub struct Widgets {
    // CPU
    pub cpu_percent:  Label,
    pub cpu_progress: ProgressBar,

    // Memory
    pub mem_total:    Label,
    pub mem_used:     Label,
    pub mem_free:     Label,
    pub mem_swap:     Label,
    pub mem_progress: ProgressBar,

    // Disk (up to 3 partitions)
    pub disk1: Label,
    pub disk2: Label,
    pub disk3: Label,
    pub disk_progresses: Vec<ProgressBar>,
}

/// Build the entire UI and return the root widget together with all
/// the widget handles the update loop will mutate.
pub fn create_ui() -> (gtk4::Box, Widgets) {
    // ── Root container ──────────────────────────────────────────────────────
    let main_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    // ── CPU panel ───────────────────────────────────────────────────────────
    let cpu_title    = Label::new(Some("CPU Usage"));
    let cpu_percent  = Label::new(Some("0.0%"));
    let cpu_progress = ProgressBar::new();
    cpu_progress.set_fraction(0.0);
    cpu_progress.set_show_text(true);

    let cpu_box = Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(4)
        .build();
    cpu_box.append(&cpu_title);
    cpu_box.append(&cpu_percent);
    cpu_box.append(&cpu_progress);

    // ── Memory panel ────────────────────────────────────────────────────────
    let mem_total    = Label::new(Some("Total: —"));
    let mem_used     = Label::new(Some("Used:  —"));
    let mem_free     = Label::new(Some("Free:  —"));
    let mem_swap     = Label::new(Some("Swap:  —"));
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
        cpu_progress,
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

    (main_box, widgets)
}
