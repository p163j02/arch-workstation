use crate::apps::{self, InstalledApp};
use crate::model::{Appearance, BorderAppearance, Document, border_style, validate_color};
use crate::storage::Paths;
use adw::prelude::*;
use gtk::{gdk, gio, glib};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

const ZONES: [&str; 3] = ["modules-left", "modules-center", "modules-right"];
const ZONE_NAMES: [&str; 3] = ["Izquierda", "Centro", "Derecha"];

struct State {
    paths: Paths,
    baseline: Document,
    draft: Document,
    source: String,
}

struct Ui {
    window: adw::ApplicationWindow,
    toast: adw::ToastOverlay,
    state: RefCell<State>,
    title: adw::WindowTitle,
    apply: gtk::Button,
    status: gtk::Label,
    preview: [gtk::Box; 3],
    preview_caption: gtk::Label,
    preview_stack: gtk::Stack,
    preview_picture: gtk::Picture,
    preview_scroll: gtk::ScrolledWindow,
    preview_zoom: gtk::ToggleButton,
    preview_battery: gtk::Box,
    preview_scenario: [gtk::ToggleButton; 4],
    preview_frame: Cell<(i32, i32)>,
    renderer: crate::preview::Renderer,
    populating: Cell<bool>,
    zone_lists: [gtk::ListBox; 3],
    hidden_expander: gtk::Expander,
    hidden_list: gtk::ListBox,
    hidden_scroll: gtk::ScrolledWindow,
    hidden_empty: gtk::Label,
    hidden_count: Cell<usize>,
    preset_list: gtk::ListBox,
    backup_list: gtk::ListBox,
    app_list: gtk::ListBox,
    app_search: gtk::SearchEntry,
    installed: Vec<InstalledApp>,
    appearance_fields: [gtk::Entry; 4],
    module_colors: gtk::Switch,
    appearance_numbers: [gtk::SpinButton; 3],
    border_color: gtk::Entry,
    border_width: gtk::SpinButton,
    border_neon: gtk::Switch,
    border_glow: gtk::SpinButton,
    bar_position: gtk::DropDown,
    bar_numbers: [gtk::SpinButton; 6],
    bar_outputs: gtk::Entry,
    preview_css: gtk::CssProvider,
}

pub fn build(application: &adw::Application, paths: Paths) {
    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("Waybar Editor")
        .default_width(1180)
        .default_height(800)
        .build();
    let content = build_page(application, &window, paths);
    window.set_content(Some(&content));
    window.present();
}

