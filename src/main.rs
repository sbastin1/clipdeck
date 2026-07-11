use anyhow::{Context, Result};
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, Label, ListBox, ListBoxRow, Orientation,
    SearchEntry,
};
use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::thread;
use std::time::Duration;

const APP_ID: &str = "dev.clipdeck.Picker";
const MAX_ITEMS: usize = 200;
const POLL_INTERVAL: Duration = Duration::from_millis(700);

#[derive(Clone, Debug, Deserialize, Serialize)]
struct HistoryItem {
    text: String,
}

fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("daemon") => run_daemon(),
        Some("help") | Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some(other) => anyhow::bail!("unknown command: {other}"),
        None => run_picker(),
    }
}

fn print_help() {
    println!("Usage:");
    println!("  clipdeck          Open clipboard picker");
    println!("  clipdeck daemon   Record text clipboard history");
}

fn run_daemon() -> Result<()> {
    ensure_wl_clipboard()?;

    let mut last_seen = String::new();

    loop {
        match read_clipboard_text() {
            Ok(text) if !text.trim().is_empty() && text != last_seen => {
                last_seen = text.clone();
                if let Err(err) = add_history_item(text) {
                    eprintln!("failed to update clipboard history: {err:#}");
                }
            }
            Ok(_) => {}
            Err(err) => eprintln!("failed to read clipboard: {err:#}"),
        }

        thread::sleep(POLL_INTERVAL);
    }
}

fn run_picker() -> Result<()> {
    ensure_wl_clipboard()?;

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);

    app.run();
    Ok(())
}

fn build_ui(app: &Application) {
    let history = match load_history() {
        Ok(items) => items,
        Err(err) => {
            eprintln!("failed to load clipboard history: {err:#}");
            Vec::new()
        }
    };

    let filtered = Rc::new(RefCell::new(history.clone()));

    let search = SearchEntry::builder()
        .placeholder_text("Search clipboard history")
        .hexpand(true)
        .build();

    let list = ListBox::builder()
        .activate_on_single_click(false)
        .vexpand(true)
        .build();

    populate_list(&list, &history);

    let root = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    root.append(&search);
    root.append(&list);

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Clipboard")
        .default_width(720)
        .default_height(460)
        .child(&root)
        .build();

    setup_layer_shell(&window);

    if let Some(row) = list.row_at_index(0) {
        list.select_row(Some(&row));
    }

    let list_for_search = list.clone();
    let history_for_search = history.clone();
    let filtered_for_search = filtered.clone();
    search.connect_search_changed(move |entry| {
        let query = entry.text().to_string().to_lowercase();
        let matches: Vec<HistoryItem> = history_for_search
            .iter()
            .filter(|item| item.text.to_lowercase().contains(&query))
            .cloned()
            .collect();

        *filtered_for_search.borrow_mut() = matches.clone();
        populate_list(&list_for_search, &matches);
        if let Some(row) = list_for_search.row_at_index(0) {
            list_for_search.select_row(Some(&row));
        }
    });

    let app_for_activate = app.clone();
    let filtered_for_activate = filtered.clone();
    list.connect_row_activated(move |_, row| {
        select_row(
            row.index(),
            &filtered_for_activate.borrow(),
            &app_for_activate,
        );
    });

    let app_for_search_stop = app.clone();
    search.connect_stop_search(move |_| {
        app_for_search_stop.quit();
    });

    let key_controller = gtk::EventControllerKey::new();
    let list_for_keys = list.clone();
    let app_for_keys = app.clone();
    let filtered_for_keys = filtered.clone();
    key_controller.connect_key_pressed(move |_, key, _, _| match key {
        gdk::Key::Escape => {
            app_for_keys.quit();
            glib::Propagation::Stop
        }
        gdk::Key::Return | gdk::Key::KP_Enter => {
            if let Some(row) = list_for_keys.selected_row() {
                select_row(row.index(), &filtered_for_keys.borrow(), &app_for_keys);
            }
            glib::Propagation::Stop
        }
        gdk::Key::Down => {
            move_selection(&list_for_keys, 1);
            glib::Propagation::Stop
        }
        gdk::Key::Up => {
            move_selection(&list_for_keys, -1);
            glib::Propagation::Stop
        }
        _ => glib::Propagation::Proceed,
    });
    window.add_controller(key_controller);

    window.present();
    search.grab_focus();
}

