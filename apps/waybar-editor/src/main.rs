mod apps;
mod glyphs;
mod model;
mod preview;
mod storage;
mod ui;

use adw::prelude::*;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Waybar Editor\n\nUsage: waybar-editor [--config PATH] [--style PATH] [--data PATH]\n       waybar-editor --check [--config PATH] [--style PATH]\n\nNative Rust/GTK4 editor. Changes remain drafts until Apply is pressed."
        );
        return;
    }
    let mut paths = match storage::Paths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    };
    let mut check = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--check" => check = true,
            "--config" | "--style" | "--data" => {
                let Some(path) = args.get(index + 1) else {
                    eprintln!("Falta una ruta para {}", args[index]);
                    std::process::exit(2);
                };
                match args[index].as_str() {
                    "--config" => paths.config = PathBuf::from(path),
                    "--style" => paths.css = PathBuf::from(path),
                    _ => paths.data = PathBuf::from(path),
                }
                index += 1;
            }
            other => {
                eprintln!("Argumento desconocido: {other}");
                std::process::exit(2);
            }
        }
        index += 1;
    }
    if check {
        match paths.load() {
            Ok(document) => {
                println!("Configuración válida: {}", paths.config.display());
                println!("CSS válido estructuralmente: {}", paths.css.display());
                for zone in ["modules-left", "modules-center", "modules-right"] {
                    match document.modules(zone) {
                        Ok(modules) => println!("{zone}: {} módulos", modules.len()),
                        Err(error) => {
                            eprintln!("{error:#}");
                            std::process::exit(1);
                        }
                    }
                }
                println!("{} aplicaciones disponibles", apps::installed_apps().len());
            }
            Err(error) => {
                eprintln!("{error:#}");
                std::process::exit(1);
            }
        }
        return;
    }
    let application = adw::Application::builder()
        .application_id("io.local.WaybarEditor")
        .build();
    application.connect_activate(move |application| {
        if let Some(window) = application.active_window() {
            window.present();
        } else {
            ui::build(application, paths.clone());
        }
    });
    application.run_with_args::<&str>(&[]);
}