pub(crate) fn build_page(
    application: &adw::Application,
    window: &adw::ApplicationWindow,
    paths: Paths,
) -> gtk::Widget {
    let window = window.clone();
    let document = match paths.load() {
        Ok(document) => document,
        Err(error) => {
            let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
            layout.append(&adw::HeaderBar::new());
            let page = adw::StatusPage::builder()
                .icon_name("dialog-warning-symbolic")
                .title("No se pudo abrir tu configuración")
                .description(format!(
                    "{error:#}\n\nArchivo: {}\nEl editor no modificó ningún archivo.",
                    paths.config.display()
                ))
                .vexpand(true)
                .build();
            layout.append(&page);
            return layout.upcast();
        }
    };

    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_str!("ui.css"));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().expect("A desktop display is required"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let preview_css = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().unwrap(),
        &preview_css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    if application.application_id().as_deref() == Some("io.local.PcSettings") {
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
    }
    let title = adw::WindowTitle::new("Waybar Editor", "Configuración actual");
    header.set_title_widget(Some(&title));
    let reload = icon_button("view-refresh-symbolic", "Recargar archivos del disco");
    header.pack_start(&reload);
    let save = gtk::Button::with_label("Guardar preset");
    let apply = gtk::Button::with_label("Aplicar");
    apply.add_css_class("suggested-action");
    apply.set_tooltip_text(Some("Guardar el borrador y recargar Waybar"));
    header.pack_end(&apply);
    header.pack_end(&save);
    layout.append(&header);

    let preview_area = gtk::Box::new(gtk::Orientation::Vertical, 8);
    preview_area.add_css_class("preview-area");
    let strip = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    strip.set_homogeneous(false);
    strip.add_css_class("bar-preview");
    let preview: [gtk::Box; 3] = std::array::from_fn(|_| {
        let zone = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        zone.set_hexpand(true);
        zone.set_halign(gtk::Align::Center);
        zone.add_css_class("preview-zone");
        strip.append(&zone);
        zone
    });
    let preview_stack = gtk::Stack::new();
    preview_stack.set_hexpand(true);
    preview_stack.add_named(&strip, Some("schematic"));
    let preview_picture = gtk::Picture::new();
    preview_picture.set_can_shrink(true);
    preview_picture.set_content_fit(gtk::ContentFit::Contain);
    preview_picture.set_size_request(-1, 48);
    preview_picture.set_hexpand(true);
    preview_picture.update_property(&[gtk::accessible::Property::Label(
        "Vista de Waybar con datos de ejemplo",
    )]);
    preview_picture.set_tooltip_text(Some("Mismo CSS y motor GTK3 que Waybar. Los valores dinámicos y el tray son ejemplos; no se ejecutan scripts ni acciones."));
    let preview_scroll = gtk::ScrolledWindow::builder()
        .child(&preview_picture)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(48)
        .max_content_height(108)
        .propagate_natural_height(true)
        .build();
    preview_stack.add_named(&preview_scroll, Some("gtk3"));
    preview_area.append(&preview_stack);
    let caption = gtk::Label::new(Some("Preparando vista GTK3 · sin scripts"));
    caption.add_css_class("caption");
    caption.add_css_class("dim-label");
    caption.set_ellipsize(gtk::pango::EllipsizeMode::End);
    caption.set_hexpand(true);
    caption.set_halign(gtk::Align::Start);
    let preview_controls = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    preview_controls.append(&caption);
    let preview_battery = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    preview_battery.append(&gtk::Label::new(Some("Batería:")));
    let scenarios = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    scenarios.add_css_class("linked");
    scenarios.set_tooltip_text(Some(
        "Solo cambia los datos de ejemplo de la vista. No cambia la batería ni sus umbrales.",
    ));
    let preview_scenario: [gtk::ToggleButton; 4] = std::array::from_fn(|i| {
        let button = gtk::ToggleButton::with_label(["75 %", "25 %", "10 %", "Cargando"][i]);
        button.update_property(&[gtk::accessible::Property::Label(
            [
                "Batería de ejemplo: 75 %",
                "Batería de ejemplo: 25 %",
                "Batería de ejemplo: 10 %",
                "Batería de ejemplo: cargando",
            ][i],
        )]);
        scenarios.append(&button);
        button
    });
    for button in &preview_scenario[1..] {
        button.set_group(Some(&preview_scenario[0]));
    }
    preview_scenario[0].set_active(true);
    preview_battery.append(&scenarios);
    preview_controls.append(&preview_battery);
    let preview_zoom = gtk::ToggleButton::with_label("Tamaño real");
    preview_zoom.set_sensitive(false);
    preview_zoom.set_tooltip_text(Some("Activado muestra la barra sin reducirla: desplaza la vista para recorrerla. Desactivado ajusta la imagen al ancho disponible."));
    preview_controls.append(&preview_zoom);
    preview_area.append(&preview_controls);
    layout.append(&preview_area);
    layout.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.set_vexpand(true);
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(140);
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 8);
    sidebar.add_css_class("navigation-sidebar");
    sidebar.set_size_request(184, -1);
    let nav = gtk::ListBox::new();
    nav.add_css_class("navigation-list");
    nav.set_selection_mode(gtk::SelectionMode::Single);
    let navigation = [
        ("bar", "Barra", "view-list-symbolic"),
        ("apps", "Aplicaciones", "view-app-grid-symbolic"),
        ("appearance", "Apariencia", "applications-graphics-symbolic"),
        ("presets", "Presets", "document-save-symbolic"),
        ("backups", "Respaldos", "document-revert-symbolic"),
    ];
    for (_, label, icon) in navigation {
        let row = gtk::ListBoxRow::new();
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.append(&gtk::Image::from_icon_name(icon));
        line.append(&gtk::Label::new(Some(label)));
        row.set_child(Some(&line));
        nav.append(&row);
    }
    sidebar.append(&nav);
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    sidebar.append(&spacer);
    let native = gtk::Label::new(Some("Local · Rust + GTK"));
    native.add_css_class("caption");
    native.add_css_class("dim-label");
    sidebar.append(&native);
    content.append(&sidebar);
    content.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    content.append(&stack);
    layout.append(&content);

    let (bar_page, bar_body) = page(
        "Tu barra, a tu manera",
        "Arrastra módulos entre zonas o usa las flechas. Ocultarlos conserva sus opciones.",
    );
    let hidden_expander = gtk::Expander::new(Some("Módulos ocultos (0)"));
    let hidden_body = gtk::Box::new(gtk::Orientation::Vertical, 10);
    hidden_body.set_margin_top(12);
    let hidden_note = gtk::Label::new(Some(
        "No están en ninguna zona ni grupo visible. Conservan sus opciones. Elige una zona y pulsa Restaurar; solo cambiará el borrador.",
    ));
    hidden_note.set_wrap(true);
    hidden_note.set_xalign(0.0);
    hidden_note.add_css_class("dim-label");
    hidden_body.append(&hidden_note);
    let hidden_list = gtk::ListBox::new();
    hidden_list.set_selection_mode(gtk::SelectionMode::None);
    hidden_list.add_css_class("boxed-list");
    hidden_list.update_property(&[gtk::accessible::Property::Label("Módulos ocultos")]);
    let hidden_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .max_content_height(240)
        .propagate_natural_height(true)
        .child(&hidden_list)
        .build();
    hidden_list
        .bind_property("visible", &hidden_scroll, "visible")
        .sync_create()
        .build();
    hidden_body.append(&hidden_scroll);
    let hidden_empty = gtk::Label::new(Some(
        "No hay módulos ocultos en este borrador. Al ocultar uno, aparecerá aquí para que puedas restaurarlo.",
    ));
    hidden_empty.set_wrap(true);
    hidden_empty.set_xalign(0.0);
    hidden_body.append(&hidden_empty);
    hidden_expander.set_child(Some(&hidden_body));
    bar_body.append(&hidden_expander);
    let zone_grid = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    zone_grid.set_homogeneous(true);
    let zone_lists: [gtk::ListBox; 3] = std::array::from_fn(|index| {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let heading = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let label = gtk::Label::new(Some(ZONE_NAMES[index]));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.add_css_class("heading");
        heading.append(&label);
        column.append(&heading);
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("boxed-list");
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .min_content_height(260)
            .max_content_height(350)
            .propagate_natural_height(true)
            .child(&list)
            .build();
        column.append(&scroll);
        zone_grid.append(&column);
        list
    });
    bar_body.append(&zone_grid);
    let add_module = gtk::Button::with_label("Añadir módulo");
    add_module.set_halign(gtk::Align::Start);
    bar_body.append(&add_module);

    let bar_group = adw::PreferencesGroup::new();
    bar_group.set_title("Posición y monitores");
    bar_group.set_description(Some(
        "Estos ajustes también se guardan dentro de los presets.",
    ));
    let bar_position = gtk::DropDown::from_strings(&["Arriba", "Abajo", "Izquierda", "Derecha"]);
    add_setting(&bar_group, "Posición", &bar_position);
    let labels = [
        "Altura mínima (px)",
        "Separación entre módulos (px)",
        "Margen superior (px)",
        "Margen inferior (px)",
        "Margen izquierdo (px)",
        "Margen derecho (px)",
    ];
    let bar_numbers: [gtk::SpinButton; 6] = std::array::from_fn(|index| {
        let spin = gtk::SpinButton::with_range(0.0, 500.0, 1.0);
        add_setting(&bar_group, labels[index], &spin);
        spin
    });
    let bar_outputs = gtk::Entry::new();
    bar_outputs.set_placeholder_text(Some("HDMI-A-1, eDP-1 · vacío: todos"));
    bar_outputs.set_width_chars(27);
    add_setting(&bar_group, "Monitores", &bar_outputs);
    bar_body.append(&bar_group);
    let stage_bar = gtk::Button::with_label("Guardar ajustes en el borrador");
    stage_bar.set_halign(gtk::Align::Start);
    bar_body.append(&stage_bar);
    stack.add_named(&bar_page, Some("bar"));

    let (apps_page, apps_body) = page(
        "Añade tus aplicaciones",
        "Elige una app instalada, su icono y su zona. Los botones nuevos recordarán el workspace de apertura.",
    );
    let app_search = gtk::SearchEntry::new();
    app_search.set_placeholder_text(Some("Buscar aplicaciones instaladas"));
    apps_body.append(&app_search);
    let manual_app = gtk::Button::with_label("Añadir un comando personalizado");
    manual_app.set_halign(gtk::Align::Start);
    apps_body.append(&manual_app);
    let app_list = gtk::ListBox::new();
    app_list.add_css_class("boxed-list");
    app_list.set_selection_mode(gtk::SelectionMode::None);
    apps_body.append(&app_list);
    stack.add_named(&apps_page, Some("apps"));

    let (appearance_page, appearance_body) = page(
        "Ajusta la apariencia",
        "Los ajustes se añaden como reglas separadas. Tu CSS original permanece intacto; las reglas complejas quedan fuera de este editor visual.",
    );
    let colors = adw::PreferencesGroup::new();
    colors.set_title("Colores");
    colors.set_description(Some("Pulsa la muestra para elegir un color y su transparencia. El código se actualiza y también puedes editarlo: #RRGGBB o rgba(r, g, b, alpha)."));
    let appearance_fields: [gtk::Entry; 4] = std::array::from_fn(|_| gtk::Entry::new());
    for (label, field) in ["Fondo de las zonas", "Texto e iconos", "Workspace activo"]
        .iter()
        .zip(&appearance_fields[..3])
    {
        color_setting(&colors, label, field);
    }
    let module_colors = gtk::Switch::new();
    let module_colors_title = "Conservar colores propios de los módulos";
    module_colors.update_property(&[gtk::accessible::Property::Label(module_colors_title)]);
    let module_colors_row = adw::ActionRow::builder()
        .title(module_colors_title)
        .subtitle("Activado respeta los colores y estados del CSS original, como batería y apagado. Desactivado usa «Texto e iconos» para todos.")
        .build();
    module_colors_row.add_suffix(&module_colors);
    module_colors_row.set_activatable_widget(Some(&module_colors));
    module_colors.set_valign(gtk::Align::Center);
    colors.add(&module_colors_row);
    appearance_body.append(&colors);
    let borders = adw::PreferencesGroup::new();
    borders.set_title("Bordes y neón");
    borders.set_description(Some(
        "Se aplica a las tres zonas. Grosor 0 oculta el borde y el resplandor.",
    ));
    let border_color = gtk::Entry::new();
    color_setting(&borders, "Color del borde", &border_color);
    let border_width = gtk::SpinButton::with_range(0.0, 6.0, 0.5);
    border_width.set_digits(1);
    add_setting(&borders, "Grosor del borde (px)", &border_width);
    let border_neon = gtk::Switch::new();
    add_setting(&borders, "Resplandor neón", &border_neon);
    let border_glow = gtk::SpinButton::with_range(0.0, 100.0, 1.0);
    border_glow.update_property(&[gtk::accessible::Property::Label(
        "Intensidad del resplandor (%)",
    )]);
    let glow_controls = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let glow_slider = gtk::Scale::new(
        gtk::Orientation::Horizontal,
        Some(&border_glow.adjustment()),
    );
    glow_slider.set_draw_value(false);
    glow_slider.set_size_request(120, -1);
    glow_slider.update_property(&[gtk::accessible::Property::Label(
        "Intensidad del resplandor",
    )]);
    glow_controls.append(&glow_slider);
    glow_controls.append(&border_glow);
    add_setting(&borders, "Intensidad (%)", &glow_controls);
    border_neon
        .bind_property("active", &glow_controls, "sensitive")
        .sync_create()
        .build();
    appearance_body.append(&borders);
    let typography = adw::PreferencesGroup::new();
    typography.set_title("Tipografía y espaciado");
    typography.set_description(Some("Los cambios se previsualizan sin modificar Waybar. Conservar colores propios también respeta los avisos de estado y los colores de hover originales."));
    appearance_fields[3].set_width_chars(25);
    add_setting(&typography, "Familia tipográfica", &appearance_fields[3]);
    let appearance_numbers: [gtk::SpinButton; 3] = std::array::from_fn(|index| {
        let spin = gtk::SpinButton::with_range(if index == 0 { 6.0 } else { 0.0 }, 100.0, 1.0);
        add_setting(
            &typography,
            [
                "Tamaño de texto (px)",
                "Radio de las zonas (px)",
                "Padding horizontal (px)",
            ][index],
            &spin,
        );
        spin
    });
    appearance_body.append(&typography);
    let stage_appearance = gtk::Button::with_label("Guardar apariencia en el borrador");
    stage_appearance.set_halign(gtk::Align::Start);
    stage_appearance.add_css_class("suggested-action");
    appearance_body.append(&stage_appearance);
    stack.add_named(&appearance_page, Some("appearance"));

    let (presets_page, presets_body) = page(
        "Una barra para cada momento",
        "Guarda el borrador con un nombre. Cargar un preset no modifica Waybar hasta que pulses Aplicar.",
    );
    let preset_actions = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let preset_save = gtk::Button::with_label("Guardar borrador como preset");
    preset_save.add_css_class("suggested-action");
    let import = gtk::Button::with_label("Importar preset…");
    preset_actions.append(&preset_save);
    preset_actions.append(&import);
    presets_body.append(&preset_actions);
    let preset_list = gtk::ListBox::new();
    preset_list.add_css_class("boxed-list");
    preset_list.set_selection_mode(gtk::SelectionMode::None);
    presets_body.append(&preset_list);
    let dependencies = gtk::Label::new(Some(
        "Los presets guardan JSONC y CSS, no instalan aplicaciones ni copian scripts. Carga solo presets de confianza: sus comandos «exec» se ejecutarán cuando apliques y recargues Waybar. La comprobación de rutas es orientativa.",
    ));
    dependencies.set_wrap(true);
    dependencies.set_xalign(0.0);
    dependencies.add_css_class("dim-label");
    presets_body.append(&dependencies);
    stack.add_named(&presets_page, Some("presets"));

    let (backups_page, backups_body) = page(
        "Cambios que puedes recuperar",
        "Antes de aplicar se guarda una copia exacta de tus archivos. Carga un respaldo, revísalo y pulsa Aplicar para restaurarlo.",
    );
    let backup_list = gtk::ListBox::new();
    backup_list.add_css_class("boxed-list");
    backup_list.set_selection_mode(gtk::SelectionMode::None);
    backups_body.append(&backup_list);
    let path_note = gtk::Label::new(Some(&format!(
        "Configuración: {}\nEstilos: {}\nPresets y respaldos: {}",
        paths.config.display(),
        paths.css.display(),
        paths.data.display()
    )));
    path_note.set_wrap(true);
    path_note.set_selectable(true);
    path_note.set_xalign(0.0);
    path_note.add_css_class("dim-label");
    backups_body.append(&path_note);
    stack.add_named(&backups_page, Some("backups"));

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    footer.add_css_class("status-footer");
    let status = gtk::Label::new(Some("Sin cambios pendientes"));
    status.set_xalign(0.0);
    status.set_hexpand(true);
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    footer.append(&status);
    let hint = gtk::Label::new(Some("Los cambios se aplican solo al confirmar"));
    hint.add_css_class("caption");
    hint.add_css_class("dim-label");
    footer.append(&hint);
    layout.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    layout.append(&footer);
    let toast = adw::ToastOverlay::new();
    toast.set_child(Some(&layout));
    let content = toast.clone().upcast();

    let ui = Rc::new(Ui {
        window,
        toast,
        state: RefCell::new(State {
            paths,
            baseline: document.clone(),
            draft: document,
            source: "Configuración actual".into(),
        }),
        title,
        apply,
        status,
        preview,
        preview_caption: caption,
        preview_stack,
        preview_picture,
        preview_scroll,
        preview_zoom,
        preview_battery,
        preview_scenario,
        preview_frame: Cell::new((0, 0)),
        renderer: crate::preview::Renderer::new(),
        populating: Cell::new(false),
        zone_lists,
        hidden_expander,
        hidden_list,
        hidden_scroll,
        hidden_empty,
        hidden_count: Cell::new(0),
        preset_list,
        backup_list,
        app_list,
        app_search,
        installed: apps::installed_apps(),
        appearance_fields,
        module_colors,
        appearance_numbers,
        border_color,
        border_width,
        border_neon,
        border_glow,
        bar_position,
        bar_numbers,
        bar_outputs,
        preview_css,
    });
    // The window owns the state; use weak captures for its own signal handlers.
    let keep_alive = ui.clone();
    ui.window.connect_destroy(move |_| {
        let _ = &keep_alive;
    });
    let weak = Rc::downgrade(&ui);
    nav.connect_row_selected(move |_, row| {
        if weak.upgrade().is_some()
            && let Some(row) = row
        {
            stack.set_visible_child_name(navigation[row.index() as usize].0);
        }
    });
    nav.select_row(nav.row_at_index(0).as_ref());
    let weak = Rc::downgrade(&ui);
    ui.preview_zoom.connect_toggled(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.update_preview_zoom();
        }
    });
    for button in &ui.preview_scenario {
        let weak = Rc::downgrade(&ui);
        button.connect_toggled(move |button| {
            if button.is_active()
                && let Some(ui) = weak.upgrade()
            {
                ui.refresh_preview();
            }
        });
    }
    let weak = Rc::downgrade(&ui);
    glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
        let Some(ui) = weak.upgrade() else {
            return glib::ControlFlow::Break;
        };
        ui.receive_preview();
        glib::ControlFlow::Continue
    });
    clicked(&ui, &reload, |ui| ui.reload());
    clicked(&ui, &save, |ui| ui.save_preset_dialog(None));
    clicked(&ui, &preset_save, |ui| ui.save_preset_dialog(None));
    clicked(&ui, &ui.apply, |ui| ui.apply_dialog());
    clicked(&ui, &add_module, |ui| ui.add_module_dialog());
    clicked(&ui, &manual_app, |ui| ui.add_app_dialog(None));
    clicked(&ui, &stage_bar, |ui| ui.stage_bar());
    clicked(&ui, &stage_appearance, |ui| ui.stage_appearance());
    for field in ui
        .appearance_fields
        .iter()
        .chain(std::iter::once(&ui.border_color))
    {
        let weak = Rc::downgrade(&ui);
        field.connect_changed(move |_| {
            if let Some(ui) = weak.upgrade()
                && !ui.populating.get()
            {
                ui.refresh_preview();
            }
        });
    }
    for spin in ui
        .appearance_numbers
        .iter()
        .chain([&ui.border_width, &ui.border_glow])
    {
        let weak = Rc::downgrade(&ui);
        spin.connect_value_changed(move |_| {
            if let Some(ui) = weak.upgrade()
                && !ui.populating.get()
            {
                ui.refresh_preview();
            }
        });
    }
    for switch in [&ui.border_neon, &ui.module_colors] {
        let weak = Rc::downgrade(&ui);
        switch.connect_active_notify(move |_| {
            if let Some(ui) = weak.upgrade()
                && !ui.populating.get()
            {
                ui.refresh_preview();
            }
        });
    }
    clicked(&ui, &import, |ui| ui.import_preset());
    let weak = Rc::downgrade(&ui);
    ui.app_search.connect_search_changed(move |_| {
        if let Some(ui) = weak.upgrade() {
            ui.refresh_apps();
        }
    });
    for index in 0..3 {
        let target = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
        let weak = Rc::downgrade(&ui);
        target.connect_drop(move |_, value, _, _| {
            if let (Some(ui), Ok(module)) = (weak.upgrade(), value.get::<String>()) {
                ui.move_module(&module, index, None);
                return true;
            }
            false
        });
        ui.zone_lists[index].add_controller(target);
    }
    let weak = Rc::downgrade(&ui);
    ui.window.connect_close_request(move |_| {
        let Some(ui) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if !ui.dirty() && !ui.appearance_pending() {
            return glib::Propagation::Proceed;
        }
        let window = ui.window.clone();
        ui.confirm(
            "¿Cerrar sin aplicar?",
            "Hay un borrador pendiente. Puedes cancelar y guardarlo como preset antes de salir.",
            "Cerrar sin guardar",
            true,
            move |_| window.destroy(),
        );
        glib::Propagation::Stop
    });
    let apply_action = gio::SimpleAction::new("apply", None);
    let weak = Rc::downgrade(&ui);
    apply_action.connect_activate(move |_, _| {
        if let Some(ui) = weak.upgrade()
            && (ui.dirty() || ui.appearance_pending())
        {
            ui.apply_dialog();
        }
    });
    ui.window.add_action(&apply_action);
    application.set_accels_for_action("win.apply", &["<Control>Return"]);
    let save_action = gio::SimpleAction::new("save-preset", None);
    let weak = Rc::downgrade(&ui);
    save_action.connect_activate(move |_, _| {
        if let Some(ui) = weak.upgrade() {
            ui.save_preset_dialog(None);
        }
    });
    ui.window.add_action(&save_action);
    application.set_accels_for_action("win.save-preset", &["<Control><Shift>s"]);
    ui.refresh();
    ui.refresh_apps();
    content
}

