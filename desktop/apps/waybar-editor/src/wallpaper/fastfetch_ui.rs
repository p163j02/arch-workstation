use super::{
    fastfetch,
    fastfetch::PaletteColor,
    theme::{Palette, Paths},
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct Editor {
    pub widget: gtk::Expander,
    controls: Rc<Controls>,
}

struct Controls {
    paths: Paths,
    wallpaper: RefCell<PathBuf>,
    loading: Cell<bool>,
    profile: gtk::DropDown,
    palette: RefCell<Option<Palette>>,
    text_color: ColorPicker,
    key_color: ColorPicker,
    frame_color: ColorPicker,
    fields: gtk::Box,
    image: gtk::Entry,
    heading: gtk::Entry,
    footer: gtk::Entry,
    memory: gtk::Entry,
    disk: gtk::Entry,
    width: gtk::SpinButton,
    preview: gtk::Picture,
    summary: gtk::Label,
}

impl Controls {
    fn settings(&self) -> fastfetch::Settings {
        fastfetch::Settings {
            profile: fastfetch::PROFILES[self.profile.selected() as usize].into(),
            image: self.image.text().to_string(),
            heading: self.heading.text().to_string(),
            footer: self.footer.text().to_string(),
            memory: self.memory.text().to_string(),
            disk: self.disk.text().to_string(),
            width: self.width.value_as_int() as u32,
            text_color: self.text_color.selected(),
            key_color: self.key_color.selected(),
            frame_color: self.frame_color.selected(),
        }
    }

    fn update_color_choices(&self) {
        let loading = self.loading.replace(true);
        if let Some(palette) = self.palette.borrow().as_ref() {
            for picker in [&self.text_color, &self.key_color, &self.frame_color] {
                picker.update_colors(palette);
            }
        }
        self.loading.set(loading);
    }

    fn refresh(&self) {
        if self.loading.get() {
            return;
        }
        let settings = self.settings();
        let wallpaper = self.wallpaper.borrow();
        let preset = fastfetch::profile(&settings, &wallpaper);
        for (entry, default) in [
            (&self.heading, preset.heading),
            (&self.footer, preset.footer),
            (&self.memory, preset.memory),
            (&self.disk, preset.disk),
        ] {
            entry.set_placeholder_text(Some(default));
        }
        self.fields.set_sensitive(settings.profile != "off");
        if settings.profile == "off" {
            self.preview.set_paintable(gtk::gdk::Paintable::NONE);
            self.summary
                .set_text("Este fondo conservará la configuración actual de fastfetch.");
            return;
        }
        let image = fastfetch::image_path(&self.paths, &wallpaper, &settings);
        match gdk_pixbuf::Pixbuf::from_file_at_scale(&image, 144, 144, true) {
            Ok(pixbuf) => {
                self.preview
                    .set_paintable(Some(&gtk::gdk::Texture::for_pixbuf(&pixbuf)));
                self.summary.set_text(&format!(
                    "{}\n\n{}",
                    fastfetch::text(&settings.heading, preset.heading),
                    fastfetch::text(&settings.footer, preset.footer)
                ));
                if let Some(palette) = self.palette.borrow().as_ref() {
                    let heading = glib::markup_escape_text(fastfetch::text(
                        &settings.heading,
                        preset.heading,
                    ));
                    let footer =
                        glib::markup_escape_text(fastfetch::text(&settings.footer, preset.footer));
                    let key =
                        glib::markup_escape_text(fastfetch::text(&settings.memory, preset.memory));
                    self.summary.set_markup(&format!(
                        "<span foreground=\"{}\">{heading}</span>\n\n<span foreground=\"{}\">{key}</span> │ <span foreground=\"{}\">Texto de ejemplo</span>\n\n<span foreground=\"{}\">{footer}</span>",
                        settings.frame_color.hex(palette), settings.key_color.hex(palette),
                        settings.text_color.hex(palette), settings.frame_color.hex(palette),
                    ));
                }
            }
            Err(_) => {
                self.preview.set_paintable(gtk::gdk::Paintable::NONE);
                self.summary.set_text(&format!(
                    "No se pudo cargar la imagen:\n{}",
                    image.display()
                ));
            }
        }
    }
}

impl Editor {
    pub fn new(paths: &Paths, window: &adw::ApplicationWindow) -> Self {
        let widget = gtk::Expander::new(Some("Personalizar fastfetch"));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(12);
        let profile = gtk::DropDown::from_strings(&[
            "Automático según el nombre del fondo",
            "Volver al futuro",
            "Espacio",
            "Naturaleza",
            "Arch / neutro",
            "No cambiar fastfetch",
        ]);
        profile.set_tooltip_text(Some("Puedes asignar el mismo perfil a varios fondos."));
        content.append(&profile);
        let fields = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let text_color = color_choice(
            &fields,
            "Color del texto de los datos",
            PaletteColor::Foreground,
        );
        let key_color = color_choice(&fields, "Color de las etiquetas", PaletteColor::Accent);
        let frame_color = color_choice(
            &fields,
            "Color de títulos y marcos",
            PaletteColor::Secondary,
        );
        let color_note = gtk::Label::new(Some(
            "Elige entre los 16 colores que muestra fastfetch. La primera fila contiene los normales; la segunda, los brillantes. Puedes ver cada color antes de seleccionarlo.",
        ));
        color_note.set_wrap(true);
        color_note.set_xalign(0.0);
        color_note.add_css_class("dim-label");
        fields.append(&color_note);
        let preview_row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        let preview = gtk::Picture::new();
        preview.set_size_request(144, 144);
        preview.set_content_fit(gtk::ContentFit::Contain);
        preview.set_can_shrink(true);
        preview_row.append(&preview);
        let summary = gtk::Label::new(None);
        summary.set_wrap(true);
        summary.set_xalign(0.0);
        summary.add_css_class("monospace");
        summary.set_hexpand(true);
        preview_row.append(&summary);
        content.append(&preview_row);
        let image_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let image = gtk::Entry::new();
        image.set_placeholder_text(Some("Imagen del perfil"));
        image.set_hexpand(true);
        image.set_tooltip_text(Some("Deja vacío para usar la imagen del perfil."));
        let library = gtk::Button::with_label("Biblioteca…");
        let choose = gtk::Button::with_label("Otra imagen…");
        image_row.append(&image);
        image_row.append(&library);
        image_row.append(&choose);
        fields.append(&image_row);
        let heading = entry(&fields, "Título");
        let footer = entry(&fields, "Frase final");
        let memory = entry(&fields, "Etiqueta de memoria");
        let disk = entry(&fields, "Etiqueta de disco");
        let width_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        width_row.append(&gtk::Label::new(Some("Ancho de la imagen")));
        let width = gtk::SpinButton::with_range(16.0, 64.0, 1.0);
        width.set_value(28.0);
        width_row.append(&width);
        width_row.append(&gtk::Label::new(Some("columnas")));
        fields.append(&width_row);
        let reset = gtk::Button::with_label("Usar valores del perfil");
        reset.set_halign(gtk::Align::Start);
        fields.append(&reset);
        content.append(&fields);
        let note = gtk::Label::new(Some(
            "Deja los campos vacíos para usar los textos del perfil. Los ajustes se guardan por fondo al guardar o aplicar. El resultado aparece la próxima vez que ejecutes fastfetch.",
        ));
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.add_css_class("dim-label");
        content.append(&note);
        widget.set_child(Some(&content));
        let controls = Rc::new(Controls {
            paths: paths.clone(),
            wallpaper: RefCell::default(),
            loading: Cell::new(false),
            profile,
            palette: RefCell::default(),
            text_color,
            key_color,
            frame_color,
            fields,
            image,
            heading,
            footer,
            memory,
            disk,
            width,
            preview,
            summary,
        });
        for field in [
            &controls.image,
            &controls.heading,
            &controls.footer,
            &controls.memory,
            &controls.disk,
        ] {
            let weak = Rc::downgrade(&controls);
            field.connect_changed(move |_| {
                if let Some(c) = weak.upgrade() {
                    c.refresh();
                }
            });
        }
        let weak = Rc::downgrade(&controls);
        controls.profile.connect_selected_notify(move |_| {
            if let Some(c) = weak.upgrade() {
                c.refresh();
            }
        });
        for picker in [
            &controls.text_color,
            &controls.key_color,
            &controls.frame_color,
        ] {
            for button in std::iter::once(&picker.default_button).chain(picker.buttons.iter()) {
                let weak = Rc::downgrade(&controls);
                button.connect_toggled(move |button| {
                    if button.is_active()
                        && let Some(c) = weak.upgrade()
                    {
                        c.refresh();
                    }
                });
            }
        }
        let weak = Rc::downgrade(&controls);
        reset.connect_clicked(move |_| {
            if let Some(c) = weak.upgrade() {
                c.loading.set(true);
                for entry in [&c.image, &c.heading, &c.footer, &c.memory, &c.disk] {
                    entry.set_text("");
                }
                c.width.set_value(28.0);
                c.text_color.select(PaletteColor::Foreground);
                c.key_color.select(PaletteColor::Accent);
                c.frame_color.select(PaletteColor::Secondary);
                c.loading.set(false);
                c.refresh();
            }
        });
        let weak = Rc::downgrade(&controls);
        let parent = window.downgrade();
        library.connect_clicked(move |_| {
            let (Some(parent), Some(c)) = (parent.upgrade(), weak.upgrade()) else {
                return;
            };
            let wallpaper = c.wallpaper.borrow().clone();
            let settings = c.settings();
            let image = fastfetch::image_path(&c.paths, &wallpaper, &settings);
            let profile = fastfetch::profile(&settings, &wallpaper);
            let weak = Rc::downgrade(&c);
            if let Err(error) = super::fastfetch_gallery::show(
                &parent,
                &c.paths,
                profile.id,
                &image,
                move |image| {
                    if let Some(c) = weak.upgrade()
                        && *c.wallpaper.borrow() == wallpaper
                    {
                        c.image.set_text(&image.to_string_lossy());
                    }
                },
            ) {
                c.summary
                    .set_text(&format!("No se pudo abrir la biblioteca: {error:#}"));
            }
        });
        let weak = Rc::downgrade(&controls);
        let parent = window.downgrade();
        choose.connect_clicked(move |_| {
            let Some(parent) = parent.upgrade() else {
                return;
            };
            let weak = weak.clone();
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Imágenes"));
            filter.add_pixbuf_formats();
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            let Some(c) = weak.upgrade() else {
                return;
            };
            let wallpaper = c.wallpaper.borrow().clone();
            let dialog = gtk::FileDialog::builder()
                .title("Imagen para fastfetch")
                .filters(&filters)
                .initial_folder(&gio::File::for_path(c.paths.fastfetch.join("images")))
                .build();
            dialog.open(Some(&parent), gio::Cancellable::NONE, move |result| {
                if let (Some(c), Ok(file)) = (weak.upgrade(), result)
                    && let Some(path) = file.path()
                    && *c.wallpaper.borrow() == wallpaper
                {
                    c.image.set_text(&path.to_string_lossy());
                }
            });
        });
        Self { widget, controls }
    }

    pub fn load(&self, wallpaper: &Path, settings: &fastfetch::Settings) {
        let c = &self.controls;
        c.loading.set(true);
        *c.wallpaper.borrow_mut() = wallpaper.to_owned();
        c.profile.set_selected(
            fastfetch::PROFILES
                .iter()
                .position(|p| *p == settings.profile)
                .unwrap_or(0) as u32,
        );
        for (entry, value) in [
            (&c.image, &settings.image),
            (&c.heading, &settings.heading),
            (&c.footer, &settings.footer),
            (&c.memory, &settings.memory),
            (&c.disk, &settings.disk),
        ] {
            entry.set_text(value);
        }
        c.width.set_value(settings.width.into());
        c.text_color.select(settings.text_color);
        c.key_color.select(settings.key_color);
        c.frame_color.select(settings.frame_color);
        c.loading.set(false);
        c.refresh();
    }

    pub fn settings(&self) -> fastfetch::Settings {
        self.controls.settings()
    }

    pub fn set_palette(&self, palette: &Palette) {
        *self.controls.palette.borrow_mut() = Some(palette.clone());
        self.controls.update_color_choices();
        self.controls.refresh();
    }
}

struct ColorPicker {
    default: PaletteColor,
    default_button: gtk::ToggleButton,
    default_swatch: gtk::DrawingArea,
    default_label: gtk::Label,
    buttons: Vec<gtk::ToggleButton>,
    swatches: Vec<gtk::DrawingArea>,
}

impl ColorPicker {
    fn selected(&self) -> PaletteColor {
        self.buttons
            .iter()
            .position(|button| button.is_active())
            .map(|index| PaletteColor::ANSI[index])
            .unwrap_or(self.default)
    }

    fn select(&self, color: PaletteColor) {
        if color == self.default {
            self.default_button.set_active(true);
        } else {
            self.buttons[color.ansi_index()].set_active(true);
        }
    }

    fn update_colors(&self, palette: &Palette) {
        let colors = super::terminal::ansi_colors(palette);
        for (index, (button, swatch)) in self.buttons.iter().zip(&self.swatches).enumerate() {
            paint_swatch(swatch, &colors[index]);
            button.set_tooltip_text(Some(&format!(
                "Color {index} · {} · {}",
                if index < 8 { "normal" } else { "brillante" },
                colors[index]
            )));
        }
        let color = self.default.hex(palette);
        paint_swatch(&self.default_swatch, &color);
        self.default_label
            .set_text(&format!("Predeterminado · {color}"));
    }
}

fn paint_swatch(swatch: &gtk::DrawingArea, hex: &str) {
    let color = gtk::gdk::RGBA::parse(hex).expect("validated palette color");
    swatch.set_draw_func(move |_, context, width, height| {
        context.rectangle(1.0, 1.0, f64::from(width - 2), f64::from(height - 2));
        context.set_source_rgb(
            color.red().into(),
            color.green().into(),
            color.blue().into(),
        );
        let _ = context.fill_preserve();
        context.set_source_rgba(1.0, 1.0, 1.0, 0.4);
        context.set_line_width(1.0);
        let _ = context.stroke();
    });
}

fn color_choice(parent: &gtk::Box, title: &str, default: PaletteColor) -> ColorPicker {
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    parent.append(&label);
    let default_button = gtk::ToggleButton::new();
    default_button.set_halign(gtk::Align::Start);
    let default_content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let default_swatch = gtk::DrawingArea::new();
    default_swatch.set_content_width(26);
    default_swatch.set_content_height(20);
    let default_label = gtk::Label::new(Some("Predeterminado"));
    default_content.append(&default_swatch);
    default_content.append(&default_label);
    default_button.set_child(Some(&default_content));
    default_button.set_active(true);
    parent.append(&default_button);
    let grid = gtk::Grid::builder()
        .column_spacing(4)
        .row_spacing(4)
        .halign(gtk::Align::Start)
        .build();
    let mut buttons = Vec::new();
    let mut swatches = Vec::new();
    for index in 0..16 {
        let swatch = gtk::DrawingArea::new();
        swatch.set_content_width(24);
        swatch.set_content_height(20);
        let button = gtk::ToggleButton::new();
        button.set_group(Some(&default_button));
        button.set_child(Some(&swatch));
        button.set_tooltip_text(Some(&format!("Color {index}")));
        grid.attach(&button, index % 8, index / 8, 1, 1);
        buttons.push(button);
        swatches.push(swatch);
    }
    parent.append(&grid);
    ColorPicker {
        default,
        default_button,
        default_swatch,
        default_label,
        buttons,
        swatches,
    }
}

fn entry(parent: &gtk::Box, title: &str) -> gtk::Entry {
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    parent.append(&label);
    let entry = gtk::Entry::new();
    entry.set_max_length(48);
    parent.append(&entry);
    entry
}
