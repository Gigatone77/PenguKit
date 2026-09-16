#!/usr/bin/env python3
"""PenguKit GUI - a WolvenKit-style front-end for the `pengu` CLI.

Layout mirrors WolvenKit: a dark theme, a left navigation rail of icon
buttons, and a Home dashboard of action tiles. Every action shells out to
the `pengu` binary (headless CLI) and streams its output into a console pane.

Launch:
    pengukit                  # GUI (default)
    pengukit <pengu args>     # headless CLI passthrough (e.g. pengukit hash)
    python3 gui/pengu_gui.py --self-test   # headless plumbing check

Dependencies: python3-gobject + GTK4 + libadwaita (all present on Bazzite).
"""

import os
import shutil
import subprocess
import sys
import threading

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.abspath(os.path.join(HERE, ".."))

APP_ID = "org.pengukit.app"
ACCENT = "#f5a623"  # shared EXALTED amber (matches biblelearn/EXALTED)


# --------------------------------------------------------------------------- #
#  backend plumbing                                                            #
# --------------------------------------------------------------------------- #
def find_pengu():
    env = os.environ.get("PENGU_BIN")
    if env and os.path.isfile(env):
        return env
    candidates = [
        os.path.join(REPO_ROOT, "pengu"),
        os.path.join(REPO_ROOT, "bin", "pengu"),
        os.path.join(HERE, "pengu"),
        os.path.join(HERE, "bin", "pengu"),
        os.path.join(REPO_ROOT, "target", "release", "pengu"),
        os.path.join(REPO_ROOT, "target", "debug", "pengu"),
        shutil.which("pengu"),
    ]
    for cand in candidates:
        if cand and os.access(cand, os.X_OK):
            return cand
    return None


def native_dir():
    env = os.environ.get("PENGU_NATIVE_DIR")
    if env:
        return env
    for cand in (os.path.join(HERE, "natives"), os.path.join(REPO_ROOT, "natives")):
        if os.path.isdir(cand):
            return os.path.abspath(cand)
    return None


class Runner:
    """Run a pengu command in a worker thread; stream output to the GUI."""

    def __init__(self, binary, natives):
        self.binary = binary
        self.natives = natives

    def start(self, args, on_line, on_done):
        threading.Thread(target=self._task, args=(list(args), on_line, on_done), daemon=True).start()

    def _task(self, args, on_line, on_done):
        env = dict(os.environ)
        if self.natives:
            env["PENGU_NATIVE_DIR"] = self.natives
        rc = -1
        try:
            proc = subprocess.Popen(
                [self.binary, *args],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                bufsize=1,
                env=env,
            )
            if proc.stdout:
                for line in proc.stdout:
                    GLib.idle_add(on_line, line.rstrip("\n"))
            rc = proc.wait()
        except Exception as exc:  # noqa: BLE001 - surface any failure to the pane
            GLib.idle_add(on_line, f"ERROR: {exc}")
        GLib.idle_add(on_done, rc)


# --------------------------------------------------------------------------- #
#  UI                                                                          #
# --------------------------------------------------------------------------- #
CSS = """
@define-color accent %s;
@define-color c-base #0d1117;
@define-color c-panel #161b22;
@define-color c-panel2 #21262d;
@define-color c-rail #131820;
@define-color c-border #30363d;
@define-color c-text #e6edf3;
@define-color c-muted #8b949e;
@define-color c-console #0d1117;

window { background-color: @c-base; color: @c-text; }

.rail { background-color: @c-rail; border-right: 1px solid @c-border; }
.rail-brand { padding: 14px 0 6px 0; }
.rail-button {
  background-color: transparent; border-radius: 8px; margin: 2px 8px;
  color: @c-text; font-size: 12px; font-weight: 600; padding: 8px 6px;
}
.rail-button:hover { background-color: @c-panel2; }
.rail-button:checked {
  background-color: alpha(@accent, 0.16); color: #ffd479;
  box-shadow: inset 0 0 0 1px alpha(@accent, 0.9);
}
.brand-dot { background-color: @accent; border-radius: 999px; }

.home-title { font-size: 34px; font-weight: 800; }
.home-subtitle { color: @c-muted; font-size: 14px; }
.chip { border-radius: 999px; padding: 3px 10px; font-size: 11px; font-weight: 700; }
.chip-ok { background-color: alpha(#3fb950, 0.16); color: #3fb950; }
.chip-bad { background-color: alpha(#f85149, 0.18); color: #f85149; }

.tile {
  background-color: @c-panel; border: 1px solid @c-border; border-radius: 12px;
  min-width: 164px; min-height: 118px; padding: 18px; font-weight: 700; font-size: 14px;
}
.tile:hover { border-color: alpha(@accent, 0.85); background-color: @c-panel2; }
.tile-icon { color: alpha(@accent, 0.95); }

.card {
  background-color: @c-panel; border: 1px solid @c-border; border-radius: 12px;
  padding: 16px; margin: 8px 0;
}
.page-header { font-size: 20px; font-weight: 800; }
.field-label { color: @c-muted; font-size: 12px; font-weight: 700; }

.output {
  background-color: @c-console; color: #79c0ff; font-family: "JetBrains Mono",
  "Fira Code", "DejaVu Sans Mono", monospace; font-size: 12px; padding: 10px;
}
.output-label { color: @c-muted; font-weight: 800; font-size: 12px; }
"""