impl Ui {
    fn dirty(&self) -> bool {
        let state = self.state.borrow();
        state.draft != state.baseline
    }

    fn appearance_pending(&self) -> bool {
        self.read_appearance() != self.state.borrow().draft.appearance()
    }

    fn prepare_appearance(self: &Rc<Self>) -> bool {
        if !self.appearance_pending() {
            return true;
        }
        let appearance = self.read_appearance();
        let mut draft = self.state.borrow().draft.clone();
        match draft
            .set_appearance(&appearance)
            .and_then(|_| draft.validate())
        {
            Ok(()) => {
                self.state.borrow_mut().draft = draft;
                self.refresh_preview();
                true
            }
            Err(error) => {
                self.error(error);
                false
            }
        }
    }

    fn notify(&self, text: &str) {
        self.toast.add_toast(adw::Toast::new(text));
    }

    fn error(&self, error: impl std::fmt::Display) {
        let dialog = adw::MessageDialog::new(
            Some(&self.window),
            Some("No se pudo completar el cambio"),
            Some(&error.to_string()),
        );
        dialog.add_response("ok", "Entendido");
        dialog.set_default_response(Some("ok"));
        dialog.present();
    }

    fn confirm(
        self: &Rc<Self>,
        title: &str,
        body: &str,
        accept: &str,
        destructive: bool,
        action: impl Fn(&Rc<Self>) + 'static,
    ) {
        let dialog = adw::MessageDialog::new(Some(&self.window), Some(title), Some(body));
        dialog.add_response("cancel", "Cancelar");
        dialog.add_response("accept", accept);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        if destructive {
            dialog.set_response_appearance("accept", adw::ResponseAppearance::Destructive);
        } else {
            dialog.set_response_appearance("accept", adw::ResponseAppearance::Suggested);
        }
        let weak = Rc::downgrade(self);
        dialog.connect_response(Some("accept"), move |_, _| {
            if let Some(ui) = weak.upgrade() {
                action(&ui);
            }
        });
        dialog.present();
    }

    fn guard_draft(self: &Rc<Self>, action: impl Fn(&Rc<Self>) + 'static) {
        if self.dirty() || self.appearance_pending() {
            self.confirm("¿Reemplazar el borrador?", "Los cambios pendientes se perderán. Cancela si quieres guardarlos primero como preset.", "Reemplazar borrador", true, action);
        } else {
            action(self);
        }
    }

    fn refresh(self: &Rc<Self>) {
        self.refresh_zones();
        self.refresh_presets();
        self.refresh_backups();
        self.populate_settings();
        self.refresh_preview();
        self.update_status();
    }

    fn update_status(&self) {
        let pending = self.appearance_pending();
        let dirty = self.dirty() || pending;
        let state = self.state.borrow();
        self.apply.set_sensitive(dirty);
        self.title.set_subtitle(&format!(
            "{}{}",
            state.source,
            if dirty { " · borrador pendiente" } else { "" }
        ));
        self.status.set_text(if pending {
            "Apariencia por guardar · Waybar todavía no ha cambiado"
        } else if dirty {
            "Borrador pendiente · Waybar todavía no ha cambiado"
        } else {
            "Sin cambios pendientes · configuración sincronizada"
        });
        if dirty {
            self.status.add_css_class("warning");
        } else {
            self.status.remove_css_class("warning");
        }
    }

