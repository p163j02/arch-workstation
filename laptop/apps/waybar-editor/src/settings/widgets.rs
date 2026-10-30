use super::store;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

pub const PANELS: [(&str, &str); 10] = [
    ("volume", "Audio"),
    ("wifi", "Wi-Fi"),
    ("bluetooth", "Bluetooth"),
    ("airpods", "AirPods"),
    ("brightness", "Brillo"),
    ("calendar", "Calendario"),
    ("profiles", "Energía"),
    ("nightlight", "Luz nocturna"),
    ("power", "Apagado"),
    ("media", "Reproductor"),
];
pub const SECTIONS: [(&str, &str); 7] = [
    ("cpu", "Uso de CPU y frecuencia"),
    ("ram", "Memoria RAM"),
    ("disk", "Disco y actividad de E/S"),
    ("cpu_temp", "Temperatura de CPU"),
    ("gpu_temp", "Temperatura de GPU"),
    ("top_cpu", "Procesos con mayor uso de CPU"),
    ("top_ram", "Procesos con mayor uso de RAM"),
];
pub const ANCHORS: [&str; 9] = [
    "top left",
    "top center",
    "top right",
    "center left",
    "center",
    "center right",
    "bottom left",
    "bottom center",
    "bottom right",
];
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub version: u8,
    pub bar_opacity: f64,
    pub panel_opacity: BTreeMap<String, f64>,
    pub sysmon_opacity: f64,
    pub anchor: String,
    pub x: i32,
    pub y: i32,
    pub sections: BTreeMap<String, bool>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            bar_opacity: 0.82,
            panel_opacity: PANELS
                .iter()
                .map(|(key, _)| (key.to_string(), 0.96))
                .collect(),
            sysmon_opacity: 1.0,
            anchor: "top right".into(),
            x: 12,
            y: 25,
            sections: SECTIONS
                .iter()
                .map(|(key, _)| (key.to_string(), true))
                .collect(),
        }
    }
}
impl Settings {
    pub fn path() -> PathBuf {
        store::config_home().join("pc-settings/widgets.json")
    }
    pub fn load() -> Result<Self> {
        Self::parse(store::read_optional(&Self::path())?.as_deref())
    }
    pub fn parse(text: Option<&str>) -> Result<Self> {
        let mut value: Self = match text {
            Some(s) => serde_json::from_str(s)
                .context("Configuración de widgets dañada; no se sobrescribió")?,
            None => Self::default(),
        };
        value.validate()?;
        for (id, _) in PANELS {
            value.panel_opacity.entry(id.into()).or_insert(0.96);
        }
        for (id, _) in SECTIONS {
            value.sections.entry(id.into()).or_insert(true);
        }
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        let opacity = |a: f64| a.is_finite() && (0.0..=1.0).contains(&a);
        if self.version != 1
            || !opacity(self.bar_opacity)
            || !opacity(self.sysmon_opacity)
            || self
                .panel_opacity
                .iter()
                .any(|(k, v)| !PANELS.iter().any(|(id, _)| id == k) || !opacity(*v))
            || self
                .sections
                .keys()
                .any(|k| !SECTIONS.iter().any(|(id, _)| id == k))
            || !ANCHORS.contains(&self.anchor.as_str())
            || self.x.abs_diff(0) > 16384
            || self.y.abs_diff(0) > 16384
        {
            bail!("Ajustes de widgets inválidos: revisa transparencia, posición y versión");
        }
        Ok(())
    }
    pub fn panel(&self, id: &str) -> f64 {
        self.panel_opacity.get(id).copied().unwrap_or(0.96)
    }
    pub fn visible(&self, id: &str) -> bool {
        self.sections.get(id).copied().unwrap_or(true)
    }
}

