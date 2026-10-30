mod displays;
pub(crate) mod store;
mod ui;
pub(crate) mod widgets;

use adw::prelude::*;
use anyhow::{Result, bail};
use std::path::Path;

pub fn cli(args: &[String]) -> Result<bool> {
    match args {
        [flag, path] if flag == "--display-guard" => {
            displays::guard(Path::new(path))?;
            Ok(true)
        }
        [flag] if flag == "--watch-widgets" => {
            widgets::watch()?;
            Ok(true)
        }
        [flag, name] if flag == "--open-sysmon" => {
            widgets::open_sysmon(name, &widgets::Settings::load()?)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
pub fn run(args: &[String]) -> Result<()> {
    if args == ["--help"] || args == ["-h"] {
        println!(
            "Configuración del notebook · Rust/GTK4\n\nUso: pc-settings [--check]\n\nIntegra Wallpaper Studio, Waybar Editor, pantallas y ajustes de widgets.\n--check valida los ajustes guardados y consulta las pantallas sin modificarlas."
        );
        return Ok(());
    }
    if args == ["--check"] {
        widgets::Settings::load()?;
        println!("Ajustes de widgets válidos");
        for m in displays::monitors()? {
            println!("{} · {} · {} modos", m.name, m.description, m.modes().len());
        }
        return Ok(());
    }
    if !args.is_empty() {
        bail!("Argumentos desconocidos. Usa pc-settings --help");
    }
    let app = adw::Application::builder()
        .application_id("io.local.PcSettings")
        .build();
    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        build(app);
    });
    app.run_with_args::<&str>(&[]);
    Ok(())
}
fn build(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Configuración del notebook")
        .default_width(1180)
        .default_height(760)
        .build();
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(
        "Configuración del notebook",
        "Tu escritorio",
    )));
    outer.append(&header);
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    content.set_vexpand(true);
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 12);
    sidebar.set_width_request(190);
    sidebar.add_css_class("navigation-sidebar");
    let nav = gtk::ListBox::new();
    nav.add_css_class("navigation-sidebar");
    nav.set_selection_mode(gtk::SelectionMode::Single);
    let pages = [
        ("displays", "Pantallas", "video-display-symbolic"),
        (
            "wallpapers",
            "Wallpaper Studio",
            "preferences-desktop-wallpaper-symbolic",
        ),
        ("waybar", "Waybar Editor", "view-list-symbolic"),
        ("widgets", "Barra y widgets", "preferences-system-symbolic"),
    ];
    for (_, name, icon) in pages {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        line.set_margin_top(12);
        line.set_margin_bottom(12);
        line.set_margin_start(12);
        line.set_margin_end(12);
        line.append(&gtk::Image::from_icon_name(icon));
        let label = gtk::Label::new(Some(name));
        label.set_xalign(0.);
        line.append(&label);
        nav.append(&line);
    }
    sidebar.append(&nav);
    content.append(&sidebar);
    content.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    let stack = gtk::Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    content.append(&stack);
    outer.append(&content);
    let app_weak = app.downgrade();
    let window_weak = window.downgrade();
    nav.connect_row_selected(move |_, row| {
        let (Some(row), Some(app), Some(window)) = (row, app_weak.upgrade(), window_weak.upgrade())
        else {
            return;
        };
        let name = pages[row.index() as usize].0;
        if stack.child_by_name(name).is_none() {
            let widget = match name {
                "wallpapers" => crate::wallpaper::build_page(&window),
                "waybar" => match crate::storage::Paths::discover() {
                    Ok(paths) => crate::ui::build_page(&app, &window, paths),
                    Err(e) => adw::StatusPage::builder()
                        .title("No se pudo abrir Waybar Editor")
                        .description(format!("{e:#}"))
                        .build()
                        .upcast(),
                },
                "widgets" => ui::widgets_page(&window),
                _ => ui::displays_page(&window),
            };
            stack.add_named(&widget, Some(name));
        }
        stack.set_visible_child_name(name);
        for action_name in ["apply", "save-preset"] {
            if let Some(action) = window
                .lookup_action(action_name)
                .and_then(|a| a.downcast::<gtk::gio::SimpleAction>().ok())
            {
                action.set_enabled(name == "waybar");
            }
        }
    });
    window.set_content(Some(&outer));
    nav.select_row(nav.row_at_index(0).as_ref());
    window.present();
}