    fn refresh_preview(&self) {
        let state = self.state.borrow();
        let value = state.draft.value().unwrap_or(json!({}));
        self.preview_battery.set_visible(
            state
                .draft
                .visible_modules()
                .unwrap_or_default()
                .iter()
                .any(|module| module.split('#').next() == Some("battery")),
        );
        let appearance = self.read_appearance();
        let mut module_styles = String::new();
        for (index, zone) in ZONES.iter().enumerate() {
            clear_box(&self.preview[index]);
            let modules = state.draft.modules(zone).unwrap_or_default();
            for (module_index, module) in modules.iter().enumerate().take(14) {
                if module == "hyprland/workspaces" {
                    for number in ["1", "2", "3"] {
                        let label = gtk::Label::new(Some(number));
                        label.add_css_class("preview-token");
                        if number == "2" {
                            label.add_css_class("preview-active");
                        }
                        label.set_tooltip_text(Some("Workspace de ejemplo"));
                        self.preview[index].append(&label);
                    }
                    continue;
                }
                let format = value
                    .get(module)
                    .and_then(|value| value.get("format"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let token = preview_token(module, format);
                let label = gtk::Label::new(Some(&token));
                label.set_tooltip_text(Some(&module_name(module)));
                label.add_css_class("preview-token");
                if appearance.module_colors
                    && let Some(color) = state.draft.module_base_color(module)
                {
                    let class = format!("preview-module-{index}-{module_index}");
                    label.add_css_class(&class);
                    module_styles
                        .push_str(&format!(".preview-token.{class} {{ color: {color}; }}"));
                }
                if preview_uses_icon_font(&token) {
                    label.add_css_class("preview-icon");
                }
                self.preview[index].append(&label);
            }
            if modules.len() > 14 {
                let label = gtk::Label::new(Some(&format!("+{}", modules.len() - 14)));
                label.set_tooltip_text(Some(
                    "Módulos adicionales ocultos solo en esta vista esquemática",
                ));
                label.add_css_class("preview-token");
                self.preview[index].append(&label);
            }
            if modules.is_empty() {
                let label = gtk::Label::new(Some("Zona vacía"));
                label.add_css_class("preview-token");
                label.add_css_class("dim-label");
                self.preview[index].append(&label);
            }
        }
        // Unsaved controls are previewed only; they never change the draft or disk.
        let mut validation = state.draft.clone();
        match validation.set_appearance(&appearance) {
            Ok(()) => {
                let (border, shadow) = border_style(&appearance.border);
                self.preview_css.load_from_data(&format!(
                    ".bar-preview {{ background-color: transparent; }} .preview-zone {{ background-color: {}; color: {}; border-radius: {}px; border: {}; box-shadow: {}; }} .preview-token {{ color: {}; }} .preview-active {{ background-color: {}; }} {}",
                    appearance.background, appearance.foreground, appearance.radius.min(28.0), border, shadow, appearance.foreground, appearance.accent, module_styles
                ));
                // Opening an unchanged document previews its exact CSS, not regenerated rules.
                let document = if appearance == state.draft.appearance() {
                    &state.draft
                } else {
                    &validation
                };
                let display = gtk::prelude::WidgetExt::display(&self.window);
                let monitor = self
                    .window
                    .surface()
                    .and_then(|surface| display.monitor_at_surface(&surface))
                    .or_else(|| {
                        display
                            .monitors()
                            .item(0)
                            .and_then(|m| m.downcast::<gdk::Monitor>().ok())
                    });
                let (width, height) = monitor
                    .map(|m| (m.geometry().width(), m.geometry().height()))
                    .unwrap_or((1920, 1080));
                let directory = state
                    .paths
                    .css
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."));
                let scenario = ["normal", "warning", "critical", "charging"][self
                    .preview_scenario
                    .iter()
                    .position(|button| button.is_active())
                    .unwrap_or(0)];
                self.renderer
                    .request(document, directory, width, height, scenario);
                self.preview_caption
                    .set_text("Actualizando vista GTK3 · datos de ejemplo · sin scripts");
            }
            Err(_) => {
                self.renderer.invalidate();
                self.preview_caption.set_text(
                    "Color o ajuste inválido · se mantiene la última vista previa válida",
                );
            }
        }
        drop(state);
        self.update_status();
    }

    fn receive_preview(&self) {
        let Some(reply) = self.renderer.poll() else {
            return;
        };
        match reply.result {
            Ok(frame) => {
                let bytes = glib::Bytes::from_owned(frame.png);
                match gdk::Texture::from_bytes(&bytes) {
                    Ok(texture) => {
                        self.preview_frame.set((frame.width, frame.height));
                        self.preview_zoom.set_sensitive(true);
                        self.preview_picture.set_paintable(Some(&texture));
                        self.preview_stack.set_visible_child_name("gtk3");
                        self.preview_caption.set_tooltip_text(None);
                        self.update_preview_zoom();
                    }
                    Err(error) => self.preview_error(&error.to_string()),
                }
            }
            Err(error) => self.preview_error(&error),
        }
    }

    fn preview_error(&self, error: &str) {
        self.preview_frame.set((0, 0));
        self.preview_picture.set_paintable(None::<&gdk::Texture>);
        self.preview_picture.set_can_shrink(true);
        self.preview_picture.set_size_request(-1, 48);
        self.preview_scroll
            .set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
        self.preview_zoom.set_sensitive(false);
        self.preview_stack.set_visible_child_name("schematic");
        self.preview_caption
            .set_text("Vista esquemática · GTK3 no pudo dibujar el CSS");
        self.preview_caption.set_tooltip_text(Some(error));
    }

    fn update_preview_zoom(&self) {
        let previous_caption = self.preview_caption.text();
        let (width, height) = self.preview_frame.get();
        if width == 0 {
            return;
        }
        if self.preview_zoom.is_active() {
            self.preview_picture.set_can_shrink(false);
            self.preview_picture.set_size_request(width, height);
            self.preview_scroll
                .set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
            self.preview_caption
                .set_text("Vista GTK3 · tamaño real · datos de ejemplo");
        } else {
            self.preview_picture.set_can_shrink(true);
            self.preview_picture.set_size_request(-1, 48);
            self.preview_scroll
                .set_policy(gtk::PolicyType::Never, gtk::PolicyType::Never);
            self.preview_caption
                .set_text("Vista GTK3 · ajustada al ancho · datos de ejemplo");
        }
        if previous_caption.starts_with("Color o ajuste inválido") {
            self.preview_caption.set_text(&previous_caption);
        }
    }

    fn refresh_zones(self: &Rc<Self>) {
        let document = self.state.borrow().draft.clone();
        for (zone, zone_key) in ZONES.iter().enumerate() {
            clear_list(&self.zone_lists[zone]);
            let modules = document.modules(zone_key).unwrap_or_default();
            if modules.is_empty() {
                let label = gtk::Label::new(Some("Suelta un módulo aquí"));
                label.add_css_class("empty-zone");
                self.zone_lists[zone].append(&label);
            }
            for (index, module) in modules.iter().enumerate() {
                let row = gtk::ListBoxRow::new();
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 2);
                line.add_css_class("module-row");
                let grip = gtk::Image::from_icon_name("view-more-symbolic");
                grip.add_css_class("dim-label");
                line.append(&grip);
                let label = gtk::Label::new(Some(&module_name(module)));
                label.set_xalign(0.0);
                label.set_hexpand(true);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_tooltip_text(Some(module));
                line.append(&label);
                let up = icon_button("go-up-symbolic", "Subir módulo");
                up.set_sensitive(index > 0);
                let down = icon_button("go-down-symbolic", "Bajar módulo");
                down.set_sensitive(index + 1 < modules.len());
                let menu_button = gtk::MenuButton::builder()
                    .icon_name("view-more-symbolic")
                    .tooltip_text("Opciones del módulo")
                    .build();
                menu_button.add_css_class("flat");
                let popover = gtk::Popover::new();
                let actions = gtk::Box::new(gtk::Orientation::Vertical, 4);
                let edit = gtk::Button::with_label("Editar opciones…");
                actions.append(&edit);
                let module_edit = module.clone();
                clicked(self, &edit, move |ui| ui.edit_module_dialog(&module_edit));
                for (destination, zone_name) in ZONE_NAMES.iter().enumerate() {
                    if destination != zone {
                        let button = gtk::Button::with_label(&format!(
                            "Mover a {}",
                            zone_name.to_lowercase()
                        ));
                        let module_move = module.clone();
                        clicked(self, &button, move |ui| {
                            ui.move_module(&module_move, destination, None)
                        });
                        actions.append(&button);
                    }
                }
                let hide = gtk::Button::with_label("Ocultar módulo");
                hide.add_css_class("destructive-action");
                let module_hide = module.clone();
                clicked(self, &hide, move |ui| ui.hide_module(&module_hide));
                actions.append(&hide);
                popover.set_child(Some(&actions));
                menu_button.set_popover(Some(&popover));
                let m = module.clone();
                clicked(self, &up, move |ui| ui.reorder(&m, zone, -1));
                let m = module.clone();
                clicked(self, &down, move |ui| ui.reorder(&m, zone, 1));
                line.append(&up);
                line.append(&down);
                line.append(&menu_button);
                row.set_child(Some(&line));
                let drag = gtk::DragSource::new();
                drag.set_actions(gdk::DragAction::MOVE);
                let text = module.clone();
                drag.connect_prepare(move |_, _, _| {
                    Some(gdk::ContentProvider::for_value(&text.to_value()))
                });
                row.add_controller(drag);
                let target = gtk::DropTarget::new(String::static_type(), gdk::DragAction::MOVE);
                let weak = Rc::downgrade(self);
                let before = module.clone();
                target.connect_drop(move |_, value, _, _| {
                    if let (Some(ui), Ok(module)) = (weak.upgrade(), value.get::<String>()) {
                        ui.move_module(&module, zone, Some(&before));
                        return true;
                    }
                    false
                });
                row.add_controller(target);
                self.zone_lists[zone].append(&row);
            }
        }
        self.refresh_hidden_modules(&document);
    }

    fn set_hidden_title(&self, title: &str) {
        self.hidden_expander.set_label(Some(title));
        if let Some(label) = self.hidden_expander.label_widget() {
            label.add_css_class("heading");
            self.hidden_expander
                .update_relation(&[gtk::accessible::Relation::LabelledBy(&[label.upcast_ref()])]);
        }
    }

    fn refresh_hidden_modules(self: &Rc<Self>, document: &Document) {
        clear_list(&self.hidden_list);
        let modules = match document.hidden_modules() {
            Ok(modules) => modules,
            Err(error) => {
                self.set_hidden_title("Módulos ocultos · error");
                self.hidden_list.set_visible(false);
                self.hidden_empty
                    .set_text(&format!("No se pudo leer la lista: {error}"));
                self.hidden_empty.set_visible(true);
                return;
            }
        };
        let previous_count = self.hidden_count.replace(modules.len());
        self.set_hidden_title(&format!("Módulos ocultos ({})", modules.len()));
        self.hidden_list.set_visible(!modules.is_empty());
        self.hidden_empty.set_text("No hay módulos ocultos en este borrador. Al ocultar uno, aparecerá aquí para que puedas restaurarlo.");
        self.hidden_empty.set_visible(modules.is_empty());
        if previous_count == 0 && !modules.is_empty() {
            self.hidden_expander.set_expanded(true);
        }
        for module in modules {
            let row = adw::ActionRow::builder()
                .title(module_name(&module))
                .subtitle(&module)
                .title_lines(1)
                .subtitle_lines(1)
                .build();
            row.set_use_markup(false);
            row.set_tooltip_text(Some(&module));
            row.set_activatable(false);
            row.set_selectable(false);
            let zone = gtk::DropDown::from_strings(&ZONE_NAMES);
            zone.set_selected(1);
            zone.set_valign(gtk::Align::Center);
            zone.update_property(&[gtk::accessible::Property::Label(&format!(
                "Zona para restaurar {module}"
            ))]);
            zone.set_tooltip_text(Some("Zona donde volverá a aparecer el módulo"));
            row.add_suffix(&zone);
            let restore = gtk::Button::with_label("Restaurar");
            restore.set_valign(gtk::Align::Center);
            restore.update_property(&[gtk::accessible::Property::Label(&format!(
                "Restaurar {module}"
            ))]);
            restore.set_tooltip_text(Some(&format!("Restaurar {module} en la zona elegida")));
            clicked(self, &restore, move |ui| {
                let destination = zone.selected() as usize;
                if let Some(zone_key) = ZONES.get(destination) {
                    ui.mutate(|draft| draft.restore_module(&module, zone_key));
                }
            });
            row.add_suffix(&restore);
            self.hidden_list.append(&row);
        }
        if self.hidden_count.get() > 0 {
            // Nested scroll views otherwise shrink to a single clipped row.
            // Show a short list completely, capped for larger collections.
            let minimum = self.hidden_list.measure(gtk::Orientation::Vertical, -1).0;
            self.hidden_scroll
                .set_min_content_height(minimum.clamp(64, 240));
        }
    }

    fn mutate(self: &Rc<Self>, change: impl FnOnce(&mut Document) -> anyhow::Result<()>) {
        let mut draft = self.state.borrow().draft.clone();
        match change(&mut draft).and_then(|_| draft.validate()) {
            Ok(()) => {
                self.state.borrow_mut().draft = draft;
                self.refresh_zones();
                self.refresh_preview();
                self.update_status();
            }
            Err(error) => self.error(format!("{error:#}")),
        }
    }

    fn reorder(self: &Rc<Self>, module: &str, zone: usize, delta: isize) {
        self.mutate(|draft| {
            let mut modules = draft.modules(ZONES[zone])?;
            if let Some(index) = modules.iter().position(|item| item == module) {
                let destination = index as isize + delta;
                if destination >= 0 && destination < modules.len() as isize {
                    modules.swap(index, destination as usize);
                }
            }
            draft.set_modules(ZONES[zone], &modules)
        });
    }

    fn move_module(self: &Rc<Self>, module: &str, destination: usize, before: Option<&str>) {
        if before == Some(module) {
            return;
        }
        self.mutate(|draft| {
            if !draft.all_modules()?.iter().any(|name| name == module) {
                anyhow::bail!("El módulo arrastrado no pertenece a esta configuración.");
            }
            for zone in ZONES {
                let modules: Vec<String> = draft
                    .modules(zone)?
                    .into_iter()
                    .filter(|name| name != module)
                    .collect();
                draft.set_modules(zone, &modules)?;
            }
            let mut modules = draft.modules(ZONES[destination])?;
            let position = before
                .and_then(|name| modules.iter().position(|item| item == name))
                .unwrap_or(modules.len());
            modules.insert(position, module.to_string());
            draft.set_modules(ZONES[destination], &modules)
        });
    }

    fn hide_module(self: &Rc<Self>, module: &str) {
        self.mutate(|draft| draft.hide_module(module));
        self.hidden_expander.set_expanded(true);
    }

    fn populate_settings(&self) {
        self.populating.set(true);
        let state = self.state.borrow();
        let appearance = state.draft.appearance();
        for (entry, value) in self.appearance_fields.iter().zip([
            appearance.background,
            appearance.foreground,
            appearance.accent,
            appearance.font,
        ]) {
            entry.set_text(&value);
        }
        for (spin, value) in self.appearance_numbers.iter().zip([
            appearance.font_size,
            appearance.radius,
            appearance.padding,
        ]) {
            spin.set_value(value);
        }
        self.border_color.set_text(&appearance.border.color);
        self.border_width.set_value(appearance.border.width);
        self.module_colors.set_active(appearance.module_colors);
        self.border_neon.set_active(appearance.border.neon);
        self.border_glow.set_value(appearance.border.glow);
        let root = state.draft.value().unwrap_or(json!({}));
        let position = root["position"].as_str().unwrap_or("top");
        self.bar_position.set_selected(
            ["top", "bottom", "left", "right"]
                .iter()
                .position(|value| *value == position)
                .unwrap_or(0) as u32,
        );
        for (spin, key) in self.bar_numbers.iter().zip([
            "height",
            "spacing",
            "margin-top",
            "margin-bottom",
            "margin-left",
            "margin-right",
        ]) {
            spin.set_value(root[key].as_f64().unwrap_or(0.0));
        }
        let outputs = match &root["output"] {
            Value::String(value) => value.clone(),
            Value::Array(values) => values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
            _ => String::new(),
        };
        self.bar_outputs.set_text(&outputs);
        self.populating.set(false);
    }

    fn read_appearance(&self) -> Appearance {
        Appearance {
            background: self.appearance_fields[0].text().to_string(),
            foreground: self.appearance_fields[1].text().to_string(),
            module_colors: self.module_colors.is_active(),
            accent: self.appearance_fields[2].text().to_string(),
            font: self.appearance_fields[3].text().to_string(),
            font_size: self.appearance_numbers[0].value(),
            radius: self.appearance_numbers[1].value(),
            padding: self.appearance_numbers[2].value(),
            border: BorderAppearance {
                color: self.border_color.text().to_string(),
                width: self.border_width.value(),
                neon: self.border_neon.is_active(),
                glow: self.border_glow.value(),
            },
        }
    }

    fn stage_appearance(self: &Rc<Self>) {
        let appearance = self.read_appearance();
        self.mutate(|draft| draft.set_appearance(&appearance));
    }

    fn stage_bar(self: &Rc<Self>) {
        let position = ["top", "bottom", "left", "right"][self.bar_position.selected() as usize];
        let values: Vec<i64> = self
            .bar_numbers
            .iter()
            .map(|spin| spin.value_as_int() as i64)
            .collect();
        let output_text = self.bar_outputs.text().to_string();
        self.mutate(|draft| {
            draft.set_property(None, "position", json!(position))?;
            for (key, value) in [
                "height",
                "spacing",
                "margin-top",
                "margin-bottom",
                "margin-left",
                "margin-right",
            ]
            .iter()
            .zip(values)
            {
                draft.set_property(None, key, json!(value))?;
            }
            let outputs: Vec<&str> = output_text
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect();
            draft.set_property(
                None,
                "output",
                if outputs.is_empty() {
                    json!(["*"])
                } else {
                    json!(outputs)
                },
            )
        });
    }

    fn reload(self: &Rc<Self>) {
        self.guard_draft(|ui| {
            let loaded = ui.state.borrow().paths.load();
            match loaded {
                Ok(document) => {
                    let mut state = ui.state.borrow_mut();
                    state.baseline = document.clone();
                    state.draft = document;
                    state.source = "Configuración actual".into();
                    drop(state);
                    ui.refresh();
                    ui.notify("Archivos recargados; no se modificó Waybar");
                }
                Err(error) => ui.error(format!("{error:#}")),
            }
        });
    }

    fn apply_dialog(self: &Rc<Self>) {
        if !self.prepare_appearance() {
            return;
        }
        let summary = {
            let state = self.state.borrow();
            format!(
                "{}\n\nSe modificarán {} y {}. Antes se guardará un respaldo. Waybar se recargará y los módulos con «exec» podrán ejecutar sus scripts. No se instalarán apps. Aplica solo presets de confianza.",
                change_summary(&state.baseline, &state.draft),
                state.paths.config.display(),
                state.paths.css.display()
            )
        };
        self.confirm("¿Aplicar el borrador a Waybar?", &summary, "Aplicar y recargar", false, |ui| {
            let result = {
                crate::settings::store::desktop_lock().and_then(|_lock| {
                    let state = ui.state.borrow();
                    state.paths.apply(&state.draft, &state.baseline)
                })
            };
            match result {
                Ok(_) => {
                    let mut state = ui.state.borrow_mut();
                    state.baseline = state.draft.clone();
                    drop(state);
                    ui.refresh_backups(); ui.update_status();
                    match std::process::Command::new("pkill").args(["-SIGUSR2", "-x", "waybar"]).status() {
                        Ok(status) if status.success() => ui.notify("Configuración aplicada y respaldo guardado. Waybar recargado."),
                        _ => ui.notify("Archivos guardados y respaldo creado. No se encontró Waybar para recargar."),
                    }
                }
                Err(error) => ui.error(format!("{error:#}\n\nTus cambios siguen en el borrador. Si otro programa editó los archivos, guárdalo como preset antes de recargar.")),
            }
        });
    }

    fn add_module_dialog(self: &Rc<Self>) {
        let mut modules = self.state.borrow().draft.all_modules().unwrap_or_default();
        for module in [
            "clock",
            "cpu",
            "memory",
            "disk",
            "temperature",
            "battery",
            "network",
            "pulseaudio",
            "backlight",
            "tray",
            "bluetooth",
            "hyprland/workspaces",
            "hyprland/window",
            "hyprland/language",
            "wlr/taskbar",
            "idle_inhibitor",
        ] {
            if crate::hardware::module_available(module)
                && !modules.iter().any(|name| name == module)
            {
                modules.push(module.into());
            }
        }
        modules.sort();
        let dialog = form_dialog(
            &self.window,
            "Añadir módulo",
            "El catálogo ofrece batería y brillo de portátil solo si el equipo dispone de ese hardware. Los módulos ya configurados se conservan; los nuevos usan los valores predeterminados de Waybar.",
        );
        let chooser =
            gtk::DropDown::from_strings(&modules.iter().map(String::as_str).collect::<Vec<_>>());
        chooser.set_enable_search(true);
        dialog.body.append(&field_row("Módulo", &chooser));
        let zone = gtk::DropDown::from_strings(&ZONE_NAMES);
        zone.set_selected(1);
        dialog.body.append(&field_row("Zona", &zone));
        let window = dialog.window.clone();
        clicked(self, &dialog.save, move |ui| {
            if let Some(module) = modules.get(chooser.selected() as usize) {
                let module = module.clone();
                let destination = zone.selected() as usize;
                ui.mutate(|draft| {
                    let mut list = draft.modules(ZONES[destination])?;
                    if !list.contains(&module) {
                        list.push(module.clone());
                    }
                    // A module may legitimately appear in multiple zones; do not overwrite its options.
                    draft.set_modules(ZONES[destination], &list)
                });
                window.close();
            }
        });
        dialog.window.present();
    }

    fn edit_module_dialog(self: &Rc<Self>, module: &str) {
        let root = self.state.borrow().draft.value().unwrap_or(json!({}));
        let original = root.get(module).cloned().unwrap_or(json!({}));
        let dialog = form_dialog(
            &self.window,
            &format!("Editar {}", module_name(module)),
            "Se guardan solo las opciones que cambies. «on-click» se ejecuta al pulsar el módulo; las opciones avanzadas «exec» pueden ejecutarse al recargar Waybar.",
        );
        let format = gtk::Entry::new();
        let tooltip = gtk::Entry::new();
        let command = gtk::Entry::new();
        format.set_text(original["format"].as_str().unwrap_or(""));
        tooltip.set_text(original["tooltip-format"].as_str().unwrap_or(""));
        command.set_text(original["on-click"].as_str().unwrap_or(""));
        dialog.body.append(&field_row("Formato / icono", &format));
        dialog.body.append(&field_row("Texto de ayuda", &tooltip));
        dialog
            .body
            .append(&field_row("Comando al hacer clic", &command));
        let expander = gtk::Expander::new(Some("Opciones avanzadas del módulo (JSON)"));
        let advanced = gtk::TextView::new();
        advanced.set_monospace(true);
        advanced.set_wrap_mode(gtk::WrapMode::WordChar);
        let initial_json = serde_json::to_string_pretty(&original).unwrap_or("{}".into());
        advanced.buffer().set_text(&initial_json);
        let advanced_scroll = gtk::ScrolledWindow::builder()
            .min_content_height(220)
            .child(&advanced)
            .build();
        expander.set_child(Some(&advanced_scroll));
        dialog.body.append(&expander);
        let note = gtk::Label::new(Some(
            "Las claves omitidas en el editor avanzado no se eliminan. Para modificar una opción existente, cambia su valor.",
        ));
        note.set_wrap(true);
        note.set_xalign(0.0);
        note.add_css_class("caption");
        note.add_css_class("dim-label");
        dialog.body.append(&note);
        let module = module.to_string();
        let window = dialog.window.clone();
        clicked(self, &dialog.save, move |ui| {
            let buffer = advanced.buffer();
            let text = buffer
                .text(&buffer.start_iter(), &buffer.end_iter(), false)
                .to_string();
            let mut changes = serde_json::Map::new();
            if text != initial_json {
                match serde_json::from_str::<Value>(&text) {
                    Ok(Value::Object(object)) => {
                        for (key, value) in object {
                            if original.get(&key) != Some(&value) {
                                changes.insert(key, value);
                            }
                        }
                    }
                    _ => {
                        ui.error("Las opciones avanzadas deben ser un objeto JSON válido. Corrige el contenido e inténtalo de nuevo.");
                        return;
                    }
                }
            }
            for (key, value) in [
                ("format", format.text()),
                ("tooltip-format", tooltip.text()),
                ("on-click", command.text()),
            ] {
                if value.as_str() != original[key].as_str().unwrap_or("") {
                    changes.insert(key.into(), json!(value.as_str()));
                }
            }
            let mut draft = ui.state.borrow().draft.clone();
            let result = (|| -> anyhow::Result<()> {
                for (key, value) in changes {
                    draft.set_property(Some(&module), &key, value)?;
                }
                draft.validate()
            })();
            match result {
                Ok(()) => {
                    ui.state.borrow_mut().draft = draft;
                    ui.refresh_zones();
                    ui.refresh_preview();
                    ui.update_status();
                    window.close();
                }
                Err(error) => ui.error(format!("{error:#}")),
            }
        });
        dialog.window.present();
    }

    fn refresh_apps(self: &Rc<Self>) {
        clear_list(&self.app_list);
        let query = self.app_search.text().to_lowercase();
        let filtered: Vec<_> = self
            .installed
            .iter()
            .filter(|app| {
                app.name.to_lowercase().contains(&query) || app.id.to_lowercase().contains(&query)
            })
            .collect();
        if filtered.is_empty() {
            self.app_list.append(&empty_state(
                "No hay aplicaciones coincidentes",
                "Prueba otro nombre o añade un comando personalizado.",
                "system-search-symbolic",
            ));
        }
        for app in filtered {
            let row = adw::ActionRow::new();
            row.set_use_markup(false);
            row.set_title(&app.name);
            row.set_subtitle(&app.id);
            let icon = if app.icon.starts_with('/') {
                gtk::Image::from_file(&app.icon)
            } else {
                gtk::Image::from_icon_name(if app.icon.is_empty() {
                    "application-x-executable-symbolic"
                } else {
                    &app.icon
                })
            };
            icon.set_pixel_size(28);
            row.add_prefix(&icon);
            let button = gtk::Button::with_label("Añadir…");
            button.set_valign(gtk::Align::Center);
            let app = app.clone();
            clicked(self, &button, move |ui| {
                ui.add_app_dialog(Some(app.clone()))
            });
            row.add_suffix(&button);
            self.app_list.append(&row);
        }
    }

    fn add_app_dialog(self: &Rc<Self>, app: Option<InstalledApp>) {
        let dialog = form_dialog(
            &self.window,
            "Añadir aplicación a Waybar",
            "Elige un glifo o texto para el botón. El comando se envolverá con el lanzador que conserva el workspace; no se ejecutará ahora.",
        );
        let name = gtk::Entry::new();
        let icon = gtk::Entry::new();
        let command = gtk::Entry::new();
        name.set_text(app.as_ref().map(|app| app.name.as_str()).unwrap_or(""));
        icon.set_text("󰣆");
        command.set_text(app.as_ref().map(|app| app.command.as_str()).unwrap_or(""));
        command.set_placeholder_text(Some("programa --argumentos · sin tuberías de shell"));
        dialog.body.append(&field_row("Nombre", &name));
        icon.set_hexpand(true);
        icon.set_placeholder_text(Some("Busca un icono o escribe texto"));
        let icon_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        icon_row.append(&icon);
        let choose_glyph = gtk::Button::with_label("Explorar iconos…");
        let picker_parent = dialog.window.clone();
        let icon_entry = icon.clone();
        choose_glyph.connect_clicked(move |_| glyph_picker(&picker_parent, &icon_entry));
        icon_row.append(&choose_glyph);
        dialog.body.append(&field_row("Icono o texto", &icon_row));
        dialog
            .body
            .append(&field_row("Comando y argumentos", &command));
        let zone = gtk::DropDown::from_strings(&ZONE_NAMES);
        zone.set_selected(1);
        dialog.body.append(&field_row("Zona", &zone));
        let module = gtk::Entry::new();
        let suggested = app
            .as_ref()
            .map(|app| app.id.trim_end_matches(".desktop"))
            .unwrap_or("mi-app");
        module.set_text(&format!("custom/{}", slug(suggested)));
        dialog
            .body
            .append(&field_row("Identificador del módulo", &module));
        if !crate::model::launcher_path().is_ok_and(|path| path.is_file()) {
            dialog.body.append(&gtk::Label::new(Some(
                "Falta launch-in-workspace.py: instala el lanzador antes de añadir apps.",
            )));
            dialog.save.set_sensitive(false);
        }
        let window = dialog.window.clone();
        clicked(self, &dialog.save, move |ui| {
            let name = name.text().trim().to_string();
            let module = module.text().trim().to_string();
            let command = command.text().trim().to_string();
            if name.is_empty() || command.is_empty() {
                ui.error("Escribe un nombre y un comando para la aplicación.");
                return;
            }
            if !module.starts_with("custom/")
                || module.len() <= 7
                || !module[7..]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
            {
                ui.error("Usa un identificador como custom/mi-app: letras sin acentos, números, guiones o guiones bajos.");
                return;
            }
            if ui
                .state
                .borrow()
                .draft
                .value()
                .ok()
                .and_then(|value| value.get(&module).cloned())
                .is_some()
            {
                ui.error("Ya existe ese identificador. Cambia el nombre del módulo o edita el existente desde Barra.");
                return;
            }
            let mut draft = ui.state.borrow().draft.clone();
            let result = (|| -> anyhow::Result<()> {
                draft.add_app(&module, &name, icon.text().as_str(), &command)?;
                let destination = zone.selected() as usize;
                if destination != 1 {
                    let center: Vec<_> = draft
                        .modules(ZONES[1])?
                        .into_iter()
                        .filter(|name| name != &module)
                        .collect();
                    draft.set_modules(ZONES[1], &center)?;
                    let mut list = draft.modules(ZONES[destination])?;
                    list.push(module.clone());
                    draft.set_modules(ZONES[destination], &list)?;
                }
                draft.validate()
            })();
            match result {
                Ok(()) => {
                    ui.state.borrow_mut().draft = draft;
                    ui.refresh_zones();
                    ui.refresh_preview();
                    ui.update_status();
                    ui.notify("Aplicación añadida al borrador");
                    window.close();
                }
                Err(error) => ui.error(format!("{error:#}")),
            }
        });
        dialog.window.present();
    }

    fn save_preset_dialog(self: &Rc<Self>, duplicate: Option<String>) {
        if duplicate.is_none() && !self.prepare_appearance() {
            return;
        }
        let document = match duplicate.as_deref() {
            Some(name) => match self.state.borrow().paths.load_preset(name) {
                Ok(document) => document,
                Err(error) => {
                    self.error(error);
                    return;
                }
            },
            None => self.state.borrow().draft.clone(),
        };
        self.name_dialog(
            if duplicate.is_some() {
                "Duplicar preset"
            } else {
                "Guardar borrador como preset"
            },
            duplicate
                .as_ref()
                .map(|name| format!("{name} copia"))
                .as_deref()
                .unwrap_or(""),
            move |ui, name| {
                let result = ui.state.borrow().paths.save_preset(name, &document);
                match result {
                    Ok(()) => {
                        ui.refresh_presets();
                        ui.notify("Preset guardado. La barra activa no cambió.");
                        true
                    }
                    Err(error) => {
                        ui.error(format!("{error:#}"));
                        false
                    }
                }
            },
        );
    }

    fn name_dialog(
        self: &Rc<Self>,
        title: &str,
        initial: &str,
        action: impl Fn(&Rc<Self>, &str) -> bool + 'static,
    ) {
        let dialog = form_dialog(
            &self.window,
            title,
            "Usa un nombre descriptivo de hasta 64 caracteres. Se permiten espacios y acentos; no se sobrescriben presets existentes.",
        );
        let entry = gtk::Entry::new();
        entry.set_text(initial);
        entry.set_placeholder_text(Some("Por ejemplo: Trabajo o Juegos"));
        dialog.body.append(&field_row("Nombre", &entry));
        let focus_entry = entry.clone();
        let window = dialog.window.clone();
        clicked(self, &dialog.save, move |ui| {
            if action(ui, entry.text().trim()) {
                window.close();
            }
        });
        dialog.window.present();
        focus_entry.grab_focus();
    }

    fn refresh_presets(self: &Rc<Self>) {
        clear_list(&self.preset_list);
        let names = match self.state.borrow().paths.list_presets() {
            Ok(names) => names,
            Err(error) => {
                self.error(error);
                return;
            }
        };
        if names.is_empty() {
            self.preset_list.append(&empty_state(
                "Tu primera configuración guardada",
                "Guarda tu barra actual como preset para volver a ella cuando quieras.",
                "document-save-symbolic",
            ));
        }
        for name in names {
            let row = adw::ActionRow::new();
            row.set_use_markup(false);
            row.set_title(&name);
            row.set_subtitle("Configuración + estilos · scripts externos no incluidos");
            row.add_prefix(&gtk::Image::from_icon_name("document-properties-symbolic"));
            let load = gtk::Button::with_label("Cargar");
            load.set_valign(gtk::Align::Center);
            let preset = name.clone();
            clicked(self, &load, move |ui| ui.load_preset(&preset));
            row.add_suffix(&load);
            let menu = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .valign(gtk::Align::Center)
                .build();
            menu.add_css_class("flat");
            let popover = gtk::Popover::new();
            let actions = gtk::Box::new(gtk::Orientation::Vertical, 4);
            let duplicate = gtk::Button::with_label("Duplicar…");
            let preset = name.clone();
            clicked(self, &duplicate, move |ui| {
                ui.save_preset_dialog(Some(preset.clone()))
            });
            actions.append(&duplicate);
            let rename = gtk::Button::with_label("Renombrar…");
            let preset = name.clone();
            clicked(self, &rename, move |ui| {
                let old = preset.clone();
                ui.name_dialog("Renombrar preset", &preset, move |ui, new| {
                    let result = ui.state.borrow().paths.rename_preset(&old, new);
                    match result {
                        Ok(()) => {
                            ui.refresh_presets();
                            true
                        }
                        Err(error) => {
                            ui.error(error);
                            false
                        }
                    }
                });
            });
            actions.append(&rename);
            let export = gtk::Button::with_label("Exportar…");
            let preset = name.clone();
            clicked(self, &export, move |ui| ui.export_preset(&preset));
            actions.append(&export);
            let delete = gtk::Button::with_label("Eliminar…");
            delete.add_css_class("destructive-action");
            let preset = name.clone();
            clicked(self, &delete, move |ui| {
                let name = preset.clone();
                ui.confirm(
                    "¿Eliminar el preset?",
                    &format!(
                        "Se eliminará «{preset}». No se cambiará tu borrador ni la barra activa."
                    ),
                    "Eliminar preset",
                    true,
                    move |ui| {
                        let result = ui.state.borrow().paths.delete_preset(&name);
                        match result {
                            Ok(()) => ui.refresh_presets(),
                            Err(error) => ui.error(error),
                        }
                    },
                );
            });
            actions.append(&delete);
            popover.set_child(Some(&actions));
            menu.set_popover(Some(&popover));
            row.add_suffix(&menu);
            self.preset_list.append(&row);
        }
    }

    fn load_preset(self: &Rc<Self>, name: &str) {
        let name = name.to_string();
        self.guard_draft(move |ui| {
            let document = ui.state.borrow().paths.load_preset(&name);
            match document {
                Ok(document) => {
                    let warnings = missing_dependencies(&document);
                    let mut state = ui.state.borrow_mut(); state.draft = document; state.source = format!("Preset: {name}"); drop(state);
                    ui.refresh();
                    if warnings.is_empty() { ui.notify("Preset cargado como borrador. Pulsa Aplicar para usarlo."); }
                    else { ui.error(format!("El preset se cargó como borrador, pero faltan estas rutas:\n{}\n\nRevisa sus comandos antes de aplicar.", warnings.join("\n"))); }
                }
                Err(error) => ui.error(error),
            }
        });
    }

    fn import_preset(self: &Rc<Self>) {
        let chooser = gtk::FileDialog::builder()
            .title("Importar preset de Waybar Editor")
            .build();
        chooser.set_default_filter(Some(&json_filter()));
        let weak = Rc::downgrade(self);
        chooser.open(Some(&self.window), gio::Cancellable::NONE, move |result| {
            if let (Some(ui), Ok(file)) = (weak.upgrade(), result)
                && let Some(path) = file.path()
            {
                let initial = path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Importado")
                    .to_string();
                ui.name_dialog("Nombre del preset importado", &initial, move |ui, name| {
                    let result = ui.state.borrow().paths.import_preset(&path, name);
                    match result {
                        Ok(()) => {
                            ui.refresh_presets();
                            ui.notify("Preset importado. Tu barra no cambió.");
                            true
                        }
                        Err(error) => {
                            ui.error(error);
                            false
                        }
                    }
                });
            }
        });
    }

    fn export_preset(self: &Rc<Self>, name: &str) {
        let chooser = gtk::FileDialog::builder()
            .title("Exportar preset · elige un archivo nuevo")
            .initial_name(format!("{name}.json"))
            .build();
        chooser.set_default_filter(Some(&json_filter()));
        let name = name.to_string();
        let weak = Rc::downgrade(self);
        chooser.save(Some(&self.window), gio::Cancellable::NONE, move |result| {
            if let (Some(ui), Ok(file)) = (weak.upgrade(), result)
                && let Some(path) = file.path()
            {
                let result = ui.state.borrow().paths.export_preset(&name, &path);
                match result {
                    Ok(()) => ui.notify("Preset exportado"),
                    Err(error) => ui.error(error),
                }
            }
        });
    }

    fn refresh_backups(self: &Rc<Self>) {
        clear_list(&self.backup_list);
        let backups = match self.state.borrow().paths.list_backups() {
            Ok(backups) => backups,
            Err(error) => {
                self.error(error);
                return;
            }
        };
        if backups.is_empty() {
            self.backup_list.append(&empty_state(
                "Todavía no hay respaldos",
                "El primer respaldo se crea automáticamente cuando apliques un cambio.",
                "document-revert-symbolic",
            ));
        }
        for path in backups {
            let row = adw::ActionRow::new();
            row.set_title(&backup_name(&path));
            row.set_subtitle("Copia exacta de los archivos anteriores a un cambio");
            row.add_prefix(&gtk::Image::from_icon_name("document-revert-symbolic"));
            let button = gtk::Button::with_label("Recuperar borrador");
            button.set_valign(gtk::Align::Center);
            clicked(self, &button, move |ui| {
                let path = path.clone();
                ui.guard_draft(move |ui| {
                    let loaded = ui.state.borrow().paths.load_backup(&path);
                    match loaded {
                        Ok(document) => {
                            let mut state = ui.state.borrow_mut();
                            state.draft = document;
                            state.source = "Respaldo recuperado".into();
                            drop(state);
                            ui.refresh();
                            ui.notify("Respaldo cargado como borrador. Revísalo y pulsa Aplicar.");
                        }
                        Err(error) => ui.error(error),
                    }
                });
            });
            row.add_suffix(&button);
            self.backup_list.append(&row);
        }
    }
}

fn clicked(ui: &Rc<Ui>, button: &gtk::Button, action: impl Fn(&Rc<Ui>) + 'static) {
    let weak = Rc::downgrade(ui);
    button.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            action(&ui);
        }
    });
}

fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .build();
    button.add_css_class("flat");
    button
}

fn page(title: &str, subtitle: &str) -> (gtk::ScrolledWindow, gtk::Box) {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    body.add_css_class("editor-page");
    let heading = gtk::Label::new(Some(title));
    heading.set_xalign(0.0);
    heading.set_wrap(true);
    heading.add_css_class("title-1");
    let description = gtk::Label::new(Some(subtitle));
    description.set_xalign(0.0);
    description.set_wrap(true);
    description.add_css_class("dim-label");
    body.append(&heading);
    body.append(&description);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&body)
        .build();
    (scroll, body)
}

fn add_setting(group: &adw::PreferencesGroup, title: &str, widget: &impl IsA<gtk::Widget>) {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_activatable(false);
    widget.as_ref().set_valign(gtk::Align::Center);
    widget
        .as_ref()
        .update_property(&[gtk::accessible::Property::Label(title)]);
    row.add_suffix(widget);
    group.add(&row);
}

fn color_setting(group: &adw::PreferencesGroup, title: &str, entry: &gtk::Entry) {
    entry.set_width_chars(24);
    entry.set_max_width_chars(24);
    entry.set_placeholder_text(Some("#65d9ff o rgba(101, 217, 255, 0.6)"));
    entry.update_property(&[gtk::accessible::Property::Label(&format!(
        "Código: {title}"
    ))]);
    let dialog = gtk::ColorDialog::builder()
        .title(format!("Elegir color: {title}"))
        .with_alpha(true)
        .modal(true)
        .build();
    let button = gtk::ColorDialogButton::new(Some(dialog));
    button.set_tooltip_text(Some(&format!("Elegir color: {title}")));
    button.update_property(&[gtk::accessible::Property::Label(&format!(
        "Elegir color: {title}"
    ))]);
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.append(&button);
    controls.append(entry);
    add_setting(group, title, &controls);
    let syncing = Rc::new(Cell::new(false));
    let weak = button.downgrade();
    let flag = syncing.clone();
    entry.connect_changed(move |entry| {
        let text = entry.text();
        if validate_color(&text).is_ok()
            && let Ok(color) = gdk::RGBA::parse(text.trim())
        {
            entry.remove_css_class("error");
            entry.set_tooltip_text(Some(
                "Puedes editar el código; la muestra refleja el mismo color.",
            ));
            if let Some(button) = weak.upgrade()
                && !flag.replace(true)
            {
                button.set_rgba(&color);
                flag.set(false);
            }
        } else {
            entry.add_css_class("error");
            entry.set_tooltip_text(Some(
                "Color inválido. Usa #RGB, #RRGGBB, #RRGGBBAA, rgb(...) o rgba(...).",
            ));
        }
    });
    let weak = entry.downgrade();
    button.connect_rgba_notify(move |button| {
        if !syncing.get()
            && let Some(entry) = weak.upgrade()
        {
            entry.set_text(&color_code(&button.rgba()));
        }
    });
}

