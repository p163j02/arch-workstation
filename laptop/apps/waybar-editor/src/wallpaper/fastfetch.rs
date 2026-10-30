use super::theme::{Palette, Paths, write_atomic_with_mode};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const PROFILES: [&str; 6] = ["auto", "bttf", "space", "nature", "arch", "off"];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum PaletteColor {
    Foreground,
    Accent,
    Secondary,
    Background,
    Ansi0,
    Ansi1,
    Ansi2,
    Ansi3,
    Ansi4,
    Ansi5,
    Ansi6,
    Ansi7,
    Ansi8,
    Ansi9,
    Ansi10,
    Ansi11,
    Ansi12,
    Ansi13,
    Ansi14,
    Ansi15,
}

impl PaletteColor {
    pub const ANSI: [Self; 16] = [
        Self::Ansi0,
        Self::Ansi1,
        Self::Ansi2,
        Self::Ansi3,
        Self::Ansi4,
        Self::Ansi5,
        Self::Ansi6,
        Self::Ansi7,
        Self::Ansi8,
        Self::Ansi9,
        Self::Ansi10,
        Self::Ansi11,
        Self::Ansi12,
        Self::Ansi13,
        Self::Ansi14,
        Self::Ansi15,
    ];

    pub fn ansi_index(self) -> usize {
        match self {
            Self::Foreground => 15,
            Self::Accent => 4,
            Self::Secondary => 5,
            Self::Background => 0,
            _ => Self::ANSI.iter().position(|color| *color == self).unwrap(),
        }
    }

    pub fn hex(self, palette: &Palette) -> String {
        super::terminal::ansi_colors(palette)[self.ansi_index()].clone()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Settings {
    pub profile: String,
    pub image: String,
    pub heading: String,
    pub footer: String,
    pub memory: String,
    pub disk: String,
    pub width: u32,
    pub text_color: PaletteColor,
    pub key_color: PaletteColor,
    pub frame_color: PaletteColor,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            profile: "auto".into(),
            image: String::new(),
            heading: String::new(),
            footer: String::new(),
            memory: String::new(),
            disk: String::new(),
            width: 28,
            text_color: PaletteColor::Foreground,
            key_color: PaletteColor::Accent,
            frame_color: PaletteColor::Secondary,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !PROFILES.contains(&self.profile.as_str()) {
            bail!("Perfil de fastfetch desconocido: {}", self.profile);
        }
        if !(16..=64).contains(&self.width) {
            bail!("El ancho de fastfetch debe estar entre 16 y 64 columnas");
        }
        for text in [&self.heading, &self.footer, &self.memory, &self.disk] {
            if text.chars().count() > 48 || text.chars().any(char::is_control) {
                bail!("Los textos de fastfetch admiten hasta 48 caracteres en una sola línea");
            }
        }
        Ok(())
    }
}

pub(super) struct Profile {
    pub id: &'static str,
    pub heading: &'static str,
    pub footer: &'static str,
    pub memory: &'static str,
    pub disk: &'static str,
}

pub(super) fn suggested(image: &Path) -> &'static str {
    let name = image
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if name == "bttf" || name.starts_with("dmc-") || name.contains("delorean") {
        "bttf"
    } else if ["galaxy", "universe", "space", "galaxia", "espacio"]
        .iter()
        .any(|word| name.contains(word))
    {
        "space"
    } else if [
        "paisaje",
        "nature",
        "naturaleza",
        "sunset",
        "forest",
        "bosque",
    ]
    .iter()
    .any(|word| name.contains(word))
    {
        "nature"
    } else {
        "arch"
    }
}

pub(super) fn profile(settings: &Settings, wallpaper: &Path) -> Profile {
    let id = if settings.profile == "auto" {
        suggested(wallpaper)
    } else {
        &settings.profile
    };
    match id {
        "bttf" => Profile {
            id: "bttf",
            heading: "TIME CIRCUITS ACTIVE",
            footer: "1.21 GIGAWATTS REQUIRED",
            memory: "Flux Memory",
            disk: "Plutonium",
        },
        "space" => Profile {
            id: "space",
            heading: "MISSION CONTROL",
            footer: "ALL SYSTEMS NOMINAL",
            memory: "Telemetry RAM",
            disk: "Cargo Storage",
        },
        "nature" => Profile {
            id: "nature",
            heading: "INTO THE WILD",
            footer: "TAKE A MOMENT. ENJOY THE VIEW.",
            memory: "Memory",
            disk: "Storage",
        },
        _ => Profile {
            id: "arch",
            heading: "ARCH LINUX",
            footer: "KEEP IT SIMPLE",
            memory: "Memory",
            disk: "Disk",
        },
    }
}

