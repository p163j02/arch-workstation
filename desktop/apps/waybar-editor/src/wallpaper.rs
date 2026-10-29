mod login;
mod palette;
mod terminal;
mod theme;

use adw::prelude::*;
use anyhow::{Context, Result, bail};
use gdk_pixbuf::{Pixbuf, PixbufLoader};
use gtk::{gdk, gio, glib};
use std::cell::{Cell, RefCell};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, SystemTime};
use theme::{
    Palette, Paths, Transition, apply_wallpaper, extract_palette, palette_for, save_palette,
};

pub fn run() {
    let args: Vec<String> = env::args()
        .skip(1)
        .filter(|s| s != "--wallpapers")
        .collect();
    if !args.is_empty() {
        if let Err(error) = cli(&args) {
            eprintln!("Wallpaper Studio: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    let app = adw::Application::builder()
        .application_id("io.local.WallpaperStudio")
        .build();
    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
        } else {
            build(app);
        }
    });
    app.run_with_args::<&str>(&[]);
}

fn cli(args: &[String]) -> Result<()> {
    let paths = Paths::discover();
    if args == ["--sync-terminal"] {
        if let Some(image) = paths.read_state()?.current {
            let image = PathBuf::from(image);
            return terminal::sync(
                &paths,
                &palette_for(&paths, &image)?,
                theme::checked_command,
            );
        }
        return Ok(());
    }
    if args == ["--sync-login"] {
        if let Some(image) = paths.read_state()?.current {
            let image = PathBuf::from(image);
            return login::sync(&paths, &image, &palette_for(&paths, &image)?);
        }
        return Ok(());
    }
    let image = match args {
        [flag] if flag == "--help" || flag == "-h" => {
            println!(
                "Wallpaper Studio\n\nUso: wallpaper-studio\n     wallpaper-studio --apply RUTA\n     wallpaper-studio --restore\n     wallpaper-studio --sync-login\n     wallpaper-studio --sync-terminal\n\nFondos estáticos y GIF animados, paletas por imagen y transiciones.\nAplica a todos los monitores. Usa awww-daemon y Hyprland."
            );
            return Ok(());
        }
        [flag] if flag == "--restore" => match paths.read_state()?.current {
            Some(image) => PathBuf::from(image),
            None => {
                let status = Command::new("waypaper")
                    .arg("--restore")
                    .status()
                    .context("Todavía no hay un fondo elegido y no se pudo restaurar Waypaper")?;
                if !status.success() {
                    bail!("Waypaper no pudo restaurar el fondo");
                }
                return Ok(());
            }
        },
        [flag, image] if flag == "--apply" => PathBuf::from(image),
        _ => bail!("Uso: wallpaper-studio [--apply RUTA | --restore | --help]"),
    };
    let palette = palette_for(&paths, &image)?;
    let transition = paths.read_state()?.transition;
    for warning in apply_wallpaper(&paths, &image, &palette, &transition)? {
        eprintln!("{warning}");
    }
    Ok(())
}

#[derive(Clone, Default)]
struct WallpaperLibrary {
    files: Rc<RefCell<Vec<PathBuf>>>,
    generation: Rc<Cell<u64>>,
}

impl WallpaperLibrary {
    fn connect_activation(&self, list: &gtk::ListBox, on_select: impl Fn(PathBuf) + 'static) {
        let files = self.files.clone();
        list.set_activate_on_single_click(true);
        // ListBoxRow::activate is a keybinding action; pointer activation is
        // delivered by the parent ListBox::row-activated signal.
        list.connect_row_activated(move |_, row| {
            let path = usize::try_from(row.index())
                .ok()
                .and_then(|index| files.borrow().get(index).cloned());
            if let Some(path) = path {
                on_select(path);
            }
        });
    }
}

#[derive(Clone)]
struct View {
    picture: gtk::Picture,
    name: gtk::Label,
    status: gtk::Label,
    apply: gtk::Button,
    save: gtk::Button,
    reset: gtk::Button,
    colors: Vec<gtk::ColorDialogButton>,
    selected: Rc<RefCell<Option<PathBuf>>>,
    generation: Rc<Cell<u64>>,
    timer: Rc<RefCell<Option<glib::SourceId>>>,
    paths: Paths,
}