fn color_code(color: &gdk::RGBA) -> String {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b) = (
        channel(color.red()),
        channel(color.green()),
        channel(color.blue()),
    );
    if color.alpha() >= 1.0 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("rgba({r}, {g}, {b}, {:.3})", color.alpha())
    }
}

fn glyph_picker(parent: &adw::Window, target: &gtk::Entry) {
    const FAMILY_LABELS: [&str; 11] = [
        "Todos los iconos",
        "Material Design",
        "Font Awesome",
        "Devicons",
        "Codicons",
        "Octicons",
        "Linux",
        "Weather",
        "Seti",
        "Font Awesome Extension",
        "Powerline",
    ];
    const FAMILY_PREFIXES: [&str; 11] = [
        "", "md-", "fa-", "dev-", "cod-", "oct-", "linux-", "weather-", "seti-", "fae-", "pl-|ple-",
    ];
    const DISPLAY_LIMIT: usize = 120;

    let window = adw::Window::builder()
        .title("Elegir icono Nerd Font")
        .transient_for(parent)
        .modal(true)
        .default_width(700)
        .default_height(560)
        .build();
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Elegir icono Nerd Font"))));
    let close = gtk::Button::with_label("Cerrar");
    let close_window = window.clone();
    close.connect_clicked(move |_| close_window.close());
    header.pack_start(&close);
    layout.append(&header);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_top(16);
    body.set_margin_bottom(16);
    body.set_margin_start(20);
    body.set_margin_end(20);
    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some("Buscar: firefox, terminal, carpeta…"));
    body.append(&search);

    let filters = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let family_label = gtk::Label::new(Some("Colección"));
    family_label.set_xalign(0.0);
    let family = gtk::DropDown::from_strings(&FAMILY_LABELS);
    family.set_hexpand(true);
    family_label.set_mnemonic_widget(Some(&family));
    filters.append(&family_label);
    filters.append(&family);
    body.append(&filters);

    let result_count = gtk::Label::new(None);
    result_count.set_xalign(0.0);
    result_count.add_css_class("caption");
    result_count.add_css_class("dim-label");
    body.append(&result_count);

    let grid = gtk::Grid::builder()
        .column_spacing(6)
        .row_spacing(6)
        .column_homogeneous(true)
        .build();
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&grid)
        .build();
    body.append(&scroll);

    let note = gtk::Label::new(Some(
        "Al elegir se inserta el glifo en el formato de la app. Instala una Nerd Font para previsualizarlo aquí y mostrarlo en Waybar.",
    ));
    note.set_wrap(true);
    note.set_xalign(0.0);
    note.add_css_class("caption");
    note.add_css_class("dim-label");
    body.append(&note);
    layout.append(&body);
    window.set_content(Some(&layout));

    let refresh: Rc<dyn Fn()> = {
        let grid = grid.clone();
        let result_count = result_count.clone();
        let search = search.clone();
        let family = family.clone();
        let target = target.clone();
        let window = window.clone();
        Rc::new(move || {
            populate_glyph_grid(
                &grid,
                &result_count,
                search.text().as_str(),
                FAMILY_PREFIXES
                    .get(family.selected() as usize)
                    .copied()
                    .unwrap_or(""),
                &target,
                &window,
                DISPLAY_LIMIT,
            );
        })
    };
    let refresh_search = refresh.clone();
    search.connect_search_changed(move |_| refresh_search());
    let refresh_family = refresh.clone();
    family.connect_selected_notify(move |_| refresh_family());
    refresh();
    window.present();
}

