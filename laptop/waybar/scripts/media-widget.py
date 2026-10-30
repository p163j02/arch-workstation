#!/usr/bin/env python3
import gi
gi.require_version('Gtk', '3.0')
gi.require_version('GtkLayerShell', '0.1')
from gi.repository import Gtk, GLib, Gdk, GdkPixbuf, GtkLayerShell
import subprocess, urllib.request, threading, os, sys, signal
from pathlib import Path

CC_ROOT = Path(__file__).resolve().parents[2] / "eww/control-center"
PID_FILE = Path(os.environ.get("XDG_RUNTIME_DIR", f"/run/user/{os.getuid()}")) / "waybar-control-center/media-panel.pid"

def run(cmd):
    try:
        return subprocess.check_output(cmd, shell=True, text=True).strip()
    except:
        return ''

def cmd(command):
    """Fire-and-forget, no bloquea el hilo principal."""
    subprocess.Popen(command, shell=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

CSS = b"""
* { font-family: "JetBrainsMono Nerd Font"; }
window {
    background-color: #1a1b26;
    border-radius: 16px;
    border: 1px solid #292e42;
}
button {
    background: transparent;
    color: #a9b1d6;
    border: none;
    border-radius: 8px;
    font-size: 17px;
    padding: 4px 8px;
    min-height: 0;
    min-width: 0;
}
button:hover { background-color: #292e42; color: #c0caf5; }
#btn-play  { color: #9ece6a; font-size: 20px; }
#btn-play:hover { background-color: #3d59a1; color: #c0caf5; }
#btn-active { color: #7aa2f7; }
/* Botones deshabilitados: opacidad baja, sin hover, cursor normal.
   TODO: quitar esto cuando shuffle/repeat funcionen via Spotify Web API. */
button:disabled { opacity: 0.3; }
button:disabled:hover { background: transparent; color: #a9b1d6; }
#art-placeholder {
    background-color: #1e2030;
    border-radius: 10px;
    color: #3d59a1;
    font-size: 32px;
}
scale trough {
    background-color: #292e42;
    border-radius: 4px;
    min-height: 4px;
}
scale highlight {
    background-color: #7aa2f7;
    border-radius: 4px;
}
scale slider {
    background-color: #c0caf5;
    border-radius: 50%;
    min-width: 10px;
    min-height: 10px;
    margin: -3px;
}
scale { padding: 0; }
"""

class MarqueeLabel(Gtk.DrawingArea):
    def __init__(self, color, font_size=13, bold=False):
        super().__init__()
        self.text = ''
        self.offset = 0.0
        self.text_width = 0
        self.widget_width = 0
        self.pause_counter = 30
        self.font_size = font_size
        self.bold = bold
        self.color = color
        self.set_size_request(190, font_size + 10)
        self.connect('draw', self.on_draw)
        self.connect('size-allocate', lambda w, a: setattr(self, 'widget_width', a.width))

    def set_text(self, text):
        if text == self.text: return
        self.text = text
        self.offset = 0.0
        self.pause_counter = 30
        self.queue_draw()

    def on_draw(self, widget, cr):
        from gi.repository import Pango, PangoCairo
        layout = PangoCairo.create_layout(cr)
        layout.set_text(self.text, -1)
        desc = Pango.FontDescription()
        desc.set_family("JetBrainsMono Nerd Font")
        desc.set_size(int(self.font_size * Pango.SCALE))
        if self.bold: desc.set_weight(Pango.Weight.BOLD)
        layout.set_font_description(desc)
        tw, th = layout.get_pixel_size()
        self.text_width = tw
        cr.rectangle(0, 0, self.widget_width, self.get_allocated_height())
        cr.clip()
        r, g, b = self.color
        cr.set_source_rgb(r, g, b)
        y = (self.get_allocated_height() - th) / 2
        cr.move_to(-self.offset, y)
        PangoCairo.show_layout(cr, layout)
        if tw > self.widget_width:
            cr.move_to(tw + 40 - self.offset, y)
            PangoCairo.show_layout(cr, layout)
        return False

    def tick(self):
        if self.text_width <= self.widget_width:
            self.offset = 0.0
            return
        if self.pause_counter > 0:
            self.pause_counter -= 1
            return
        self.offset += 0.8
        if self.offset >= self.text_width + 40:
            self.offset = 0.0
            self.pause_counter = 20
        self.queue_draw()


class ProgressBar(Gtk.DrawingArea):
    def __init__(self):
        super().__init__()
        self.progress = 0.0
        self.set_size_request(-1, 6)
        self.connect('draw', self.on_draw)
        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        self.on_seek = None

    def set_progress(self, value):
        self.progress = max(0.0, min(1.0, value))
        self.queue_draw()

    def on_draw(self, widget, cr):
        w = self.get_allocated_width()
        h = self.get_allocated_height()
        # Track
        cr.set_source_rgb(0.16, 0.18, 0.26)
        cr.rectangle(0, 0, w, h)
        cr.fill()
        # Fill
        cr.set_source_rgb(0.48, 0.64, 0.97)
        cr.rectangle(0, 0, w * self.progress, h)
        cr.fill()
        # Thumb
        x = w * self.progress
        cr.set_source_rgb(0.75, 0.73, 0.96)
        cr.arc(x, h / 2, 5, 0, 6.28)
        cr.fill()
        return False


class MediaWidget(Gtk.Window):
    def __init__(self, x=12, y=54, width=380, maxheight=260, screen_index=0):
        super().__init__()
        self.set_title("media-widget")
        GtkLayerShell.init_for_window(self)
        GtkLayerShell.set_namespace(self, 'waybar-control-center')
        GtkLayerShell.set_layer(self, GtkLayerShell.Layer.OVERLAY)
        GtkLayerShell.set_anchor(self, GtkLayerShell.Edge.TOP, True)
        GtkLayerShell.set_anchor(self, GtkLayerShell.Edge.LEFT, True)
        GtkLayerShell.set_margin(self, GtkLayerShell.Edge.LEFT, x)
        GtkLayerShell.set_margin(self, GtkLayerShell.Edge.TOP, y)
        GtkLayerShell.set_keyboard_mode(self, GtkLayerShell.KeyboardMode.ON_DEMAND)
        GtkLayerShell.set_exclusive_zone(self, 0)
        display = Gdk.Display.get_default()
        if display is not None and 0 <= screen_index < display.get_n_monitors():
            GtkLayerShell.set_monitor(self, display.get_monitor(screen_index))
        self.set_size_request(width, -1)
        self.set_decorated(False)
        self.set_keep_above(True)
        self.set_resizable(False)
        self.set_type_hint(Gdk.WindowTypeHint.POPUP_MENU)
        screen = self.get_screen()
        visual = screen.get_rgba_visual()
        if visual: self.set_visual(visual)
        self.set_app_paintable(True)

        provider = Gtk.CssProvider()
        provider.load_from_data(CSS)
        Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)

        self.get_style_context().add_class('pc-settings-media')
        self.settings_theme = Gtk.CssProvider()
        Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), self.settings_theme, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION + 1)
        self.settings_signature = None
        self.update_settings_theme()
        GLib.timeout_add(500, self.update_settings_theme)
        self.current_art_url = ''
        self.duration = 0
        self.build_ui()
        self.update_info()
        GLib.timeout_add(1000, self.update_progress)
        GLib.timeout_add(2000, self.update_info)
        GLib.timeout_add(40,   self.tick_marquee)
        self.connect('key-press-event', self.on_key)
        self.connect('delete-event', lambda *_: self.request_close())

    def request_close(self):
        subprocess.Popen([str(CC_ROOT / 'scripts/popup-manager.py'), 'close'],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return True

    def on_key(self, _widget, event):
        if event.keyval == Gdk.KEY_Escape:
            return self.request_close()
        return False

    def update_settings_theme(self):
        path = CC_ROOT / 'wallpaper-studio.scss'
        try:
            stat = path.stat()
            signature = (stat.st_ino, stat.st_mtime_ns, stat.st_size)
            if signature != self.settings_signature:
                self.settings_theme.load_from_data(path.read_bytes())
                self.settings_signature = signature
        except (OSError, GLib.Error):
            pass
        return True

    def build_ui(self):
        outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        outer.set_margin_top(12)
        outer.set_margin_bottom(12)
        outer.set_margin_start(12)
        outer.set_margin_end(12)

        header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        title = Gtk.Label(label="Multimedia")
        title.set_xalign(0)
        header.pack_start(title, True, True, 0)
        close_button = Gtk.Button(label="󰅖")
        close_button.set_tooltip_text("Cerrar · Escape")
        close_button.connect('clicked', lambda *_: self.request_close())
        header.pack_end(close_button, False, False, 0)
        outer.pack_start(header, False, False, 0)

        # ── Row 1: art + info ──
        top = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)

        self.art_stack = Gtk.Stack()
        self.art_stack.set_size_request(80, 80)
        self.art_image = Gtk.Image()
        self.art_image.set_size_request(80, 80)
        ph = Gtk.Label(label="󰝚")
        ph.set_name("art-placeholder")
        ph.set_size_request(80, 80)
        self.art_stack.add_named(ph, "placeholder")
        self.art_stack.add_named(self.art_image, "art")
        self.art_stack.set_visible_child_name("placeholder")
        top.pack_start(self.art_stack, False, False, 0)

        info = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        info.set_valign(Gtk.Align.CENTER)

        self.title_m  = MarqueeLabel((0.75, 0.73, 0.96), 13, bold=True)
        self.artist_m = MarqueeLabel((0.34, 0.37, 0.54), 11)
        info.pack_start(self.title_m,  False, False, 0)
        info.pack_start(self.artist_m, False, False, 0)

        # Controls row: shuffle prev play next repeat
        ctrl = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=0)
        ctrl.set_margin_top(6)

        self.btn_shuffle = Gtk.Button(label="󰒝")
        self.btn_prev    = Gtk.Button(label="⏮")
        self.btn_play    = Gtk.Button(label="▶")
        self.btn_play.set_name("btn-play")
        self.btn_next    = Gtk.Button(label="⏭")
        self.btn_repeat  = Gtk.Button(label="󰑖")

        # TODO (shuffle/repeat): el cliente de Spotify para Linux ignora los comandos
        # MPRIS de shuffle y loop — dbus-send y playerctl son aceptados pero sin efecto.
        # Solución futura: implementar via Spotify Web API:
        #   PUT https://api.spotify.com/v1/me/player/shuffle?state=true
        #   PUT https://api.spotify.com/v1/me/player/repeat?state=track|context|off
        # Requiere OAuth2 con scope 'user-modify-playback-state'. Ver:
        #   https://developer.spotify.com/documentation/web-api/reference/toggle-shuffle-for-users-playback
        # Por ahora los botones están deshabilitados.
        self.btn_shuffle.set_sensitive(False)
        self.btn_shuffle.set_tooltip_text("Shuffle no soportado por MPRIS en Linux")
        self.btn_repeat.set_sensitive(False)
        self.btn_repeat.set_tooltip_text("Repeat no soportado por MPRIS en Linux")
        # self.btn_shuffle.connect('clicked', self.toggle_shuffle)  # descomentar con Web API
        # self.btn_repeat.connect('clicked',  self.toggle_repeat)   # descomentar con Web API
        self.btn_prev.connect('clicked',    lambda _: [cmd('playerctl -p spotify previous'),  GLib.timeout_add(400, lambda: self.update_info() and False)])
        self.btn_play.connect('clicked',    lambda _: [cmd('playerctl -p spotify play-pause'), GLib.timeout_add(200, lambda: self.update_info() and False)])
        self.btn_next.connect('clicked',    lambda _: [cmd('playerctl -p spotify next'),       GLib.timeout_add(400, lambda: self.update_info() and False)])

        for b in [self.btn_shuffle, self.btn_prev, self.btn_play, self.btn_next, self.btn_repeat]:
            ctrl.pack_start(b, False, False, 0)

        info.pack_start(ctrl, False, False, 0)
        top.pack_start(info, True, True, 0)
        outer.pack_start(top, False, False, 0)

        # ── Row 2: progress bar + timestamps ──
        outer.pack_start(Gtk.Box(), False, False, 4)  # spacer

        self.progress_bar = ProgressBar()
        self.progress_bar.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        self.progress_bar.connect('button-press-event', self.on_seek)
        outer.pack_start(self.progress_bar, False, False, 0)

        times = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        self.lbl_pos = Gtk.Label(label="0:00")
        self.lbl_pos.set_markup('<span size="9000" color="#565f89">0:00</span>')
        self.lbl_dur = Gtk.Label(label="0:00")
        self.lbl_dur.set_markup('<span size="9000" color="#565f89">0:00</span>')
        times.pack_start(self.lbl_pos, False, False, 0)
        times.pack_end(self.lbl_dur,   False, False, 0)
        outer.pack_start(times, False, False, 2)

        # ── Row 3: volume ──
        vol_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        vol_row.set_margin_top(2)

        lbl_vol = Gtk.Label(label="󰕾")
        lbl_vol.set_markup('<span color="#565f89">󰕾</span>')
        vol_row.pack_start(lbl_vol, False, False, 0)

        self.vol_scale = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1)
        self.vol_scale.set_draw_value(False)
        self.vol_scale.set_hexpand(True)
        self.vol_scale.connect('value-changed', self.on_volume_change)
        vol_row.pack_start(self.vol_scale, True, True, 0)

        self.lbl_vol_pct = Gtk.Label(label="")
        self.lbl_vol_pct.set_markup('<span size="9000" color="#565f89">--</span>')
        vol_row.pack_start(self.lbl_vol_pct, False, False, 0)

        outer.pack_start(vol_row, False, False, 0)
        self.add(outer)

    # ── Callbacks ──

    def toggle_shuffle(self, _):
        # TODO: reemplazar con Spotify Web API cuando esté implementado.
        # El comando MPRIS 'playerctl -p spotify shuffle toggle' es aceptado
        # por D-Bus pero Spotify lo ignora sin cambiar el estado real.
        # Implementación futura:
        #   state = run('playerctl -p spotify shuffle') != 'On'
        #   requests.put('https://api.spotify.com/v1/me/player/shuffle',
        #                params={'state': str(state).lower()},
        #                headers={'Authorization': f'Bearer {self.access_token}'})
        pass

    def toggle_repeat(self, _):
        # TODO: reemplazar con Spotify Web API cuando esté implementado.
        # El comando MPRIS 'playerctl -p spotify loop <mode>' es aceptado
        # por D-Bus pero Spotify lo ignora sin cambiar el estado real.
        # Implementación futura (ciclo None -> Track -> Playlist -> None):
        #   loop = run('playerctl -p spotify loop')  # None | Track | Playlist
        #   next_state = {'None': 'track', 'Track': 'context', 'Playlist': 'off'}.get(loop, 'off')
        #   requests.put('https://api.spotify.com/v1/me/player/repeat',
        #                params={'state': next_state},
        #                headers={'Authorization': f'Bearer {self.access_token}'})
        pass

    def on_seek(self, widget, event):
        w = widget.get_allocated_width()
        ratio = event.x / w
        if self.duration > 0:
            pos = ratio * self.duration
            cmd(f'playerctl -p spotify position {pos:.1f}')
            self.progress_bar.set_progress(ratio)

    def on_volume_change(self, scale):
        vol = scale.get_value() / 100.0
        cmd(f'playerctl -p spotify volume {vol:.2f}')
        self.lbl_vol_pct.set_markup(f'<span size="9000" color="#565f89">{int(vol*100)}%</span>')

    # ── Update ──

    def update_info(self):
        status  = run('playerctl -p spotify status')
        if not status: return True
        title   = run('playerctl -p spotify metadata title')
        artist  = run('playerctl -p spotify metadata artist')
        art_url = run('playerctl -p spotify metadata mpris:artUrl')
        shuffle = run('playerctl -p spotify shuffle')
        loop    = run('playerctl -p spotify loop')
        vol_str = run('playerctl -p spotify volume')
        dur_str = run('playerctl -p spotify metadata mpris:length')  # microseconds

        self.title_m.set_text(title)
        self.artist_m.set_text(artist)
        self.btn_play.set_label("⏸" if status == "Playing" else "▶")

        # Shuffle color — lectura de estado solamente (botón deshabilitado).
        # TODO: reactivar el resaltado visual cuando toggle_shuffle use Web API.
        # if shuffle == 'On':
        #     self.btn_shuffle.set_name("btn-active")
        # else:
        #     self.btn_shuffle.set_name("")

        # Repeat icon + color — lectura de estado solamente (botón deshabilitado).
        # TODO: reactivar el resaltado visual cuando toggle_repeat use Web API.
        # if loop == 'Track':
        #     self.btn_repeat.set_label("󰑘")
        #     self.btn_repeat.set_name("btn-active")
        # elif loop == 'Playlist':
        #     self.btn_repeat.set_label("󰑖")
        #     self.btn_repeat.set_name("btn-active")
        # else:
        #     self.btn_repeat.set_label("󰑖")
        #     self.btn_repeat.set_name("")

        # Duration
        try:
            self.duration = int(dur_str) / 1_000_000
            m, s = divmod(int(self.duration), 60)
            self.lbl_dur.set_markup(f'<span size="9000" color="#565f89">{m}:{s:02d}</span>')
        except: pass

        # Volume (block signal to avoid loop)
        try:
            vol = float(vol_str)
            self.vol_scale.handler_block_by_func(self.on_volume_change)
            self.vol_scale.set_value(vol * 100)
            self.vol_scale.handler_unblock_by_func(self.on_volume_change)
            self.lbl_vol_pct.set_markup(f'<span size="9000" color="#565f89">{int(vol*100)}%</span>')
        except: pass

        # Album art
        if art_url and art_url != self.current_art_url:
            self.current_art_url = art_url
            threading.Thread(target=self.load_art, args=(art_url,), daemon=True).start()

        return True

    def update_progress(self):
        pos_str = run('playerctl -p spotify position')
        try:
            pos = float(pos_str)
            if self.duration > 0:
                self.progress_bar.set_progress(pos / self.duration)
            m, s = divmod(int(pos), 60)
            self.lbl_pos.set_markup(f'<span size="9000" color="#565f89">{m}:{s:02d}</span>')
        except: pass
        return True

    def tick_marquee(self):
        self.title_m.tick()
        self.artist_m.tick()
        return True

    def load_art(self, url):
        try:
            if url.startswith('file://'): path = url[7:]
            else:
                path = '/tmp/waybar_media_art.jpg'
                urllib.request.urlretrieve(url, path)
            pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(path, 80, 80, True)
            GLib.idle_add(self._set_art, pixbuf)
        except Exception as e:
            print(f"Art error: {e}")

    def _set_art(self, pixbuf):
        self.art_image.set_from_pixbuf(pixbuf)
        self.art_stack.set_visible_child_name("art")


if __name__ == '__main__':
    if len(sys.argv) < 6:
        os.execv(str(CC_ROOT / 'scripts/popup-manager.py'), ['popup-manager.py', 'media'])
    x, y, width, maxheight, screen_index = map(int, sys.argv[1:6])
    signal.signal(signal.SIGTERM, lambda *_: Gtk.main_quit())
    signal.signal(signal.SIGINT, lambda *_: Gtk.main_quit())
    app = MediaWidget(x, y, width, maxheight, screen_index)
    app.show_all()
    PID_FILE.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    PID_FILE.write_text(str(os.getpid()))
    try:
        Gtk.main()
    finally:
        PID_FILE.unlink(missing_ok=True)