fn setup_layer_shell(window: &ApplicationWindow) {
    if !gtk4_layer_shell::is_supported() {
        eprintln!("gtk-layer-shell is not supported by this compositor; using a normal window");
        return;
    }

    window.init_layer_shell();
    window.set_namespace(Some("clipdeck"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    window.set_exclusive_zone(-1);
}

fn populate_list(list: &ListBox, items: &[HistoryItem]) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    for item in items {
        let preview = preview_text(&item.text);
        let row = ListBoxRow::new();
        let label = Label::builder()
            .label(preview)
            .xalign(0.0)
            .wrap(true)
            .max_width_chars(96)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(10)
            .margin_end(10)
            .build();
        row.set_child(Some(&label));
        list.append(&row);
    }
}

fn preview_text(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview: String = compact.chars().take(220).collect();
    if compact.chars().count() > 220 {
        preview.push_str("...");
    }
    preview
}

fn move_selection(list: &ListBox, delta: i32) {
    let current = list.selected_row().map(|row| row.index()).unwrap_or(0);
    let next = (current + delta).max(0);

    if let Some(row) = list.row_at_index(next) {
        list.select_row(Some(&row));
        row.grab_focus();
    }
}

fn select_row(index: i32, items: &[HistoryItem], app: &Application) {
    if let Some(item) = items.get(index as usize) {
        if let Err(err) =
            write_clipboard_text(&item.text).and_then(|_| add_history_item(item.text.clone()))
        {
            eprintln!("failed to select clipboard item: {err:#}");
        }
    }

    app.quit();
}

fn history_path() -> Result<PathBuf> {
    let mut dir = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .context("XDG_RUNTIME_DIR is not set; cannot store runtime clipboard history")?;
    dir.push("clipdeck");
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    Ok(dir.join("history.json"))
}

fn load_history() -> Result<Vec<HistoryItem>> {
    let path = history_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let items = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(items)
}

fn save_history(items: &[HistoryItem]) -> Result<()> {
    let path = history_path()?;
    let contents = serde_json::to_string_pretty(items)?;
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn add_history_item(text: String) -> Result<()> {
    let mut items = load_history()?;
    items.retain(|item| item.text != text);
    items.insert(0, HistoryItem { text });
    items.truncate(MAX_ITEMS);
    save_history(&items)
}

fn read_clipboard_text() -> Result<String> {
    let output = Command::new("wl-paste")
        .args(["--no-newline", "--type", "text"])
        .output()
        .context("failed to run wl-paste")?;

    if !output.status.success() {
        anyhow::bail!("wl-paste exited with status {}", output.status);
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn write_clipboard_text(text: &str) -> Result<()> {
    let mut child = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to run wl-copy")?;

    child
        .stdin
        .as_mut()
        .context("failed to open wl-copy stdin")?
        .write_all(text.as_bytes())
        .context("failed to write to wl-copy")?;

    let status = child.wait().context("failed to wait for wl-copy")?;
    if !status.success() {
        anyhow::bail!("wl-copy exited with status {status}");
    }

    Ok(())
}

fn ensure_wl_clipboard() -> Result<()> {
    for command in ["wl-copy", "wl-paste"] {
        let status = Command::new("sh")
            .args(["-c", &format!("command -v {command} >/dev/null 2>&1")])
            .status()
            .with_context(|| format!("failed to check for {command}"))?;

        if !status.success() {
            anyhow::bail!("{command} was not found; install wl-clipboard");
        }
    }

    Ok(())
}