class ConsolePane(Gtk.Box):
    """A scrolling read-only console with a small header bar."""

    def __init__(self, title):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=6)

        head = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        title_lbl = Gtk.Label(label=title, css_classes=["output-label"], xalign=0.0)
        title_lbl.set_hexpand(True)
        clear = Gtk.Button.new_from_icon_name("edit-clear-all-symbolic")
        clear.set_tooltip_text("Clear console")
        clear.connect("clicked", self._clear)
        head.append(title_lbl)
        head.append(clear)

        self._buffer = Gtk.TextBuffer()
        self._view = Gtk.TextView(buffer=self._buffer, editable=False, wrap_mode=Gtk.WrapMode.WORD)
        self._view.add_css_class("output")
        scroller = Gtk.ScrolledWindow(vscrollbar_policy=Gtk.PolicyType.ALWAYS)
        scroller.set_vexpand(True)
        scroller.set_hexpand(True)
        scroller.set_child(self._view)
        self.append(head)
        self.append(scroller)

    def _clear(self, *_):
        self._buffer.delete(self._buffer.get_start_iter(), self._buffer.get_end_iter())

    def write(self, text):
        return GLib.idle_add(self._append, text)

    def _append(self, text):
        self._buffer.insert(self._buffer.get_end_iter(), text + "\n")
        self._view.scroll_to_iter(self._buffer.get_end_iter(), 0.0, False, 0.0, 0.0)