fn populate_glyph_grid(
    grid: &gtk::Grid,
    result_count: &gtk::Label,
    query: &str,
    family_prefix: &str,
    target: &gtk::Entry,
    picker: &adw::Window,
    display_limit: usize,
) {
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }
    let (glyphs, total) = crate::glyphs::search(query, family_prefix, display_limit);
    for (index, glyph) in glyphs.iter().enumerate() {
        let button = gtk::Button::with_label(&glyph.character);
        button.add_css_class("glyph-choice");
        button.set_size_request(48, 44);
        button.set_tooltip_text(Some(&format!(
            "{} · U+{}",
            glyph.name,
            glyph.code.to_uppercase()
        )));
        button.update_property(&[gtk::accessible::Property::Label(glyph.name.as_str())]);
        let character = glyph.character.clone();
        let target = target.clone();
        let picker = picker.clone();
        button.connect_clicked(move |_| {
            target.set_text(&character);
            picker.close();
        });
        grid.attach(&button, (index % 10) as i32, (index / 10) as i32, 1, 1);
    }

    if query.trim().is_empty() && family_prefix.is_empty() {
        result_count.set_text(&format!(
            "{total} destacados · {} iconos en el catálogo; busca o filtra por colección para explorar el resto.",
            crate::glyphs::catalog().len()
        ));
    } else if total == 0 {
        result_count.set_text("No se encontraron iconos. Prueba otro nombre o colección.");
    } else if total > display_limit {
        result_count.set_text(&format!(
            "{total} resultados · se muestran los primeros {display_limit}; afina la búsqueda para encontrar uno concreto."
        ));
    } else {
        result_count.set_text(&format!("{total} iconos"));
    }
}