impl View {
    fn palette(&self) -> Palette {
        let colors: Vec<_> = self
            .colors
            .iter()
            .map(|button| {
                let color = button.rgba();
                format!(
                    "#{:02x}{:02x}{:02x}",
                    (color.red() * 255.).round() as u8,
                    (color.green() * 255.).round() as u8,
                    (color.blue() * 255.).round() as u8
                )
            })
            .collect();
        Palette {
            accent: colors[0].clone(),
            secondary: colors[1].clone(),
            background: colors[2].clone(),
            foreground: colors[3].clone(),
        }
    }
    fn set_palette(&self, palette: &Palette) {
        for (button, color) in self.colors.iter().zip([
            &palette.accent,
            &palette.secondary,
            &palette.background,
            &palette.foreground,
        ]) {
            if let Ok(rgba) = gdk::RGBA::parse(color) {
                button.set_rgba(&rgba);
            }
        }
    }
    fn actions(&self, enabled: bool) {
        self.apply.set_sensitive(enabled);
        self.save.set_sensitive(enabled);
        self.reset.set_sensitive(enabled);
        for button in &self.colors {
            button.set_sensitive(enabled);
        }
    }
    fn stop_animation(&self) {
        if let Some(timer) = self.timer.borrow_mut().take() {
            timer.remove();
        }
    }
    fn select(&self, path: PathBuf) {
        self.stop_animation();
        self.picture.set_paintable(gdk::Paintable::NONE);
        self.actions(false);
        *self.selected.borrow_mut() = Some(path.clone());
        self.name
            .set_text(&path.file_name().unwrap_or_default().to_string_lossy());
        self.status.set_text("Cargando vista previa y paleta…");
        self.generation.set(self.generation.get().wrapping_add(1));
        let generation = self.generation.get();
        let view = self.clone();
        let paths = self.paths.clone();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(move || -> Result<_> {
                let palette = palette_for(&paths, &path)?;
                let data = fs::read(&path).context("No se pudo leer la vista previa")?;
                Ok((palette, data))
            })
            .await;
            if view.generation.get() != generation {
                return;
            }
            match result {
                Ok(Ok((palette, data))) => {
                    view.set_palette(&palette);
                    match view.preview(&data) {
                        Ok(animated) => {
                            view.actions(true);
                            view.status.set_text(if animated { "GIF animado · Edita la paleta o aplica el fondo en todos los monitores." } else { "Edita la paleta o aplica el fondo en todos los monitores." });
                        }
                        Err(error) => view
                            .status
                            .set_text(&format!("No se pudo abrir la imagen: {error:#}")),
                    }
                }
                Ok(Err(error)) => view
                    .status
                    .set_text(&format!("No se pudo cargar: {error:#}")),
                Err(_) => view.status.set_text("Falló la tarea de carga."),
            }
        });
    }
    fn preview(&self, data: &[u8]) -> Result<bool> {
        let loader = PixbufLoader::new();
        // Decode the preview at display size instead of retaining full-size GIF frames.
        loader.connect_size_prepared(|loader, width, height| {
            let scale = (960.0 / f64::from(width))
                .min(540.0 / f64::from(height))
                .min(1.0);
            loader.set_size(
                (f64::from(width) * scale).round().max(1.0) as i32,
                (f64::from(height) * scale).round().max(1.0) as i32,
            );
        });
        let written = loader.write(data);
        let closed = loader.close();
        written?;
        closed?;
        let animation = loader
            .animation()
            .context("Formato de imagen no compatible")?;
        let iter = animation.iter(Some(SystemTime::now()));
        self.picture
            .set_paintable(Some(&gdk::Texture::for_pixbuf(&iter.pixbuf())));
        let animated = !animation.is_static_image();
        if animated {
            let weak_picture = self.picture.downgrade();
            let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
                let Some(picture) = weak_picture.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                if picture.is_mapped() && iter.advance(SystemTime::now()) {
                    picture.set_paintable(Some(&gdk::Texture::for_pixbuf(&iter.pixbuf())));
                }
                glib::ControlFlow::Continue
            });
            *self.timer.borrow_mut() = Some(timer);
        }
        Ok(animated)
    }
}