class PenguKitWindow(Adw.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title="PenguKit")
        self.set_default_size(980, 660)
        self.set_icon_name("pengukit")

        self.pengu = find_pengu()
        self.natives = native_dir()
        self.runner = Runner(self.pengu, self.natives)

        self.arch_path = None
        self.out_path = None
        self.pack_path = None

        self._build()
        self._boot_check()

    # ------------------------------------------------------------- nav ------
    def _rail(self):
        labels = [
            ("go-home-symbolic", "Home", "home"),
            ("folder-open-symbolic", "Extract", "unbundle"),
            ("document-save-symbolic", "Pack", "pack"),
            ("edit-find-symbolic", "Hash", "hash"),
            ("emblem-ok-symbolic", "Self-Check", "check"),
        ]
        rail = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2, css_classes=["rail"])

        self.rail_buttons = []
        self.rail_by_page = {}
        for icon, label, page in labels:
            b = Gtk.ToggleButton()
            vb = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
            vb.append(Gtk.Image(icon_name=icon, pixel_size=22))
            vb.append(Gtk.Label(label=label))
            b.set_child(vb)
            b.add_css_class("rail-button")
            b.set_tooltip_text(page)
            if self.rail_buttons:
                b.set_group(self.rail_buttons[0])
            b.connect("toggled", self._on_rail, page)
            self.rail_buttons.append(b)
            self.rail_by_page[page] = b
            rail.append(b)
        self.rail_buttons[0].set_active(True)
        return rail

    def _on_rail(self, btn, page):
        if btn.get_active() and hasattr(self, "stack"):
            self.stack.set_visible_child_name(page)

    def _nav_to(self, page):
        self.stack.set_visible_child_name(page)
        btn = self.rail_by_page.get(page)
        if btn and not btn.get_active():
            btn.set_active(True)

    def _tile(self, icon, label, page):
        b = Gtk.Button()
        b.add_css_class("tile")
        vb = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        img = Gtk.Image(icon_name=icon, pixel_size=34)
        img.add_css_class("tile-icon")
        vb.append(img)
        vb.append(Gtk.Label(label=label))
        b.set_child(vb)
        b.connect("clicked", lambda _b, n=page: self._nav_to(n))
        return b

    # -------------------------------------------------------------- build ----
    def _build(self):
        toolbar = Adw.ToolbarView()

        header = Adw.HeaderBar()
        header.set_title_widget(Gtk.Label(label="PenguKit"))
        toolbar.add_top_bar(header)

        body = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        body.append(self._rail())

        self.stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.CROSSFADE)
        self.stack.add_named(self._home_page(), "home")
        self.stack.add_named(self._unbundle_page(), "unbundle")
        self.stack.add_named(self._pack_page(), "pack")
        self.stack.add_named(self._hash_page(), "hash")
        self.stack.add_named(self._check_page(), "check")
        self.stack.set_visible_child_name("home")
        body.append(self.stack)
        toolbar.set_content(body)
        self.set_content(toolbar)

    def _home_page(self):
        page = Gtk.ScrolledWindow(vexpand=True)
        clamp = Adw.Clamp(maximum_size=760)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, css_classes=["card"])
        box.set_margin_top(36)
        box.set_margin_bottom(24)

        box.append(Gtk.Label(label="Welcome to PenguKit", css_classes=["home-title"], xalign=0.0))
        box.append(Gtk.Label(
            label="WolvenKit for Linux — REDengine 4 (.archive) toolkit, Rust-native.",
            css_classes=["home-subtitle"], xalign=0.0, wrap=True))

        status = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.home_chip = Gtk.Label(label="checking…", css_classes=["chip"])
        self.home_bin = Gtk.Label(label="", css_classes=["home-subtitle"], xalign=0.0)
        status.append(self.home_chip)
        box.append(status)
        box.append(self.home_bin)

        tiles = Gtk.FlowBox()
        tiles.set_max_children_per_line(4)
        tiles.set_min_children_per_line(2)
        tiles.set_selection_mode(Gtk.SelectionMode.NONE)
        tiles.append(self._tile("folder-open-symbolic", "Extract Archive", "unbundle"))
        tiles.append(self._tile("document-save-symbolic", "Pack Folder", "pack"))
        tiles.append(self._tile("edit-find-symbolic", "Hash a Path", "hash"))
        tiles.append(self._tile("emblem-ok-symbolic", "Self-Check", "check"))
        box.append(tiles)

        clamp.set_child(box)
        page.set_child(clamp)
        return page

    # ------------------------------------------------------------ form bits --
    def _row(self, label, widget):
        row = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        row.append(Gtk.Label(label=label, css_classes=["field-label"], xalign=0.0))
        row.append(widget)
        return row

    def _pick_row(self, label, is_folder, on_picked):
        btn = Gtk.Button(label=label)
        value = Gtk.Label(label="", css_classes=["home-subtitle"], xalign=0.0,
                          ellipsize=3, wrap=True)
        btn.connect("clicked", self._open_picker(is_folder, on_picked))
        row = self._row(label, btn)
        row.append(value)
        return btn, value, row

    def _open_picker(self, is_folder, on_picked):
        def cb(_button):
            dlg = Gtk.FileDialog()
            if is_folder:
                dlg.select_folder(self, None, lambda d, r: _finish(d, r))
            else:
                dlg.open(self, None, lambda d, r: _finish(d, r))

        def _finish(dlg, result):
            try:
                f = dlg.select_folder_finish(result) if is_folder else dlg.open_finish(result)
            except GLib.Error:
                return  # user cancelled
            on_picked(f.get_path())

        return cb

    # --------------------------------------------------------------- pages ----
    def _unbundle_page(self):
        clamp = Adw.Clamp(maximum_size=820)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, css_classes=["card"])

        box.append(Gtk.Label(label="Extract Archive", css_classes=["page-header"], xalign=0.0))
        _b, self.arch_val, arch_row = self._pick_row(
            "Choose .archive file", False, self._arch_picked)
        _b, self.out_val, out_row = self._pick_row(
            "Output folder (default: next to the archive)", True, self._out_picked)
        box.append(arch_row)
        box.append(out_row)

        list_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        list_row.append(Gtk.Label(label="List files only (no extraction)", css_classes=["field-label"]))
        self.list_switch = Gtk.Switch()
        list_row.append(self.list_switch)
        box.append(list_row)

        self.unbund_run = Gtk.Button(label="Run unbundle", css_classes=["suggested-action"])
        self.unbund_run.connect("clicked", self._run_unbundle)
        box.append(self.unbund_run)
        self.unbund_status = Gtk.Label(label="", css_classes=["home-subtitle"], xalign=0.0)
        box.append(self.unbund_status)

        self.unbund_console = ConsolePane("unbundle")
        box.append(self.unbund_console)

        page = Gtk.ScrolledWindow(vexpand=True)
        clamp.set_child(box)
        page.set_child(clamp)
        return page

    def _arch_picked(self, path):
        self.arch_path = path
        self.arch_val.set_text(path)
        if not self.out_val.get_text():
            self.out_val.set_text(os.path.dirname(path))

    def _out_picked(self, path):
        self.out_path = path
        self.out_val.set_text(path)

    def _pack_page(self):
        clamp = Adw.Clamp(maximum_size=820)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, css_classes=["card"])
        box.append(Gtk.Label(label="Pack Folder", css_classes=["page-header"], xalign=0.0))

        _b, self.pack_val, pack_row = self._pick_row(
            "Choose input folder", True, self._pack_picked)
        box.append(pack_row)

        self.pack_out_entry = Gtk.Entry(placeholder_text="e.g. ~/mods/my_mod.archive")
        box.append(self._row("Output .archive", self.pack_out_entry))

        self.pack_run = Gtk.Button(label="Run pack", css_classes=["suggested-action"])
        self.pack_run.connect("clicked", self._run_pack)
        box.append(self.pack_run)
        self.pack_status = Gtk.Label(label="", css_classes=["home-subtitle"], xalign=0.0)
        box.append(self.pack_status)

        self.pack_console = ConsolePane("pack")
        box.append(self.pack_console)

        page = Gtk.ScrolledWindow(vexpand=True)
        clamp.set_child(box)
        page.set_child(clamp)
        return page

    def _pack_picked(self, path):
        self.pack_path = path
        self.pack_val.set_text(path)
        if not self.pack_out_entry.get_text():
            self.pack_out_entry.set_text(path + ".archive")

    def _hash_page(self):
        clamp = Adw.Clamp(maximum_size=820)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, css_classes=["card"])
        box.append(Gtk.Label(label="Hash a Resource Path", css_classes=["page-header"], xalign=0.0))
        box.append(Gtk.Label(
            label="REDengine-4 paths hash to FNV-1a 64 (plus the index CRC-64/XZ). "
                  "Same result as `pengu hash`.",
            css_classes=["home-subtitle"], xalign=0.0, wrap=True))

        entry = Gtk.Entry(placeholder_text="e.g. base\\characters\\head_average.mesh")
        box.append(self._row("Path", entry))

        run = Gtk.Button(label="Hash it", css_classes=["suggested-action"])
        run.connect("clicked", lambda *_a: self._run_hash(entry.get_text()))
        box.append(run)

        self.hash_console = ConsolePane("hash — FNV-1a / red4 / crc64xz")
        box.append(self.hash_console)

        page = Gtk.ScrolledWindow(vexpand=True)
        clamp.set_child(box)
        page.set_child(clamp)
        return page

    def _run_hash(self, text):
        if not text.strip():
            return
        self.hash_console.write("$ pengu hash " + text.strip())
        self._launch(["hash", text.strip()], self.hash_console, None)

    def _check_page(self):
        clamp = Adw.Clamp(maximum_size=820)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, css_classes=["card"])
        box.append(Gtk.Label(label="Self-Check", css_classes=["page-header"], xalign=0.0))
        box.append(Gtk.Label(
            label="Verifies the bundled native libraries (Kraken / Oodle / Wwise) "
                  "and the binary.",
            css_classes=["home-subtitle"], xalign=0.0, wrap=True))
        run = Gtk.Button(label="Run self-check", css_classes=["suggested-action"])
        run.connect("clicked", lambda *_a: self._run_selfcheck())
        box.append(run)

        self.check_console = ConsolePane("self-check")
        box.append(self.check_console)

        page = Gtk.ScrolledWindow(vexpand=True)
        clamp.set_child(box)
        page.set_child(clamp)
        return page

    def _run_selfcheck(self):
        self.check_console.write("$ pengu self-check")
        self._launch(["self-check"], self.check_console, None)

    # ---------------------------------------------------------- command glue --
    def _run_unbundle(self, _btn=None):
        if not self.arch_path:
            self.unbund_status.set_text("Choose an archive first.")
            return
        args = ["unbundle", self.arch_path]
        if self.list_switch.get_active():
            args.append("--list")
            self.unbund_status.set_text("Listing…")
        elif not self.out_val.get_text():
            self.unbund_status.set_text("Choose an output folder.")
            return
        else:
            args += ["-o", self.out_val.get_text()]
            self.unbund_status.set_text("Extracting…")
        self.unbund_console.write("$ pengu " + " ".join(args))
        self._launch(args, self.unbund_console, lambda rc: self.unbund_status.set_text(
            "Done" if rc == 0 else f"Failed (exit {rc})"))

    def _run_pack(self, _btn=None):
        if not self.pack_path:
            self.pack_status.set_text("Choose an input folder first.")
            return
        out = self.pack_out_entry.get_text().strip()
        if not out:
            self.pack_status.set_text("Set an output archive path.")
            return
        args = ["pack", self.pack_path, "-o", os.path.expanduser(out)]
        self.pack_status.set_text("Packing…")
        self.pack_console.write("$ pengu " + " ".join(args))
        self._launch(args, self.pack_console, lambda rc: self.pack_status.set_text(
            "Done" if rc == 0 else f"Failed (exit {rc})"))

    def _launch(self, args, console, on_done):
        if not self.pengu:
            console.write("ERROR: pengu binary not found. Build it (cargo build --release) "
                          "or set PENGU_BIN.")
            if on_done:
                on_done(-1)
            return
        self.runner.start(args, console.write, lambda rc: self._task_done(on_done, rc, console))

    def _task_done(self, on_done, rc, console):
        if on_done:
            on_done(rc)
        if rc != 0:
            console.write(f"exit status: {rc}")

    # --------------------------------------------------------------- startup --
    def _boot_check(self):
        if not self.pengu:
            self.home_chip.set_text("BINARY MISSING")
            self.home_chip.add_css_class("chip-bad")
            self.home_bin.set_text("cargo build --release first (or set PENGU_BIN).")
            return
        self.home_bin.set_text(f"binary: {self.pengu}")
        self.runner.start(["self-check"], lambda _line: None, self._boot_done)

    def _boot_done(self, rc):
        if rc == 0:
            self.home_chip.set_text("READY")
            self.home_chip.add_css_class("chip-ok")
        else:
            self.home_chip.set_text("NATIVE MISSING")
            self.home_chip.add_css_class("chip-bad")
            self.home_bin.set_text("drop natives/ next to the binary or set PENGU_NATIVE_DIR.")