const META: &str = "/* pc-settings:bar-opacity ";
// Keep the override in the stylesheet itself, so exported presets are portable.
// Wallpaper Studio and Waybar Editor rebase it whenever their colors change.
pub fn rebase_bar(css: &str) -> Result<String> {
    let Some((_, tail)) = css.split_once(META) else {
        return Ok(css.into());
    };
    let value = tail
        .split_once(" */")
        .context("Metadatos de transparencia incompletos")?
        .0;
    let opacity: f64 = value.parse()?;
    bar_css(css, opacity)
}
pub fn bar_css(css: &str, opacity: f64) -> Result<String> {
    if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
        bail!("Transparencia inválida");
    }
    let palette = regex::Regex::new(r"/\* wallpaper-studio:colors (.*?) \*/")?
        .captures(css)
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c[1]).ok());
    let doc = crate::model::Document {
        config: "{}".into(),
        css: css.into(),
    };
    let bg = palette
        .as_ref()
        .and_then(|p| p["background"].as_str())
        .map(str::to_string)
        .unwrap_or_else(|| doc.appearance().background);
    crate::model::validate_color(&bg)?;
    // Normalize to opaque RGB before alpha(): GTK multiplies existing alpha.
    let (r, g, b, _) =
        crate::model::parse_rgba_components(&bg).context("Color de fondo inválido")?;
    let bg = format!("#{r:02x}{g:02x}{b:02x}");
    let scope = "window#waybar#waybar#waybar#waybar";
    let rule = format!(
        "{META}{opacity:.4} */\n{scope} .modules-left,\n{scope} .modules-center,\n{scope} .modules-right {{ background-color: alpha({bg}, {opacity:.4}); }}"
    );
    store::replace_block(css, "bar", &rule, true)
}
// Append after the palette with greater specificity. GTK3 Wi-Fi and media load
// this same file, and Eww imports it. Text and icons remain fully opaque.
pub fn panel_css(css: &str, settings: &Settings, background: &str) -> Result<String> {
    settings.validate()?;
    crate::model::validate_color(background)?;
    let (r, g, b, _) =
        crate::model::parse_rgba_components(background).context("Color de fondo inválido")?;
    let mut rules = String::new();
    for (id, _) in PANELS {
        let selector = if id == "media" {
            "window.pc-settings-media".into()
        } else {
            format!(".cc-panel.cc-panel.{id}")
        };
        rules.push_str(&format!(
            // Shared with Eww's SCSS compiler and native GTK CSS consumers.
            // Sass owns alpha() as a one-argument function; emit literal RGBA.
            "{selector} {{ background-color: rgba({r}, {g}, {b}, {:.4}); }}\n",
            settings.panel(id)
        ));
    }
    store::replace_block(css, "panels", &rules, true)
}
pub fn rebase_panels(css: &str, background: &str) -> Result<String> {
    let settings = Settings::load()?;
    // Defaults need no override until the settings have actually been saved.
    if !Settings::path().exists() {
        return Ok(css.into());
    }
    panel_css(css, &settings, background)
}
pub fn open_sysmon(name: &str, settings: &Settings) -> Result<()> {
    settings.validate()?;
    if !["sysmon-widget", "sysmon-widget-ext"].contains(&name) {
        bail!("Monitor de recursos desconocido: {name}");
    }
    // Eww 0.6 evaluates window geometry using local window arguments only.
    // Its built-in open overrides work at login and when reopening after edits.
    let pos = format!("--pos={}x{}", settings.x, settings.y);
    let anchor = format!("--anchor={}", settings.anchor);
    store::command("eww", &["open", name, &pos, &anchor])?;
    Ok(())
}