fn build(app: &adw::Application) {
    let paths = Paths::discover();
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Wallpaper Studio")
        .default_width(1100)
        .default_height(780)
        .build();
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "Wallpaper Studio",
        "Fondos y colores del sistema",
    )));
    let add_folder = gtk::Button::with_label("Añadir carpeta");
    header.pack_start(&add_folder);
    let refresh = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh.set_tooltip_text(Some("Volver a buscar fondos"));
    header.pack_start(&refresh);
    root.append(&header);
    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.set_vexpand(true);
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sidebar.set_width_request(320);
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Buscar fondos…"));
    search.set_margin_start(10);
    search.set_margin_end(10);
    sidebar.append(&search);
    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::Single);
    list.add_css_class("navigation-sidebar");
    sidebar.append(
        &gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build(),
    );
    body.append(&sidebar);
    body.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    let detail = gtk::Box::new(gtk::Orientation::Vertical, 16);
    detail.set_margin_top(24);
    detail.set_margin_bottom(24);
    detail.set_margin_start(24);
    detail.set_margin_end(24);
    detail.set_hexpand(true);
    let picture = gtk::Picture::new();
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk::ContentFit::Contain);
    picture.set_size_request(-1, 260);
    picture.set_vexpand(true);
    picture.add_css_class("card");
    detail.append(&picture);
    let name = gtk::Label::new(Some("Elige un fondo"));
    name.add_css_class("title-2");
    name.set_halign(gtk::Align::Start);
    name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    detail.append(&name);
    let swatches = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let mut colors = Vec::new();
    for caption in ["Acento", "Secundario", "Fondo", "Texto"] {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let dialog = gtk::ColorDialog::builder()
            .with_alpha(false)
            .title(caption)
            .build();
        let button = gtk::ColorDialogButton::new(Some(dialog));
        button.set_size_request(88, 42);
        button.set_tooltip_text(Some(&format!("Editar {caption}")));
        let hex = gtk::Label::new(Some("#000000"));
        hex.add_css_class("monospace");
        let hex_label = hex.clone();
        button.connect_rgba_notify(move |button| {
            let color = button.rgba();
            hex_label.set_text(&format!(
                "#{:02x}{:02x}{:02x}",
                (color.red() * 255.).round() as u8,
                (color.green() * 255.).round() as u8,
                (color.blue() * 255.).round() as u8
            ));
        });
        column.append(&button);
        column.append(&hex);
        // The color dialog also exposes a hexadecimal entry for precise edits.
        column.append(&gtk::Label::new(Some(caption)));
        colors.push(button);
        swatches.append(&column);
    }
    detail.append(&swatches);
    let palette_actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let save = gtk::Button::with_label("Guardar paleta");
    let reset = gtk::Button::with_label("Extraer de nuevo");
    palette_actions.append(&save);
    palette_actions.append(&reset);
    detail.append(&palette_actions);
    let transitions = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    transitions.append(&gtk::Label::new(Some("Transición")));
    let effects = ["fade", "wipe", "wave", "center", "outer", "random", "none"];
    let effect = gtk::DropDown::from_strings(&[
        "Fundido",
        "Barrido",
        "Onda",
        "Círculo",
        "Círculo inverso",
        "Aleatoria",
        "Sin transición",
    ]);
    transitions.append(&effect);
    let duration = gtk::SpinButton::with_range(0.1, 10.0, 0.1);
    duration.set_digits(1);
    duration.set_value(2.0);
    transitions.append(&duration);
    transitions.append(&gtk::Label::new(Some("segundos")));
    detail.append(&transitions);
    let apply = gtk::Button::with_label("Aplicar fondo y paleta");
    apply.add_css_class("suggested-action");
    apply.add_css_class("pill");
    apply.set_halign(gtk::Align::Start);
    detail.append(&apply);
    let status = gtk::Label::new(Some("Buscando fondos…"));
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    detail.append(&status);
    body.append(
        &gtk::ScrolledWindow::builder()
            .child(&detail)
            .hexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build(),
    );
    root.append(&body);
    window.set_content(Some(&root));
    let view = Rc::new(View {
        picture,
        name,
        status,
        apply,
        save,
        reset,
        colors,
        selected: Rc::default(),
        generation: Rc::default(),
        timer: Rc::default(),
        paths: paths.clone(),
    });
    view.actions(false);
    let state = paths.read_state();
    let roots = Rc::new(RefCell::new(vec![paths.wallpapers.clone()]));
    match state {
        Ok(state) => {
            roots
                .borrow_mut()
                .extend(state.folders.into_iter().map(PathBuf::from));
            effect.set_selected(
                effects
                    .iter()
                    .position(|e| *e == state.transition.effect)
                    .unwrap_or_default() as u32,
            );
            duration.set_value(state.transition.duration);
        }
        Err(error) => view
            .status
            .set_text(&format!("No se pudo leer el estado: {error:#}")),
    }
    let query = Rc::new(RefCell::new(String::new()));
    let filter = query.clone();
    list.set_filter_func(move |row| {
        row.tooltip_text()
            .is_some_and(|text| text.to_lowercase().contains(&*filter.borrow()))
    });
    let search_list = list.clone();
    search.connect_search_changed(move |entry| {
        *query.borrow_mut() = entry.text().to_lowercase();
        search_list.invalidate_filter();
    });
    let library = WallpaperLibrary::default();
    let selection_view = Rc::downgrade(&view);
    library.connect_activation(&list, move |path| {
        if let Some(view) = selection_view.upgrade() {
            view.select(path);
        }
    });
    load_library(&view, &list, roots.borrow().clone(), &library);
    let refresh_view = view.clone();
    let refresh_list = list.clone();
    let refresh_roots = roots.clone();
    let refresh_library = library.clone();
    refresh.connect_clicked(move |_| {
        load_library(
            &refresh_view,
            &refresh_list,
            refresh_roots.borrow().clone(),
            &refresh_library,
        )
    });
    let folder_view = view.clone();
    let folder_list = list.clone();
    let parent = window.downgrade();
    add_folder.connect_clicked(move |_| {
        let Some(parent) = parent.upgrade() else {
            return;
        };
        let view = folder_view.clone();
        let list = folder_list.clone();
        let roots = roots.clone();
        let library = library.clone();
        gtk::FileDialog::new().select_folder(
            Some(&parent),
            gio::Cancellable::NONE,
            move |result| {
                let Ok(folder) = result else {
                    return;
                };
                let Some(path) = folder.path() else {
                    return;
                };
                let path = path.canonicalize().unwrap_or(path);
                let Some(key) = path.to_str() else {
                    view.status
                        .set_text("La carpeta debe tener una ruta UTF-8.");
                    return;
                };
                if let Err(error) = view.paths.update_state(|state| {
                    state.folders.push(key.to_owned());
                    state.folders.sort();
                    state.folders.dedup();
                }) {
                    view.status
                        .set_text(&format!("No se pudo guardar la carpeta: {error:#}"));
                    return;
                }
                roots.borrow_mut().push(path);
                load_library(&view, &list, roots.borrow().clone(), &library);
            },
        );
    });
    let save_view = Rc::downgrade(&view);
    view.save.connect_clicked(move |_| {
        let Some(save_view) = save_view.upgrade() else {
            return;
        };
        let Some(path) = save_view.selected.borrow().clone() else {
            return;
        };
        match save_palette(&save_view.paths, &path, &save_view.palette()) {
            Ok(()) => save_view.status.set_text(
                "Paleta guardada para este fondo. Pulsa Aplicar para actualizar el sistema.",
            ),
            Err(error) => save_view
                .status
                .set_text(&format!("No se pudo guardar: {error:#}")),
        }
    });
    let reset_view = Rc::downgrade(&view);
    view.reset.connect_clicked(move |_| {
        let Some(reset_view) = reset_view.upgrade() else {
            return;
        };
        let Some(path) = reset_view.selected.borrow().clone() else {
            return;
        };
        let view = reset_view.clone();
        let generation = view.generation.get();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(move || extract_palette(&path)).await;
            if view.generation.get() != generation {
                return;
            }
            match result {
                Ok(Ok(palette)) => {
                    view.set_palette(&palette);
                    view.status
                        .set_text("Paleta extraída. Guarda o aplica para conservarla.");
                }
                Ok(Err(error)) => view
                    .status
                    .set_text(&format!("No se pudo extraer: {error:#}")),
                Err(_) => view.status.set_text("Falló la extracción de colores."),
            }
        });
    });
    let apply_view = Rc::downgrade(&view);
    let busy = Rc::new(Cell::new(false));
    let apply_busy = busy.clone();
    view.apply.connect_clicked(move |_| {
        let Some(apply_view) = apply_view.upgrade() else {
            return;
        };
        let Some(image) = apply_view.selected.borrow().clone() else {
            return;
        };
        let view = apply_view.clone();
        view.generation.set(view.generation.get().wrapping_add(1));
        let palette = view.palette();
        let transition = Transition {
            effect: effects[effect.selected() as usize].into(),
            duration: duration.value(),
        };
        let paths = view.paths.clone();
        let body = body.clone();
        let header = header.clone();
        body.set_sensitive(false);
        header.set_sensitive(false);
        view.status.set_text(
            "Aplicando fondo y colores… El primer GIF puede tardar mientras awww lo prepara.",
        );
        apply_busy.set(true);
        let busy = apply_busy.clone();
        glib::spawn_future_local(async move {
            let result =
                gio::spawn_blocking(move || apply_wallpaper(&paths, &image, &palette, &transition))
                    .await;
            busy.set(false);
            body.set_sensitive(true);
            header.set_sensitive(true);
            match result {
                Ok(Ok(warnings)) if warnings.is_empty() => view
                    .status
                    .set_text("Fondo y paleta aplicados a Waybar, Eww y los bordes de Hyprland."),
                Ok(Ok(warnings)) => view.status.set_text(&warnings.join("\n")),
                Ok(Err(error)) => view
                    .status
                    .set_text(&format!("No se pudo aplicar: {error:#}")),
                Err(_) => view.status.set_text("Falló la tarea de aplicación."),
            }
        });
    });
    window.connect_close_request(move |_| {
        if busy.get() {
            view.status
                .set_text("Espera a que termine de aplicarse el fondo antes de cerrar.");
            glib::Propagation::Stop
        } else {
            view.stop_animation();
            glib::Propagation::Proceed
        }
    });
    window.present();
}