pub(super) fn settings_for(paths: &Paths, image: &Path) -> Result<Settings> {
    let key = image.canonicalize()?.to_string_lossy().into_owned();
    Ok(paths
        .read_state()?
        .fastfetch
        .get(&key)
        .cloned()
        .unwrap_or_default())
}

pub(super) fn image_path(paths: &Paths, wallpaper: &Path, settings: &Settings) -> PathBuf {
    if settings.image.is_empty() {
        paths.fastfetch.join("images").join(format!(
            "wallpaper-studio-{}.png",
            profile(settings, wallpaper).id
        ))
    } else if let Some(relative) = settings.image.strip_prefix("~/") {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(relative)
    } else {
        PathBuf::from(&settings.image)
    }
}

#[derive(Clone)]
pub(super) struct LibraryImage {
    pub path: PathBuf,
    pub name: String,
    pub profile: &'static str,
}

pub(super) fn image_library(paths: &Paths) -> Result<Vec<LibraryImage>> {
    let directory = paths.fastfetch.join("images");
    let mut images = Vec::new();
    for (profile, title) in [
        ("bttf", "Volver al futuro"),
        ("space", "Espacio"),
        ("nature", "Naturaleza"),
        ("arch", "Arch"),
    ] {
        let original = directory.join(format!("wallpaper-studio-{profile}.png"));
        if original.is_file() {
            images.push(LibraryImage {
                path: original,
                name: format!("Original · {title}"),
                profile,
            });
        }
        let folder = directory.join("silphwave").join(profile);
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("No se pudo leer {}", folder.display()));
            }
        };
        let mut files = Vec::new();
        for entry in entries {
            let path = entry?.path();
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if path.is_file()
                && ["png", "jpg", "jpeg", "webp", "gif", "bmp"].contains(&extension.as_str())
            {
                files.push(path);
            }
        }
        files.sort();
        for path in files {
            let name = format!(
                "Silphwave · {}",
                path.file_stem().unwrap_or_default().to_string_lossy()
            );
            images.push(LibraryImage {
                path,
                name,
                profile,
            });
        }
    }
    Ok(images)
}

pub(super) fn text<'a>(custom: &'a str, default: &'a str) -> &'a str {
    if custom.is_empty() { default } else { custom }
}

fn color(hex: &str) -> String {
    let values = [1, 3, 5].map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap());
    format!("38;2;{};{};{}", values[0], values[1], values[2])
}

pub(super) fn config(
    paths: &Paths,
    wallpaper: &Path,
    palette: &Palette,
    settings: &Settings,
) -> Result<Option<Value>> {
    settings.validate()?;
    palette.validate()?;
    if settings.profile == "off" {
        return Ok(None);
    }
    let source = image_path(paths, wallpaper, settings);
    let image = source.canonicalize().with_context(|| {
        format!(
            "No se encontró la imagen de fastfetch: {}",
            source.display()
        )
    })?;
    let metadata = fs::metadata(&image)?;
    if !metadata.is_file() || metadata.len() == 0 {
        bail!(
            "La imagen de fastfetch debe ser un archivo no vacío: {}",
            image.display()
        );
    }
    let preset = profile(settings, wallpaper);
    let key_color = color(&settings.key_color.hex(palette));
    let frame_color = color(&settings.frame_color.hex(palette));
    let text_color = color(&settings.text_color.hex(palette));
    // Fastfetch escapes an opening brace as {{; closing braces are literal.
    let literal = |s: String| s.replace('{', "{{");
    let custom =
        |s: String| json!({"type":"custom", "format":literal(s), "outputColor":frame_color});
    let mut modules = vec![
        json!("break"),
        custom(format!(
            "╭{:─^52}╮",
            format!(" {} ", text(&settings.heading, preset.heading))
        )),
        json!({"type":"title", "color":{"user":key_color, "host":frame_color, "at":text_color}}),
        custom(format!("├{}┤", "─".repeat(52))),
    ];
    for (kind, key) in [
        ("os", "󰣇 OS".to_owned()),
        ("kernel", "󰒋 Kernel".into()),
        ("uptime", "󱦟 Uptime".into()),
        ("packages", "󰏖 Packages".into()),
        ("shell", "󰆍 Shell".into()),
        ("terminal", "󰞷 Terminal".into()),
        ("wm", "󱂬 WM".into()),
        ("cpu", "󰻠 CPU".into()),
        ("gpu", "󰢮 GPU".into()),
        (
            "memory",
            format!("󰍛 {}", text(&settings.memory, preset.memory)),
        ),
        ("disk", format!("󰋊 {}", text(&settings.disk, preset.disk))),
    ] {
        modules.push(json!({"type":kind,"key":key,"keyColor":key_color,"outputColor":text_color}));
    }
    modules.extend([
        custom(format!("├{}┤", "─".repeat(52))),
        json!({"type":"colors", "paddingLeft":6}),
        custom(format!("│ {:<50} │", text(&settings.footer, preset.footer))),
        custom(format!("╰{}╯", "─".repeat(52))),
    ]);
    Ok(Some(json!({
        "$schema":"https://github.com/fastfetch-cli/fastfetch/raw/dev/doc/json_schema.json",
        "logo":{"type":"kitty", "source":image, "width":settings.width, "padding":{"top":2,"right":3,"left":2}},
        "display":{"separator":" │ "}, "modules":modules
    })))
}