pub fn apply(settings: &Settings, baseline: Option<String>) -> Result<Vec<String>> {
    let _lock = store::desktop_lock()?;
    settings.validate()?;
    let config = store::config_home();
    let style = config.join("waybar/style.css");
    let old_style = store::read_optional(&style)?.context("No se encontró el CSS de Waybar")?;
    let control = config.join("eww/control-center/wallpaper-studio.scss");
    let old_control = store::read_optional(&control)?;
    let palette = regex::Regex::new(r"/\* wallpaper-studio:colors (.*?) \*/")?
        .captures(&old_style)
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c[1]).ok());
    let background = palette
        .as_ref()
        .and_then(|p| p["background"].as_str())
        .unwrap_or("#1a1b26");
    let new_style = bar_css(&old_style, settings.bar_opacity)?;
    let new_control = panel_css(
        old_control.as_deref().unwrap_or_default(),
        settings,
        background,
    )?;
    let backup = store::commit(&[
        (style, Some(old_style), new_style),
        (control, old_control, new_control),
        (
            Settings::path(),
            baseline,
            serde_json::to_string_pretty(settings)?,
        ),
    ])?;
    let mut messages = vec![format!("Ajustes guardados. Respaldo: {}", backup.display())];
    for (program, args) in [
        ("pkill", vec!["-SIGUSR2", "-x", "waybar"]),
        ("eww", vec!["reload"]),
        (
            "eww",
            vec![
                "--config",
                config
                    .join("eww/control-center")
                    .to_str()
                    .unwrap_or_default(),
                "reload",
            ],
        ),
    ] {
        if let Err(e) = store::command(program, &args) {
            // pkill exit 1 simply means the bar isn't currently running.
            if program != "pkill"
                || !store::command("pgrep", &["-x", "waybar"])
                    .unwrap_or_default()
                    .is_empty()
            {
                messages.push(format!("Guardado; no se pudo actualizar en vivo: {e:#}"));
            }
        }
    }
    let assignment = format!("pc_settings={}", serde_json::to_string(settings)?);
    if let Err(e) = store::command("eww", &["update", &assignment]) {
        messages.push(format!(
            "Guardado; no se pudieron actualizar los ajustes del monitor de recursos: {e:#}"
        ));
    }
    // Geometry changes need reopening; retain exactly the windows already open.
    match store::command("eww", &["active-windows"]) {
        Ok(active) => {
            for name in ["sysmon-widget", "sysmon-widget-ext"] {
                if active.lines().any(|l| l.split(':').next() == Some(name))
                    && let Err(e) = store::command("eww", &["close", name])
                        .and_then(|_| open_sysmon(name, settings))
                {
                    messages.push(format!("Widget {name}: {e:#}"));
                }
            }
        }
        Err(e) => messages.push(format!("Guardado; no se pudo consultar Eww: {e:#}")),
    }
    Ok(messages)
}
pub fn watch() -> Result<()> {
    let mut previous = String::new();
    let mut last = Settings::default();
    loop {
        match Settings::load() {
            Ok(s) => last = s,
            Err(e) => eprintln!("PC Settings: {e:#}"),
        }
        let json = serde_json::to_string(&last)?;
        if json != previous {
            let mut out = io::stdout().lock();
            if writeln!(out, "{json}").and_then(|_| out.flush()).is_err() {
                return Ok(());
            }
            previous = json;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_maps_emit_every_field_eww_uses() {
        let settings = Settings::parse(Some(
            r#"{"sections":{"cpu":false},"panel_opacity":{"wifi":0.4}}"#,
        ))
        .unwrap();
        assert_eq!(settings.sections.len(), SECTIONS.len());
        assert!(!settings.visible("cpu"));
        assert!(settings.visible("gpu_temp"));
        assert_eq!(settings.panel("wifi"), 0.4);
    }
    #[test]
    fn rgba_background_alpha_is_replaced_instead_of_multiplied() {
        let mut document = crate::model::Document {
            config: "{}".into(),
            css: String::new(),
        };
        let mut appearance = document.appearance();
        appearance.background = "rgba(12, 34, 56, 0.4)".into();
        document.set_appearance(&appearance).unwrap();
        document.css = bar_css(&document.css, 0.5).unwrap();
        assert!(document.css.contains("alpha(#0c2238, 0.5000)"));
        appearance.background = "#112233".into();
        document.set_appearance(&appearance).unwrap();
        assert!(document.css.contains("alpha(#112233, 0.5000)"));
    }
    #[test]
    fn generated_rules_are_accepted_by_the_waybar_gtk3_parser_when_available() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let css =
            bar_css("", 0.37).unwrap() + &panel_css("", &Settings::default(), "#123456").unwrap();
        let script = "import sys\ntry:\n import gi\n gi.require_version('Gtk','3.0')\n from gi.repository import Gtk\nexcept (ImportError,ValueError): sys.exit(77)\nGtk.CssProvider().load_from_data(sys.stdin.buffer.read())\n";
        let Ok(mut child) = Command::new("python3")
            .args(["-c", script])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        else {
            return;
        };
        child
            .stdin
            .take()
            .unwrap()
            .write_all(css.as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success() || result.status.code() == Some(77),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    #[test]
    fn validation_rejects_bad_saved_settings() {
        for value in [
            r#"{"bar_opacity":2}"#,
            r#"{"anchor":"top right; injected"}"#,
            r#"{"version":2}"#,
            r#"{"panel_opacity":{"unknown":0.5}}"#,
            "{",
        ] {
            assert!(Settings::parse(Some(value)).is_err(), "{value}");
        }
        assert_eq!(Settings::parse(None).unwrap(), Settings::default());
    }
    #[test]
    fn opacity_survives_palette_and_editor_changes() {
        let css = bar_css("/* personal */\n", 0.25).unwrap();
        let changed =
            format!("{css}\n/* wallpaper-studio:colors {{\"background\":\"#123456\"}} */");
        let rebased = rebase_bar(&changed).unwrap();
        assert!(rebased.contains("alpha(#123456, 0.2500)"));
        assert_eq!(rebased.matches("pc-settings:bar:begin").count(), 1);
        assert!(rebased.contains("/* personal */"));
    }
    #[test]
    fn panels_use_independent_background_alpha() {
        let mut s = Settings::default();
        s.panel_opacity.insert("wifi".into(), 0.3);
        let css = panel_css("/* original */", &s, "#112233").unwrap();
        assert!(
            css.contains(".cc-panel.cc-panel.wifi { background-color: rgba(17, 34, 51, 0.3000)")
        );
        assert!(
            css.contains(".cc-panel.cc-panel.volume { background-color: rgba(17, 34, 51, 0.9600)")
        );
        assert!(!css.contains(" opacity:"));
    }
    #[test]
    fn shared_panel_styles_compile_as_scss_when_sassc_is_available() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        // Sass owns alpha() as a one-argument builtin. Valid GTK CSS alone
        // does not establish SCSS compatibility for Eww's imported stylesheet.
        let mut settings = Settings::default();
        settings.panel_opacity.insert("wifi".into(), 0.0);
        settings.panel_opacity.insert("volume".into(), 1.0);
        let source = panel_css("/* shared palette */", &settings, "rgba(12, 19, 35, 0.5)").unwrap();
        let child = Command::new("sassc")
            .arg("-s")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("No se pudo iniciar el compilador SCSS: {error}"),
        };
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let css = String::from_utf8(result.stdout).unwrap();
        assert!(!source.contains("alpha("));
        assert!(source.contains("rgba(12, 19, 35, 0.0000)"));
        assert!(source.contains("rgba(12, 19, 35, 1.0000)"));
        assert!(source.contains("rgba(12, 19, 35, 0.9600)"));
        assert!(css.contains(".cc-panel.cc-panel.wifi"));
        assert!(css.contains("window.pc-settings-media"));
    }
}