fn load_library(view: &View, list: &gtk::ListBox, roots: Vec<PathBuf>, library: &WallpaperLibrary) {
    let generation = library.generation.get().wrapping_add(1);
    library.generation.set(generation);
    let view = view.clone();
    let list = list.clone();
    let library = library.clone();
    view.status
        .set_text("Buscando fondos y preparando miniaturas…");
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            let mut files = Vec::new();
            for root in roots {
                collect_images(&root, &mut files, 0);
            }
            files.sort();
            files.dedup();
            files
                .into_iter()
                .map(|path| {
                    let thumbnail = Pixbuf::from_file_at_scale(&path, 112, 64, true)
                        .map(|pixbuf| {
                            (
                                pixbuf.read_pixel_bytes().as_ref().to_vec(),
                                pixbuf.width(),
                                pixbuf.height(),
                                pixbuf.rowstride() as usize,
                                pixbuf.has_alpha(),
                            )
                        })
                        .ok();
                    (path, thumbnail)
                })
                .collect::<Vec<_>>()
        })
        .await;
        if library.generation.get() != generation {
            return;
        }
        let files = match result {
            Ok(files) => files,
            Err(_) => {
                view.status.set_text("No se pudo cargar la biblioteca.");
                return;
            }
        };
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        *library.files.borrow_mut() = files.iter().map(|(path, _)| path.clone()).collect();
        let count = files.len();
        let had_selection = view.selected.borrow().is_some();
        let current = view.selected.borrow().clone().or_else(|| {
            view.paths
                .read_state()
                .ok()
                .and_then(|s| s.current.map(PathBuf::from))
        });
        let mut matched = false;
        for (path, thumbnail) in files {
            let row = gtk::ListBoxRow::new();
            row.set_tooltip_text(Some(&path.to_string_lossy()));
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            line.set_margin_top(6);
            line.set_margin_bottom(6);
            let thumb = gtk::Picture::new();
            if let Some((data, width, height, stride, alpha)) = thumbnail {
                let format = if alpha {
                    gdk::MemoryFormat::R8g8b8a8
                } else {
                    gdk::MemoryFormat::R8g8b8
                };
                let texture = gdk::MemoryTexture::new(
                    width,
                    height,
                    format,
                    &glib::Bytes::from_owned(data),
                    stride,
                );
                thumb.set_paintable(Some(&texture));
            }
            thumb.set_content_fit(gtk::ContentFit::Cover);
            thumb.set_can_shrink(true);
            thumb.set_size_request(112, 64);
            line.append(&thumb);
            let labels = gtk::Box::new(gtk::Orientation::Vertical, 4);
            labels.set_valign(gtk::Align::Center);
            let label = gtk::Label::new(Some(
                &path.file_name().unwrap_or_default().to_string_lossy(),
            ));
            label.set_halign(gtk::Align::Start);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_max_width_chars(21);
            labels.append(&label);
            if path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("gif"))
            {
                let animated = gtk::Label::new(Some("GIF · animado"));
                animated.add_css_class("dim-label");
                animated.set_halign(gtk::Align::Start);
                labels.append(&animated);
            }
            line.append(&labels);
            row.set_child(Some(&line));
            let selected = current.as_ref() == Some(&path);
            list.append(&row);
            if selected {
                matched = true;
                list.select_row(Some(&row));
                if !had_selection {
                    list.emit_by_name::<()>("row-activated", &[&row]);
                }
            }
        }
        if had_selection && !matched {
            view.generation.set(view.generation.get().wrapping_add(1));
            view.stop_animation();
            view.picture.set_paintable(gdk::Paintable::NONE);
            view.actions(false);
            view.name.set_text("Elige un fondo");
            *view.selected.borrow_mut() = None;
        }
        if view.selected.borrow().is_none() {
            view.status.set_text(&if count == 0 {
                "No hay imágenes. Añade una carpeta con fondos estáticos o GIF.".into()
            } else {
                format!("{count} fondos disponibles. Selecciona uno para ver su paleta.")
            });
        }
    });
}