pub(super) fn sync_settings(
    paths: &Paths,
    wallpaper: &Path,
    palette: &Palette,
    settings: &Settings,
) -> Result<()> {
    let Some(config) = config(paths, wallpaper, palette, settings)? else {
        return Ok(());
    };
    let destination = paths.fastfetch.join("config.jsonc");
    let backup = paths
        .state
        .with_file_name("fastfetch-backup")
        .join("config.jsonc");
    if !backup.exists() && destination.exists() {
        write_atomic_with_mode(&backup, &fs::read(&destination)?, 0o600)?;
    }
    let output = format!(
        "// Generated by Wallpaper Studio; edit the per-wallpaper settings in the app.\n{}\n",
        serde_json::to_string_pretty(&config)?
    );
    write_atomic_with_mode(&destination, output.as_bytes(), 0o600)
}

pub(super) fn sync(paths: &Paths, wallpaper: &Path, palette: &Palette) -> Result<()> {
    let key = wallpaper.canonicalize()?.to_string_lossy().into_owned();
    if let Some(settings) = paths.read_state()?.fastfetch.get(&key) {
        sync_settings(paths, wallpaper, palette, settings)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Paths, PathBuf, Palette) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let paths = Paths {
            wallpapers: root.join("wallpapers"),
            state: root.join("state/state.json"),
            style: root.join("style.css"),
            hypr: root.join("local.conf"),
            control_style: root.join("control.scss"),
            kitty: root.join("kitty.conf"),
            fastfetch: root.join("fastfetch"),
        };
        fs::create_dir_all(paths.fastfetch.join("images")).unwrap();
        for id in ["bttf", "space", "nature", "arch"] {
            fs::write(
                paths
                    .fastfetch
                    .join(format!("images/wallpaper-studio-{id}.png")),
                b"image decoded by fastfetch",
            )
            .unwrap();
        }
        let wallpaper = root.join("galaxy.png");
        fs::write(&wallpaper, b"wallpaper handled by the backend").unwrap();
        let palette = Palette {
            accent: "#123456".into(),
            secondary: "#abcdef".into(),
            foreground: "#eeeeee".into(),
            background: "#111111".into(),
        };
        (dir, paths, wallpaper, palette)
    }

    #[test]
    fn profile_changes_image_and_text_and_uses_explicit_wallpaper_colors() {
        let (_dir, paths, wallpaper, palette) = fixture();
        for (id, heading, memory) in [
            ("bttf", "TIME CIRCUITS ACTIVE", "Flux Memory"),
            ("space", "MISSION CONTROL", "Telemetry RAM"),
            ("nature", "INTO THE WILD", "Memory"),
            ("arch", "ARCH LINUX", "Memory"),
        ] {
            let settings = Settings {
                profile: id.into(),
                ..Settings::default()
            };
            let config = config(&paths, &wallpaper, &palette, &settings)
                .unwrap()
                .unwrap();
            assert!(
                config["logo"]["source"]
                    .as_str()
                    .unwrap()
                    .ends_with(&format!("wallpaper-studio-{id}.png"))
            );
            assert!(
                config["modules"][1]["format"]
                    .as_str()
                    .unwrap()
                    .contains(heading)
            );
            assert!(config.to_string().contains(memory));
            assert_eq!(config["modules"][4]["keyColor"], "38;2;18;52;86");
        }
        assert_eq!(profile(&Settings::default(), &wallpaper).id, "space");
        assert_eq!(suggested(Path::new("dmc-2.jpg")), "bttf");
        assert_eq!(suggested(Path::new("couple-bus-sunset.jpg")), "nature");
        assert_eq!(suggested(Path::new("unknown.png")), "arch");
    }

    #[test]
    fn synchronization_requires_a_binding_and_preserves_original_backup_once() {
        let (_dir, paths, wallpaper, palette) = fixture();
        let destination = paths.fastfetch.join("config.jsonc");
        fs::write(&destination, "// original with comments\n{}\n").unwrap();
        sync(&paths, &wallpaper, &palette).unwrap();
        assert_eq!(
            fs::read_to_string(&destination).unwrap(),
            "// original with comments\n{}\n"
        );
        let settings = Settings {
            heading: "Hola {mundo}".into(),
            width: 32,
            text_color: PaletteColor::Accent,
            ..Settings::default()
        };
        paths
            .update_state(|state| {
                state
                    .fastfetch
                    .insert(wallpaper.to_string_lossy().into_owned(), settings);
            })
            .unwrap();
        sync(&paths, &wallpaper, &palette).unwrap();
        let updated = fs::read_to_string(&destination).unwrap();
        assert!(updated.contains("Hola {{mundo}"));
        assert!(!updated.contains("MISSION CONTROL"));
        assert!(updated.contains("wallpaper-studio-space.png"));
        assert_eq!(
            settings_for(&paths, &wallpaper).unwrap().text_color,
            PaletteColor::Accent
        );
        sync(&paths, &wallpaper, &palette).unwrap();
        assert_eq!(fs::read_to_string(&destination).unwrap(), updated);
        assert_eq!(
            fs::read_to_string(paths.state.with_file_name("fastfetch-backup/config.jsonc"))
                .unwrap(),
            "// original with comments\n{}\n"
        );
    }

    #[test]
    fn text_colors_use_only_palette_roles_and_follow_palette_edits() {
        let (_dir, paths, wallpaper, mut palette) = fixture();
        for (role, expected) in [
            (PaletteColor::Foreground, "38;2;238;238;238"),
            (PaletteColor::Accent, "38;2;18;52;86"),
            (PaletteColor::Secondary, "38;2;171;205;239"),
            (PaletteColor::Background, "38;2;17;17;17"),
        ] {
            let settings = Settings {
                text_color: role,
                ..Settings::default()
            };
            let result = config(&paths, &wallpaper, &palette, &settings)
                .unwrap()
                .unwrap();
            assert_eq!(result["modules"][4]["outputColor"], expected);
            assert_eq!(result["modules"][2]["color"]["at"], expected);
            // Changing the data color leaves labels and frames on their defaults.
            assert_eq!(result["modules"][4]["keyColor"], "38;2;18;52;86");
            assert_eq!(result["modules"][1]["outputColor"], "38;2;171;205;239");
        }
        let settings = Settings {
            text_color: PaletteColor::Background,
            key_color: PaletteColor::Secondary,
            frame_color: PaletteColor::Accent,
            ..Settings::default()
        };
        palette.secondary = "#654321".into();
        let result = config(&paths, &wallpaper, &palette, &settings)
            .unwrap()
            .unwrap();
        assert_eq!(result["modules"][4]["outputColor"], "38;2;17;17;17");
        assert_eq!(result["modules"][4]["keyColor"], "38;2;101;67;33");
        assert_eq!(result["modules"][1]["outputColor"], "38;2;18;52;86");
        assert_eq!(result["modules"][2]["color"]["user"], "38;2;101;67;33");
        assert_eq!(result["modules"][2]["color"]["host"], "38;2;18;52;86");
    }

    #[test]
    fn existing_settings_keep_default_colors_and_arbitrary_colors_are_rejected() {
        let old: Settings =
            serde_json::from_value(json!({"profile":"nature", "heading":"CUSTOM"})).unwrap();
        assert_eq!(old.text_color, PaletteColor::Foreground);
        assert_eq!(old.key_color, PaletteColor::Accent);
        assert_eq!(old.frame_color, PaletteColor::Secondary);
        assert_eq!(old.heading, "CUSTOM");
        assert!(serde_json::from_value::<Settings>(json!({"text_color":"#ff0000"})).is_err());
        let serialized = serde_json::to_value(&old).unwrap();
        assert_eq!(serialized["text_color"], "foreground");
        assert_eq!(serialized["key_color"], "accent");
        assert_eq!(serialized["frame_color"], "secondary");
        assert!(serde_json::from_value::<Settings>(json!({"text_color":"ansi16"})).is_err());
    }

    #[test]
    fn all_sixteen_terminal_colors_can_be_selected_and_restored() {
        let (_dir, paths, wallpaper, palette) = fixture();
        let colors = super::super::terminal::ansi_colors(&palette);
        for (index, expected) in colors.iter().enumerate() {
            let encoded = json!({"text_color": format!("ansi{index}"), "profile":"space"});
            let settings: Settings = serde_json::from_value(encoded).unwrap();
            assert_eq!(settings.text_color, PaletteColor::ANSI[index]);
            assert_eq!(settings.text_color.ansi_index(), index);
            assert_eq!(
                serde_json::to_value(&settings).unwrap()["text_color"],
                format!("ansi{index}")
            );
            let result = config(&paths, &wallpaper, &palette, &settings)
                .unwrap()
                .unwrap();
            assert_eq!(result["modules"][4]["outputColor"], color(expected));
            assert_eq!(result["modules"][4]["keyColor"], color(&palette.accent));
        }
        let settings = Settings {
            text_color: PaletteColor::Ansi13,
            ..Default::default()
        };
        paths
            .update_state(|state| {
                state
                    .fastfetch
                    .insert(wallpaper.to_string_lossy().into_owned(), settings);
            })
            .unwrap();
        assert_eq!(
            settings_for(&paths, &wallpaper).unwrap().text_color,
            PaletteColor::Ansi13
        );
    }

    #[test]
    fn image_library_groups_variants_keeps_originals_and_uses_selected_file() {
        let (_dir, paths, wallpaper, palette) = fixture();
        let space = paths.fastfetch.join("images/silphwave/space");
        let arch = paths.fastfetch.join("images/silphwave/arch");
        fs::create_dir_all(&space).unwrap();
        fs::create_dir_all(&arch).unwrap();
        for name in ["space-2.png", "space-1.PNG", "ignore.txt", "space-3"] {
            fs::write(space.join(name), b"image").unwrap();
        }
        fs::write(arch.join("arch-1.png"), b"image").unwrap();
        let images = image_library(&paths).unwrap();
        assert_eq!(images.len(), 7);
        let variants: Vec<_> = images
            .iter()
            .filter(|image| image.profile == "space")
            .collect();
        assert_eq!(variants.len(), 3);
        assert_eq!(variants[0].name, "Original · Espacio");
        assert_eq!(variants[1].name, "Silphwave · space-1");
        assert_eq!(variants[2].name, "Silphwave · space-2");
        let settings = Settings {
            profile: "space".into(),
            image: variants[2].path.to_string_lossy().into_owned(),
            ..Default::default()
        };
        let result = config(&paths, &wallpaper, &palette, &settings)
            .unwrap()
            .unwrap();
        assert_eq!(result["logo"]["source"], variants[2].path.to_str().unwrap());
        assert!(
            result["modules"][1]["format"]
                .as_str()
                .unwrap()
                .contains("MISSION CONTROL")
        );
    }

    #[test]
    fn invalid_or_missing_image_preserves_config_and_off_needs_no_assets() {
        let (_dir, paths, wallpaper, palette) = fixture();
        let destination = paths.fastfetch.join("config.jsonc");
        fs::write(&destination, "original").unwrap();
        let mut settings = Settings {
            image: "/missing/image.png".into(),
            ..Settings::default()
        };
        assert!(sync_settings(&paths, &wallpaper, &palette, &settings).is_err());
        assert_eq!(fs::read_to_string(&destination).unwrap(), "original");
        settings.profile = "off".into();
        sync_settings(&paths, &wallpaper, &palette, &settings).unwrap();
        assert_eq!(fs::read_to_string(&destination).unwrap(), "original");
        settings.heading = "two\nlines".into();
        assert!(settings.validate().is_err());
        settings.heading = String::new();
        settings.width = 100;
        assert!(settings.validate().is_err());
    }
}