fn field_row(title: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_mnemonic_widget(Some(widget));
    row.append(&label);
    row.append(widget);
    row
}

struct FormDialog {
    window: adw::Window,
    body: gtk::Box,
    save: gtk::Button,
}

fn form_dialog(parent: &adw::ApplicationWindow, title: &str, description: &str) -> FormDialog {
    let window = adw::Window::builder()
        .title(title)
        .transient_for(parent)
        .modal(true)
        .default_width(600)
        .default_height(420)
        .build();
    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some(title))));
    let cancel = gtk::Button::with_label("Cancelar");
    let copy = window.clone();
    cancel.connect_clicked(move |_| copy.close());
    header.pack_start(&cancel);
    let save = gtk::Button::with_label("Guardar");
    save.add_css_class("suggested-action");
    header.pack_end(&save);
    layout.append(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
    body.add_css_class("dialog-body");
    let note = gtk::Label::new(Some(description));
    note.set_wrap(true);
    note.set_xalign(0.0);
    note.add_css_class("dim-label");
    body.append(&note);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&body)
        .build();
    layout.append(&scroll);
    window.set_content(Some(&layout));
    FormDialog { window, body, save }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}
fn clear_list(container: &gtk::ListBox) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn empty_state(title: &str, description: &str, icon: &str) -> adw::StatusPage {
    adw::StatusPage::builder()
        .title(title)
        .description(description)
        .icon_name(icon)
        .build()
}

fn module_name(module: &str) -> String {
    match module {
        "hyprland/workspaces" => "Workspaces".into(),
        "hyprland/window" => "Ventana activa".into(),
        "clock" | "custom/clock" => "Reloj".into(),
        "battery" => "Batería".into(),
        "network" => "Red".into(),
        "pulseaudio" => "Volumen".into(),
        "backlight" => "Brillo".into(),
        "tray" => "Bandeja".into(),
        "cpu" => "CPU".into(),
        "memory" => "Memoria".into(),
        "bluetooth" => "Bluetooth".into(),
        _ => module
            .strip_prefix("custom/")
            .unwrap_or(module)
            .replace('-', " "),
    }
}

fn preview_uses_icon_font(token: &str) -> bool {
    // Nerd Font icons live in Unicode private-use ranges. Keep normal text,
    // digits and punctuation in the readable UI font, even when localized.
    token.chars().any(|character| {
        matches!(character as u32, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD)
            || character == '⏻'
    })
}

fn preview_token(module: &str, format: &str) -> String {
    if !format.is_empty()
        && !format.contains('{')
        && !format.contains('<')
        && format.chars().count() <= 5
    {
        return format.into();
    }
    match module {
        "hyprland/workspaces" => "1  2  3".into(),
        "battery" => "BAT".into(),
        "network" => "NET".into(),
        "pulseaudio" => "VOL".into(),
        "bluetooth" => "BT".into(),
        "backlight" => "BRI".into(),
        "tray" => "···".into(),
        "clock" | "custom/clock" => "12:00".into(),
        _ => module_name(module)
            .chars()
            .take(3)
            .collect::<String>()
            .to_uppercase(),
    }
}

fn slug(value: &str) -> String {
    let value: String = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = value.trim_matches('-');
    if trimmed.is_empty() {
        "mi-app".into()
    } else {
        trimmed.into()
    }
}

fn json_filter() -> gtk::FileFilter {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Preset de Waybar Editor (*.json)"));
    filter.add_pattern("*.json");
    filter
}

fn backup_name(path: &std::path::Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Respaldo");
    let seconds = stem
        .split('-')
        .next()
        .and_then(|value| value.parse::<u128>().ok())
        .map(|value| (value / 1_000_000_000) as i64);
    seconds
        .and_then(|seconds| glib::DateTime::from_unix_local(seconds).ok())
        .and_then(|time| time.format("%d/%m/%Y · %H:%M:%S").ok())
        .map(|date| date.to_string())
        .unwrap_or_else(|| stem.into())
}

fn change_summary(before: &Document, after: &Document) -> String {
    let mut lines = Vec::new();
    for index in 0..3 {
        let before_modules = before.modules(ZONES[index]).unwrap_or_default();
        let after_modules = after.modules(ZONES[index]).unwrap_or_default();
        if before_modules != after_modules {
            lines.push(format!(
                "{}: {} → {} módulos (orden o visibilidad cambiados)",
                ZONE_NAMES[index],
                before_modules.len(),
                after_modules.len()
            ));
        }
    }
    if before.config != after.config {
        lines.push("Se guardarán las opciones editadas en JSONC.".into());
    }
    if before.css != after.css {
        lines.push("Se guardarán cambios de apariencia en CSS.".into());
    }
    if lines.is_empty() {
        "Sin cambios en los archivos.".into()
    } else {
        lines.join("\n")
    }
}

fn missing_dependencies(document: &Document) -> Vec<String> {
    let mut missing = Vec::new();
    let Ok(value) = document.value() else {
        return missing;
    };
    let Some(root) = value.as_object() else {
        return missing;
    };
    for module in root.values().filter_map(Value::as_object) {
        for key in [
            "exec",
            "on-click",
            "on-click-right",
            "on-scroll-up",
            "on-scroll-down",
        ] {
            if let Some(command) = module.get(key).and_then(Value::as_str) {
                // Best effort: only explicit file paths, not arbitrary shell syntax.
                for token in command.split_whitespace() {
                    let token = token.trim_matches(['\'', '"', ';']);
                    if token.starts_with('/') || token.starts_with("~/") {
                        let expanded = if let Some(relative) = token.strip_prefix("~/") {
                            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(relative)
                        } else {
                            PathBuf::from(token)
                        };
                        if !expanded.exists()
                            && !token.contains(['$', '*', '|', '(', ')'])
                            && !missing.contains(&expanded.display().to_string())
                        {
                            missing.push(expanded.display().to_string());
                        }
                    }
                }
            }
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schematic_does_not_execute_or_render_live_formats() {
        assert_eq!(preview_token("battery", "{capacity}%"), "BAT");
        assert_eq!(preview_token("custom/files", "󰉋"), "󰉋");
        assert_eq!(slug("org.Example App.desktop"), "org-example-app-desktop");
    }
    #[test]
    fn preview_font_keeps_numbers_and_text_out_of_nerd_font() {
        for text in [
            "1  2  3",
            "MED",
            "BRI",
            "12:00",
            "+2",
            "···",
            "Zona vacía",
            "Ñ",
        ] {
            assert!(!preview_uses_icon_font(text), "{text}");
        }
        for icon in ["󰉋", "", "⏻"] {
            assert!(preview_uses_icon_font(icon), "{icon}");
        }
    }

    #[test]
    fn chosen_color_code_preserves_channels_and_transparency() {
        assert_eq!(
            color_code(&gdk::RGBA::new(101.0 / 255.0, 217.0 / 255.0, 1.0, 1.0)),
            "#65d9ff"
        );
        assert_eq!(
            color_code(&gdk::RGBA::new(101.0 / 255.0, 217.0 / 255.0, 1.0, 0.6)),
            "rgba(101, 217, 255, 0.600)"
        );
    }

    #[test]
    fn dependencies_detect_missing_explicit_script() {
        let doc = Document {
            config: r#"{"custom/x":{"on-click":"/nonexistent/waybar-test-file"}}"#.into(),
            css: String::new(),
        };
        assert_eq!(missing_dependencies(&doc).len(), 1);
    }
}