fn collect_images(root: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            collect_images(&path, out, depth + 1);
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
            ["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff"]
                .iter()
                .any(|ext| s.eq_ignore_ascii_case(ext))
        }) {
            out.push(path.canonicalize().unwrap_or(path));
        }
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    #[ignore = "requires a GTK display; can run with the Broadway backend"]
    fn thumbnail_activation_uses_parent_signal_and_preserves_paths_after_filter_and_refresh() {
        gtk::init().expect("GTK display required for the thumbnail activation regression test");
        let list = gtk::ListBox::new();
        let library = WallpaperLibrary::default();
        *library.files.borrow_mut() = vec![
            PathBuf::from("/images/a/same.png"),
            PathBuf::from("/images/b/same.png"),
        ];
        let selections = Rc::new(RefCell::new(Vec::new()));
        let received = selections.clone();
        library.connect_activation(&list, move |path| received.borrow_mut().push(path));
        assert!(list.activates_on_single_click());
        let first = gtk::ListBoxRow::new();
        let second = gtk::ListBoxRow::new();
        list.append(&first);
        list.append(&second);
        // This is the signal GTK emits for a mouse activation, unlike the
        // row's keybinding action that originally left clicks unhandled.
        list.emit_by_name::<()>("row-activated", &[&first]);
        list.set_filter_func(|row| row.index() == 1);
        list.emit_by_name::<()>("row-activated", &[&second]);
        assert_eq!(
            *selections.borrow(),
            [
                PathBuf::from("/images/a/same.png"),
                PathBuf::from("/images/b/same.png")
            ]
        );
        list.remove(&first);
        list.remove(&second);
        *library.files.borrow_mut() = vec![PathBuf::from("/images/new.gif")];
        list.unset_filter_func();
        let replacement = gtk::ListBoxRow::new();
        list.append(&replacement);
        // Keyboard activation also reaches the parent's row-activated signal.
        replacement.emit_by_name::<()>("activate", &[]);
        assert_eq!(
            selections.borrow().last(),
            Some(&PathBuf::from("/images/new.gif"))
        );
        assert_eq!(selections.borrow().len(), 3);
    }
}
