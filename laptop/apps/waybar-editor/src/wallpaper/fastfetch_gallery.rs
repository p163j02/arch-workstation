use super::{fastfetch, theme::Paths};
use adw::prelude::*;
use anyhow::Result;
use gtk::{gdk, gio, glib};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

pub(super) fn show(
    parent: &adw::ApplicationWindow,
    paths: &Paths,
    profile: &str,
    selected: &std::path::Path,
    on_select: impl Fn(PathBuf) + 'static,
) -> Result<()> {
    let images = fastfetch::image_library(paths)?;
    let dialog = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Biblioteca de imágenes de fastfetch")
        .default_width(760)
        .default_height(620)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "Imágenes de fastfetch",
        "Originales y colección Silphwave",
    )));
    content.append(&header);
    let themes = ["all", "bttf", "space", "nature", "arch"];
    let filter = gtk::DropDown::from_strings(&[
        "Todos los temas",
        "Volver al futuro",
        "Espacio",
        "Naturaleza",
        "Arch",
    ]);
    filter.set_selected(
        themes
            .iter()
            .position(|value| *value == profile)
            .unwrap_or(0) as u32,
    );
    filter.set_margin_start(18);
    filter.set_margin_end(18);
    content.append(&filter);
    let status = gtk::Label::new(Some("Preparando miniaturas…"));
    status.set_wrap(true);
    status.set_margin_start(18);
    status.set_margin_end(18);
    content.append(&status);
    let flow = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .max_children_per_line(4)
        .min_children_per_line(1)
        .column_spacing(10)
        .row_spacing(10)
        .homogeneous(true)
        .build();
    flow.set_margin_start(16);
    flow.set_margin_end(16);
    flow.set_margin_bottom(16);
    content.append(
        &gtk::ScrolledWindow::builder()
            .child(&flow)
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build(),
    );
    dialog.set_content(Some(&content));
    let profiles: Vec<_> = images.iter().map(|image| image.profile).collect();
    let chosen = Rc::new(Cell::new(filter.selected()));
    let active_filter = chosen.clone();
    flow.set_filter_func(move |child| {
        let index = active_filter.get() as usize;
        index == 0
            || profiles
                .get(child.index() as usize)
                .zip(themes.get(index))
                .is_some_and(|(profile, theme)| profile == theme)
    });
    let filtered_flow = flow.clone();
    filter.connect_selected_notify(move |dropdown| {
        chosen.set(dropdown.selected());
        filtered_flow.invalidate_filter();
    });
    let current = selected.to_path_buf();
    let weak_dialog = dialog.downgrade();
    let on_select = Rc::new(on_select);
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(move || {
            images
                .into_iter()
                .map(|image| {
                    let preview =
                        gdk_pixbuf::Pixbuf::from_file_at_scale(&image.path, 144, 144, true)
                            .map(|pixbuf| {
                                (
                                    pixbuf.read_pixel_bytes().as_ref().to_vec(),
                                    pixbuf.width(),
                                    pixbuf.height(),
                                    pixbuf.rowstride() as usize,
                                    pixbuf.has_alpha(),
                                )
                            })
                            .map_err(|error| error.to_string());
                    (image, preview)
                })
                .collect::<Vec<_>>()
        })
        .await;
        let Some(dialog) = weak_dialog.upgrade() else {
            return;
        };
        let Ok(images) = result else {
            status.set_text("No se pudieron preparar las miniaturas.");
            return;
        };
        let total = images.len();
        let mut missing = 0;
        for (image, preview) in images {
            let button = gtk::Button::new();
            let card = gtk::Box::new(gtk::Orientation::Vertical, 6);
            let picture = gtk::Picture::new();
            picture.set_size_request(144, 144);
            picture.set_can_shrink(true);
            picture.set_content_fit(gtk::ContentFit::Contain);
            match preview {
                Ok((data, width, height, stride, alpha)) => {
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
                    picture.set_paintable(Some(&texture));
                }
                Err(error) => {
                    missing += 1;
                    picture.set_tooltip_text(Some(&error));
                }
            }
            card.append(&picture);
            let label = gtk::Label::new(Some(&image.name));
            label.set_wrap(true);
            label.set_max_width_chars(20);
            card.append(&label);
            if image.path == current {
                button.add_css_class("suggested-action");
            }
            button.set_tooltip_text(Some(&image.path.to_string_lossy()));
            button.set_child(Some(&card));
            let weak_dialog = dialog.downgrade();
            let on_select = on_select.clone();
            button.connect_clicked(move |_| {
                on_select(image.path.clone());
                if let Some(dialog) = weak_dialog.upgrade() {
                    dialog.close();
                }
            });
            flow.insert(&button, -1);
        }
        flow.invalidate_filter();
        status.set_text(&if missing == 0 {
            format!("{total} imágenes · Selecciona una miniatura para usarla con este fondo.")
        } else { format!("{total} imágenes · {missing} miniaturas no disponibles; puedes seleccionar sus archivos igualmente.") });
    });
    dialog.present();
    Ok(())
}