class PenguKitApp(Adw.Application):
    def __init__(self):
        super().__init__(application_id=APP_ID, flags=Gio.ApplicationFlags.DEFAULT_FLAGS)
        self.connect("activate", self._on_activate)

    def _on_activate(self, app):
        Adw.StyleManager.get_default().set_color_scheme(Adw.ColorScheme.FORCE_DARK)
        window = PenguKitWindow(app)
        window.present()


def register_icon_theme():
    icons = os.path.join(REPO_ROOT, "icons")
    if os.path.isdir(icons):
        Gtk.IconTheme.get_for_display(Gdk.Display.get_default()).add_search_path(icons)


def load_css():
    provider = Gtk.CssProvider()
    provider.load_from_string(CSS % ACCENT)
    Gtk.StyleContext.add_provider_for_display(
        Gdk.Display.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)


def run_sync(binary, natives, args):
    """Run a pengu command synchronously (used only by the selftest)."""
    env = dict(os.environ)
    if natives:
        env["PENGU_NATIVE_DIR"] = natives
    try:
        proc = subprocess.run([binary, *args], capture_output=True, text=True, env=env)
        return proc.stdout + proc.stderr, proc.returncode
    except OSError as exc:
        return f"ERROR: {exc}", -1


def selftest():
    """Headless plumbing check: init GTK, build a window, run one real command."""
    result = {}

    def one_shot(_app):
        register_icon_theme()
        load_css()
        win = PenguKitWindow(_app)
        if not win.pengu:
            print("SELFTEST FAIL: pengu binary not found")
            result["code"] = 1
        else:
            out, rc = run_sync(win.pengu, win.natives, ["hash", "base\\foo\\bar.mesh"])
            print("GUI build ......... ok")
            print(f"pengu binary ...... {win.pengu}")
            print(f"natives ........... {win.natives}")
            print(f"hash rc ........... {rc}")
            print(f"hash test ......... {out.splitlines()[0] if out else '(none)'}")
            result["code"] = 0 if rc == 0 else 1
        _app.quit()

    app = Adw.Application(application_id=APP_ID + ".selftest",
                          flags=Gio.ApplicationFlags.DEFAULT_FLAGS)
    app.connect("activate", one_shot)
    app.run()
    return result.get("code", 1)


def main():
    if "--self-test" in sys.argv:
        sys.exit(selftest())
    register_icon_theme()
    load_css()
    app = PenguKitApp()
    app.run()


if __name__ == "__main__":
    main()