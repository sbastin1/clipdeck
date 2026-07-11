# Clipdeck

A small Rust clipboard history utility for Wayland.

The picker uses `gtk4-layer-shell` for rofi-like overlay behavior on compositors that support the `wlr-layer-shell` protocol, such as Hyprland, Sway, Wayfire, river, and other wlroots-based compositors. On compositors without layer-shell support, it falls back to a normal GTK window.

It has two parts:

- `clipdeck daemon` polls the Wayland clipboard and stores text history.
- `clipdeck` opens a GTK picker window. Type to filter, use arrow keys, press Enter to copy the selected item back to the clipboard.

## Requirements

- Rust
- GTK4 development libraries
- gtk4-layer-shell development libraries
- `wl-clipboard` (`wl-copy` and `wl-paste`)

## Install Dependencies

### Arch / EndeavourOS / Manjaro

```sh
sudo pacman -S rust gtk4 gtk4-layer-shell wl-clipboard
```

### Ubuntu

Ubuntu package names can vary by release. On recent Ubuntu versions, start with:

```sh
sudo apt update
sudo apt install cargo rustc libgtk-4-dev wl-clipboard pkg-config
```

You also need the native `gtk4-layer-shell` development package. If your Ubuntu release does not provide it, install/build `gtk4-layer-shell` from your distro packages, a PPA, or from source: <https://github.com/wmww/gtk4-layer-shell>

### Fedora

```sh
sudo dnf install cargo rust gtk4-devel gtk4-layer-shell-devel wl-clipboard pkg-config
```

### openSUSE

```sh
sudo zypper install cargo rust gtk4-devel gtk4-layer-shell-devel wl-clipboard pkg-config
```

## Build

```sh
cargo build --release
```

The compiled binary will be at:

```text
target/release/clipdeck
```

## Run The History Collector

Start the daemon somewhere in your session startup:

```sh
/path/to/clipdeck daemon
```

For example, if you built this repository in `/home/admin/apps/clipdeck`:

```sh
/home/admin/apps/clipdeck/target/release/clipdeck daemon
```

### Hyprland Example

In `~/.config/hypr/hyprland.conf`:

```ini
exec-once = /home/admin/apps/clipdeck/target/release/clipdeck daemon
```

## Open The Picker

Run the binary without arguments:

```sh
/path/to/clipdeck
```

### Hyprland Keybind Example

```ini
bind = $mainMod, V, exec, /home/admin/apps/clipdeck/target/release/clipdeck
```

On layer-shell-capable compositors, the picker should appear as a centered overlay without extra window rules.

## Controls

- Type to search
- `Up` / `Down` selects entries
- `Enter` copies selected entry back to the clipboard and closes the window
- `Escape` closes the window

History is temporary and stored under your session runtime directory:

```text
$XDG_RUNTIME_DIR/clipdeck/history.json
```

It is not stored in your home directory and should disappear after logout or reboot.
