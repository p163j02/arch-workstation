use anyhow::{Context, Result, bail};
use gdk_pixbuf::Pixbuf;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Palette {
    pub accent: String,
    pub secondary: String,
    pub background: String,
    pub foreground: String,
}

impl Palette {
    pub fn validate(&self) -> Result<()> {
        for color in [
            &self.accent,
            &self.secondary,
            &self.background,
            &self.foreground,
        ] {
            if color.len() != 7
                || !color.starts_with('#')
                || !color[1..].bytes().all(|b| b.is_ascii_hexdigit())
            {
                bail!("Color inválido: {color}; se requiere #RRGGBB");
            }
        }
        Ok(())
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct StateFile {
    pub current: Option<String>,
    pub palettes: HashMap<String, Palette>,
    pub folders: Vec<String>,
    pub transition: Transition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Transition {
    pub effect: String,
    pub duration: f64,
}
impl Default for Transition {
    fn default() -> Self {
        Self {
            effect: "fade".into(),
            duration: 2.0,
        }
    }
}
impl Transition {
    fn validate(&self) -> Result<()> {
        if !["none", "fade", "wipe", "wave", "center", "outer", "random"]
            .contains(&self.effect.as_str())
            || !self.duration.is_finite()
            || !(0.1..=10.0).contains(&self.duration)
        {
            bail!("Transición inválida");
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct Paths {
    pub wallpapers: PathBuf,
    pub state: PathBuf,
    pub style: PathBuf,
    pub hypr: PathBuf,
    pub control_style: PathBuf,
    pub kitty: PathBuf,
}
impl Paths {
    pub fn discover() -> Self {
        let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let config = env::var_os("XDG_CONFIG_HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let state = env::var_os("XDG_STATE_HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        Self {
            wallpapers: home.join("wallpapers"),
            state: state.join("wallpaper-studio/state.json"),
            style: config.join("waybar/style.css"),
            hypr: config.join("hypr/local.conf"),
            control_style: config.join("eww/control-center/wallpaper-studio.scss"),
            kitty: config.join("kitty/kitty.conf"),
        }
    }
    pub fn read_state(&self) -> Result<StateFile> {
        match fs::read(&self.state) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("Estado dañado: {}", self.state.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StateFile::default()),
            Err(e) => Err(e).context("No se pudo leer el estado"),
        }
    }
    fn lock(&self) -> Result<File> {
        let lock = self.state.with_extension("lock");
        fs::create_dir_all(lock.parent().context("Ruta de estado inválida")?)?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock)?;
        file.lock()?;
        Ok(file)
    }
    pub fn update_state(&self, update: impl FnOnce(&mut StateFile)) -> Result<()> {
        let _lock = self.lock()?;
        let mut state = self.read_state()?;
        update(&mut state);
        write_atomic(&self.state, &serde_json::to_vec_pretty(&state)?)
    }
}

pub(super) fn palette_for(paths: &Paths, image: &Path) -> Result<Palette> {
    let key = image_key(image)?;
    let state = paths.read_state()?;
    if let Some(palette) = state
        .palettes
        .get(&key)
        .or_else(|| state.palettes.get(&image.to_string_lossy().to_string()))
    {
        palette.validate()?;
        return Ok(palette.clone());
    }
    extract_palette(image)
}

fn image_key(image: &Path) -> Result<String> {
    let path = image
        .canonicalize()
        .with_context(|| format!("El archivo ya no existe: {}", image.display()))?;
    path.to_str()
        .map(str::to_owned)
        .context("La ruta del fondo debe ser UTF-8")
}

pub(super) fn save_palette(paths: &Paths, image: &Path, palette: &Palette) -> Result<()> {
    palette.validate()?;
    let key = image_key(image)?;
    paths.update_state(|state| {
        state.palettes.insert(key, palette.clone());
    })
}
pub(super) fn extract_palette(path: &Path) -> Result<Palette> {
    let pixbuf = Pixbuf::from_file_at_scale(path, 128, 128, true)
        .with_context(|| format!("No se pudo leer {}", path.display()))?;
    Ok(palette_from_pixbuf(&pixbuf))
}

fn palette_from_pixbuf(pixbuf: &Pixbuf) -> Palette {
    let bytes = pixbuf.read_pixel_bytes();
    let pixels = bytes.as_ref();
    let channels = pixbuf.n_channels() as usize;
    let stride = pixbuf.rowstride() as usize;
    let mut samples = Vec::with_capacity((pixbuf.width() * pixbuf.height()) as usize);
    for y in 0..pixbuf.height() as usize {
        for x in 0..pixbuf.width() as usize {
            let i = y * stride + x * channels;
            if i + channels > pixels.len() || (channels > 3 && pixels[i + 3] < 64) {
                continue;
            }
            samples.push([pixels[i], pixels[i + 1], pixels[i + 2]]);
        }
    }
    super::palette::extract(&samples)
}

pub(super) fn apply_wallpaper(
    paths: &Paths,
    image: &Path,
    palette: &Palette,
    transition: &Transition,
) -> Result<Vec<String>> {
    apply_with(paths, image, palette, transition, checked_command)
}

fn apply_with(
    paths: &Paths,
    image: &Path,
    palette: &Palette,
    transition: &Transition,
    command: impl Fn(&str, &[&str]) -> Result<()>,
) -> Result<Vec<String>> {
    palette.validate()?;
    transition.validate()?;
    let key = image_key(image)?;
    if !Path::new(&key).is_file() {
        bail!("No es una imagen: {key}");
    }
    let _lock = paths.lock()?;
    let mut state = paths.read_state()?;
    // Prepare and validate every managed file before changing the wallpaper.
    let old_style = fs::read(&paths.style).context("No se pudo leer el CSS de Waybar")?;
    let css = themed_css(std::str::from_utf8(&old_style)?, palette)?;
    let old_hypr = read_optional(&paths.hypr)?;
    let old_control = read_optional(&paths.control_style)?;
    let old_kitty = read_optional(&paths.kitty)?;
    let kitty = old_kitty
        .as_deref()
        .map(|original| themed_kitty(std::str::from_utf8(original)?, palette))
        .transpose()?;
    let control_css = control_center_css(palette)?;
    let hypr = replace_block(
        std::str::from_utf8(old_hypr.as_deref().unwrap_or_default())?,
        "# wallpaper-studio:begin",
        "# wallpaper-studio:end",
        &format!(
            "general:col.active_border = rgba({}ee) rgba({}ee) 45deg\ngeneral:col.inactive_border = rgba({}aa)",
            &palette.accent[1..],
            &palette.secondary[1..],
            &palette.background[1..]
        ),
    )?;
    for path in [
        &paths.style,
        &paths.hypr,
        &paths.state,
        &paths.control_style,
        &paths.kitty,
    ] {
        if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            bail!("No se reemplazan enlaces simbólicos: {}", path.display());
        }
    }
    write_atomic(&paths.style, css.as_bytes())?;
    let result = (|| -> Result<()> {
        write_atomic(&paths.hypr, hypr.as_bytes())?;
        write_atomic(&paths.control_style, control_css.as_bytes())?;
        if let Some(config) = &kitty {
            write_atomic(&paths.kitty, config.as_bytes())?;
        }
        command(
            "awww",
            &[
                "img",
                "--transition-type",
                &transition.effect,
                "--transition-duration",
                &transition.duration.to_string(),
                "--transition-fps",
                "60",
                "--",
                &key,
            ],
        )
    })();
    if let Err(error) = result {
        let css_rollback = write_atomic(&paths.style, &old_style);
        let hypr_rollback = restore_optional(&paths.hypr, old_hypr.as_deref());
        let control_rollback = restore_optional(&paths.control_style, old_control.as_deref());
        let kitty_rollback = restore_optional(&paths.kitty, old_kitty.as_deref());
        css_rollback.context(format!("{error:#}; además no se pudo restaurar el CSS"))?;
        hypr_rollback.context(format!("{error:#}; además no se pudo restaurar local.conf"))?;
        control_rollback.context(format!(
            "{error:#}; además no se pudo restaurar el tema de Eww"
        ))?;
        kitty_rollback.context(format!(
            "{error:#}; además no se pudo restaurar el tema de Kitty"
        ))?;
        return Err(error);
    }
    state.current = Some(key.clone());
    state.palettes.insert(key, palette.clone());
    state.transition = transition.clone();
    let mut warnings = Vec::new();
    if let Err(error) = write_atomic(&paths.state, &serde_json::to_vec_pretty(&state)?) {
        warnings.push(format!(
            "Fondo aplicado, pero no se pudo guardar la restauración: {error:#}"
        ));
    }
    let batch = format!(
        "keyword general:col.active_border rgba({}ee) rgba({}ee) 45deg ; keyword general:col.inactive_border rgba({}aa)",
        &palette.accent[1..],
        &palette.secondary[1..],
        &palette.background[1..]
    );
    if let Err(error) = command("hyprctl", &["--batch", &batch]) {
        warnings.push(format!(
            "Bordes guardados en local.conf; no se pudieron actualizar en vivo: {error:#}"
        ));
    }
    if let Err(error) = command("pkill", &["-SIGUSR2", "-x", "waybar"]) {
        warnings.push(format!(
            "No se pudo solicitar la recarga de Waybar: {error:#}"
        ));
    }
    if kitty.is_some()
        && let Err(error) = command("pkill", &["-SIGUSR1", "-x", "kitty"])
    {
        warnings.push(format!(
            "Paleta guardada; no se pudo solicitar la recarga de Kitty: {error:#}"
        ));
    }
    if paths.control_style.with_file_name("eww.yuck").is_file() {
        let config = paths.control_style.parent().unwrap().to_string_lossy();
        if let Err(error) = command("eww", &["--config", &config, "reload"]) {
            warnings.push(format!(
                "Paleta guardada; no se pudieron recargar los paneles de Eww: {error:#}"
            ));
        }
    }
    if paths.state.with_file_name("login-theme").is_dir()
        && let Err(error) = super::login::sync(paths, image, palette)
    {
        warnings.push(format!(
            "Fondo aplicado; no se pudo sincronizar SDDM/Hyprlock: {error:#}"
        ));
    }
    Ok(warnings)
}

fn themed_kitty(original: &str, palette: &Palette) -> Result<String> {
    palette.validate()?;
    let midpoint = |offset| -> Result<u16> {
        Ok(
            (u16::from_str_radix(&palette.background[offset..offset + 2], 16)?
                + u16::from_str_radix(&palette.foreground[offset..offset + 2], 16)?)
                / 2,
        )
    };
    let muted = format!(
        "#{:02x}{:02x}{:02x}",
        midpoint(1)?,
        midpoint(3)?,
        midpoint(5)?
    );
    replace_block(
        original,
        "# wallpaper-studio:begin",
        "# wallpaper-studio:end",
        &format!(
            "foreground {fg}\nbackground {bg}\ncursor {accent}\ncursor_text_color {bg}\nselection_foreground {bg}\nselection_background {accent}\nactive_tab_background {accent}\nactive_tab_foreground {bg}\ninactive_tab_background {bg}\ninactive_tab_foreground {fg}\ntab_bar_background {bg}\nactive_border_color {accent}\ninactive_border_color {bg}\ncolor0 {bg}\ncolor8 {muted}\ncolor7 {fg}\ncolor15 {fg}\ncolor4 {accent}\ncolor12 {accent}\ncolor5 {secondary}\ncolor13 {secondary}\ncolor6 {secondary}\ncolor14 {accent}\ncolor1 #e06c75\ncolor9 #e88a93\ncolor2 #4d9a7a\ncolor10 #6ab394\ncolor3 #c9a84c\ncolor11 #d4b86a",
            fg = palette.foreground,
            bg = palette.background,
            accent = palette.accent,
            secondary = palette.secondary,
        ),
    )
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(data) => Ok(Some(data)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("No se pudo leer {}", path.display())),
    }
}

fn restore_optional(path: &Path, data: Option<&[u8]>) -> Result<()> {
    match data {
        Some(data) => write_atomic(path, data),
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        },
    }
}

fn checked_command(program: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("No se pudo ejecutar {program}"))?;
    if !(output.status.success() || program == "pkill" && output.status.code() == Some(1)) {
        bail!(
            "{program}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    if program == "hyprctl"
        && String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| !line.trim().is_empty() && line.trim() != "ok")
    {
        bail!(
            "hyprctl: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        );
    }
    Ok(())
}

fn replace_block(text: &str, start: &str, end: &str, content: &str) -> Result<String> {
    let mut output = text.to_owned();
    loop {
        match (output.find(start), output.find(end)) {
            (None, None) => break,
            (Some(a), Some(b)) if b > a => {
                output.replace_range(a..b + end.len(), "");
            }
            _ => bail!("Bloque de Wallpaper Studio incompleto; revisa el archivo antes de aplicar"),
        }
    }
    Ok(format!(
        "{}\n{start}\n{content}\n{end}\n",
        output.trim_end()
    ))
}

pub(super) fn themed_css(css: &str, palette: &Palette) -> Result<String> {
    palette.validate()?;
    let bg = hex_rgba(&palette.background, 0.82);
    let glow = hex_rgba(&palette.accent, 0.60);
    let inset = hex_rgba(&palette.accent, 0.22);
    let accent = &palette.accent;
    let secondary = &palette.secondary;
    let foreground = &palette.foreground;
    let on_accent = contrasting_text(accent);
    let on_secondary = contrasting_text(secondary);
    // Match the editor's uniform-color scope, then add the actual module/state
    // selectors. A lower-priority color on a parent cannot override a label's
    // own color, and the old active rule lost to the editor's workspace rule.
    let scope = "window#waybar#waybar#waybar";
    replace_block(
        css,
        "/* wallpaper-studio:begin */",
        "/* wallpaper-studio:end */",
        &format!(
            r#"/* wallpaper-studio:colors {colors} */
{scope} {{ color: {foreground}; }}
{scope} .modules-left,
{scope} .modules-center,
{scope} .modules-right {{
    background-color: {bg};
    border-color: {accent};
    box-shadow: 0 0 6px {glow}, inset 0 0 4px {inset};
    color: {foreground};
}}
/* All center launchers, including Wallpaper Studio and newly added apps. */
{scope} .modules-center .module,
{scope} .modules-center .module label,
{scope} .modules-right .module,
{scope} .modules-right .module label {{ color: {secondary}; }}
{scope} .modules-center .module:hover,
{scope} .modules-center .module:hover label,
{scope} .modules-right .module:hover,
{scope} .modules-right .module:hover label {{ color: {accent}; }}
/* Workspace labels need their own rules when uniform editor colors exist. */
{scope} #workspaces button:not(.urgent),
{scope} #workspaces button:not(.urgent) label {{ color: {secondary}; }}
{scope} #workspaces button:hover:not(.urgent) {{ background: {secondary}; color: {on_secondary}; }}
{scope} #workspaces button:hover:not(.urgent) label {{ color: {on_secondary}; }}
{scope} #workspaces button.active:not(.urgent) {{ background: {accent}; color: {on_accent}; }}
{scope} #workspaces button.active:not(.urgent) label {{ color: {on_accent}; }}
{scope} #taskbar button,
{scope} #taskbar button label {{ color: {secondary}; }}
{scope} #taskbar button:hover {{ background: {secondary}; color: {on_secondary}; }}
{scope} #taskbar button:hover label {{ color: {on_secondary}; }}
{scope} #taskbar button.active {{ background: {accent}; color: {on_accent}; border-bottom-color: {secondary}; }}
{scope} #taskbar button.active label {{ color: {on_accent}; }}
{scope} #cpu,
{scope} #network,
{scope} #bluetooth,
{scope} #clock,
{scope} #custom-clock,
{scope} #custom-newws,
{scope} #custom-nightlight {{ color: {accent}; }}
{scope} #memory,
{scope} #pulseaudio:not(.muted),
{scope} #backlight,
{scope} #custom-media,
{scope} #power-profiles-daemon:not(.performance),
{scope} #battery:not(.warning):not(.critical) {{ color: {secondary}; }}
{scope} #battery.warning:not(.charging) {{ color: #e0af68; }}
{scope} #battery.critical:not(.charging) {{ color: #f7768e; }}
{scope} #custom-power:hover {{ color: {accent}; }}"#,
            colors = serde_json::to_string(palette)?,
        ),
    )
}

pub(super) fn control_center_css(palette: &Palette) -> Result<String> {
    palette.validate()?;
    let accent = &palette.accent;
    let secondary = &palette.secondary;
    let foreground = &palette.foreground;
    let background = hex_rgba(&palette.background, 0.96);
    let mix = |amount: f64| {
        let channel =
            |color: &str, start| u8::from_str_radix(&color[start..start + 2], 16).unwrap();
        let values: Vec<_> = [1, 3, 5]
            .into_iter()
            .map(|i| {
                (f64::from(channel(&palette.background, i)) * (1.0 - amount)
                    + f64::from(channel(foreground, i)) * amount)
                    .round() as u8
            })
            .collect();
        format!("#{:02x}{:02x}{:02x}", values[0], values[1], values[2])
    };
    let muted = mix(0.72);
    let card = mix(0.08);
    let track = mix(0.18);
    let on_accent = contrasting_text(accent);
    Ok(format!(
        r#"/* Generated by Wallpaper Studio; imported by Eww and the native Wi-Fi panel. */
.cc-panel.cc-panel {{ background: {background}; color: {foreground}; border-color: {accent}; }}
.cc-panel.cc-panel label, .cc-panel.cc-panel button {{ color: {foreground}; }}
.cc-panel.cc-panel .cc-header, .cc-panel.cc-panel .cc-footer {{ border-color: {track}; }}
.cc-panel.cc-panel .cc-icon, .cc-panel.cc-panel .cc-row-icon,
.cc-panel.cc-panel .cc-accent, .cc-panel.cc-panel .cc-toggle.on,
.cc-panel.cc-panel .cc-spinner, .cc-panel.cc-panel .cc-scan {{ color: {accent}; }}
.cc-panel.cc-panel .cc-secondary, .cc-panel.cc-panel .cc-section,
.cc-panel.cc-panel .cc-close, .cc-panel.cc-panel .cc-gear,
.cc-panel.cc-panel .cc-weekday, .cc-panel.cc-panel .cc-day.outside,
.cc-panel.cc-panel button:disabled, .cc-panel.cc-panel .cc-toggle:not(.on) {{ color: {muted}; }}
.cc-panel.cc-panel .cc-mute, .cc-panel.cc-panel .cc-green,
.cc-panel.cc-panel .cc-info, .cc-panel.cc-panel .cc-day.weekend {{ color: {secondary}; }}
.cc-panel.cc-panel .cc-current, .cc-panel.cc-panel .cc-preset,
.cc-panel.cc-panel .cc-power-option, .cc-panel.cc-panel .cc-pod-battery,
.cc-panel.cc-panel .cc-password, .cc-panel.cc-panel entry {{ background: {card}; color: {foreground}; }}
.cc-panel.cc-panel .cc-battery-pill {{ background: {card}; color: {secondary}; }}
.cc-panel.cc-panel button:hover {{ background: {track}; }}
.cc-panel.cc-panel button:active, .cc-panel.cc-panel .selected,
.cc-panel.cc-panel .cc-day.selected {{ background: {accent}; color: {on_accent}; }}
.cc-panel.cc-panel .selected label, .cc-panel.cc-panel .selected .cc-row-icon,
.cc-panel.cc-panel .selected .cc-accent, .cc-panel.cc-panel .selected .cc-secondary {{ color: {on_accent}; }}
.cc-panel.cc-panel .cc-scan, .cc-panel.cc-panel entry {{ border-color: {track}; }}
.cc-panel.cc-panel button:focus, .cc-panel.cc-panel entry:focus,
.cc-panel.cc-panel .cc-day.today {{ border-color: {accent}; }}
.cc-panel.cc-panel scale trough, .cc-panel.cc-panel progressbar trough,
.cc-panel.cc-panel scrollbar slider {{ background: {track}; }}
.cc-panel.cc-panel scale highlight, .cc-panel.cc-panel progressbar progress {{ background: {accent}; }}
.cc-panel.cc-panel scale slider {{ background: {foreground}; }}
.cc-panel.cc-panel .cc-airpods-image, .cc-panel.cc-panel .cc-pod-icon {{ color: {foreground}; }}
/* Preserve the meaning of errors and destructive actions. */
.cc-panel.cc-panel .cc-error {{ color: #e0af68; }}
.cc-panel.cc-panel .cc-disconnect, .cc-panel.cc-panel .cc-danger,
.cc-panel.cc-panel .cc-danger .cc-row-icon {{ color: #f7768e; }}
"#
    ))
}

pub(super) fn contrasting_text(color: &str) -> &'static str {
    let rgb = u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or_default();
    let linear = |c: u32| {
        let v = f64::from(c) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let lum = 0.2126 * linear((rgb >> 16) & 255)
        + 0.7152 * linear((rgb >> 8) & 255)
        + 0.0722 * linear(rgb & 255);
    if lum > 0.179 { "#10131d" } else { "#ffffff" }
}

fn hex_rgba(color: &str, alpha: f32) -> String {
    let value = u32::from_str_radix(&color[1..], 16).unwrap_or_default();
    format!(
        "rgba({}, {}, {}, {:.2})",
        (value >> 16) & 255,
        (value >> 8) & 255,
        value & 255,
        alpha
    )
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    write_atomic_with_mode(path, data, 0o600)
}

pub(super) fn write_atomic_with_mode(path: &Path, data: &[u8], mode: u32) -> Result<()> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let parent = path.parent().context("Ruta sin directorio padre")?;
    fs::create_dir_all(parent)?;
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("No se reemplazan enlaces simbólicos: {}", path.display());
    }
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&temp)?;
        file.write_all(data)?;
        file.set_permissions(fs::Permissions::from_mode(mode))?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result.with_context(|| format!("No se pudo actualizar {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, Paths, PathBuf, Palette) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            wallpapers: dir.path().join("images"),
            state: dir.path().join("state/state.json"),
            style: dir.path().join("waybar/style.css"),
            hypr: dir.path().join("hypr/local.conf"),
            control_style: dir.path().join("eww/control-center/wallpaper-studio.scss"),
            kitty: dir.path().join("kitty/kitty.conf"),
        };
        fs::create_dir_all(paths.style.parent().unwrap()).unwrap();
        fs::write(
            &paths.style,
            "/* original */\n#battery.critical { color: red; }\n",
        )
        .unwrap();
        let image = dir.path().join("a wallpaper.png");
        fs::write(
            &image,
            b"mock image; backend commands are injected in these tests",
        )
        .unwrap();
        let palette = Palette {
            accent: "#65d9ff".into(),
            secondary: "#bb9af7".into(),
            background: "#10131d".into(),
            foreground: "#f2f4ff".into(),
        };
        (dir, paths, image, palette)
    }

    #[test]
    fn css_preserves_rules_replaces_repeated_blocks_and_keeps_status_colors() {
        let (_dir, paths, _image, palette) = fixture();
        let original = fs::read_to_string(&paths.style).unwrap();
        let first = themed_css(&original, &palette).unwrap();
        let repeated = format!("{first}\n{first}");
        let mut changed = palette.clone();
        changed.accent = "#ffcc00".into();
        let css = themed_css(&repeated, &changed).unwrap();
        assert_eq!(css.matches("/* wallpaper-studio:begin */").count(), 1);
        assert!(css.contains("#battery.critical { color: red; }"));
        assert!(css.contains("background: #ffcc00; color: #10131d;"));
        assert!(!css.contains("#65d9ff"));
        assert!(themed_css("/* wallpaper-studio:begin */", &palette).is_err());
    }

    fn declarations(css: &str, selector: &str) -> String {
        let comments = regex::Regex::new(r"(?s)/\*.*?\*/").unwrap();
        let css = comments.replace_all(css, "");
        let rules = regex::Regex::new(r"(?s)([^{}]+)\{([^{}]*)\}").unwrap();
        rules
            .captures_iter(&css)
            .filter(|rule| {
                rule[1]
                    .split(',')
                    .any(|candidate| candidate.trim() == selector)
            })
            .last()
            .unwrap_or_else(|| panic!("Missing selector: {selector}"))[2]
            .to_owned()
    }

    #[test]
    fn wallpaper_workspace_colors_override_existing_editor_and_uniform_rules() {
        let (_dir, _paths, _image, palette) = fixture();
        let original = include_str!("../../../../waybar/style.css");
        let old_selector = "window#waybar #workspaces button.active";
        assert!(declarations(original, old_selector).contains("background: #62a0ea;"));
        let original = format!("{original}\nwindow#waybar#waybar#waybar * {{ color: #ff00ff; }}");
        let css = themed_css(&original, &palette).unwrap();
        let scope = "window#waybar#waybar#waybar";
        let active_selector = format!("{scope} #workspaces button.active:not(.urgent)");
        let active = declarations(&css, &active_selector);
        assert!(active.contains(&format!("background: {};", palette.accent)));
        assert!(active.contains("color: #10131d;"));
        assert!(active_selector.matches('#').count() > old_selector.matches('#').count());
        let label_selector = format!("{active_selector} label");
        assert!(
            label_selector.matches('#').count()
                > "window#waybar#waybar#waybar *".matches('#').count()
        );
        assert!(declarations(&css, &label_selector).contains("color: #10131d;"));
        assert!(
            declarations(
                &css,
                &format!("{scope} #workspaces button:not(.urgent) label")
            )
            .contains(&palette.secondary)
        );
    }

    #[test]
    fn center_launchers_share_normal_and_hover_colors_without_highlighting_wallpaper() {
        let (_dir, _paths, _image, palette) = fixture();
        let original = include_str!("../../../../waybar/style.css");
        let css = themed_css(original, &palette).unwrap();
        let scope = "window#waybar#waybar#waybar";
        for selector in [
            format!("{scope} .modules-center .module"),
            format!("{scope} .modules-center .module label"),
        ] {
            assert!(
                declarations(&css, &selector).contains(&format!("color: {};", palette.secondary))
            );
        }
        assert!(
            declarations(&css, &format!("{scope} .modules-center .module:hover"))
                .contains(&palette.accent)
        );
        let managed = css.split("/* wallpaper-studio:begin */").last().unwrap();
        assert!(!managed.contains("#custom-wallpaper"));
        assert!(managed.contains("#custom-clock"));
        assert!(managed.contains("#custom-media"));
        // Urgent workspaces and battery alarms retain their original rules.
        assert!(css.contains(&declarations(original, "#workspaces button.urgent")));
        assert!(css.contains(&declarations(original, "#battery.critical:not(.charging)")));
    }

    #[test]
    fn apply_saves_canonical_path_transition_and_persistent_borders() {
        let (_dir, paths, image, palette) = fixture();
        fs::create_dir_all(paths.hypr.parent().unwrap()).unwrap();
        fs::write(&paths.hypr, "# user settings\ninput:kb_layout = latam\n").unwrap();
        let calls = RefCell::new(Vec::new());
        let transition = Transition {
            effect: "wave".into(),
            duration: 1.5,
        };
        let warnings = apply_with(&paths, &image, &palette, &transition, |program, args| {
            calls.borrow_mut().push((
                program.to_owned(),
                args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
            ));
            Ok(())
        })
        .unwrap();
        assert!(warnings.is_empty());
        assert!(!paths.kitty.exists());
        let calls = calls.borrow();
        assert_eq!(
            calls.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(),
            ["awww", "hyprctl", "pkill"]
        );
        assert_eq!(
            &calls[0].1[1..5],
            ["--transition-type", "wave", "--transition-duration", "1.5"]
        );
        assert_eq!(calls[0].1.last().unwrap(), image.to_str().unwrap());
        let state = paths.read_state().unwrap();
        assert_eq!(state.current.as_deref(), image.to_str());
        assert_eq!(state.transition.effect, "wave");
        let hypr = fs::read_to_string(&paths.hypr).unwrap();
        assert!(hypr.contains("input:kb_layout = latam"));
        assert!(hypr.contains("general:col.active_border = rgba(65d9ffee) rgba(bb9af7ee) 45deg"));
        assert_eq!(palette_for(&paths, &image).unwrap(), palette);
    }

    #[test]
    fn kitty_palette_preserves_settings_and_replaces_only_the_managed_block() {
        let (_dir, _paths, _image, palette) = fixture();
        let original = "font_size 10.5\nbackground_opacity 0.82\nmap ctrl+c copy_to_clipboard\n";
        let first = themed_kitty(original, &palette).unwrap();
        let mut changed = palette.clone();
        changed.accent = "#ffcc00".into();
        let updated = themed_kitty(&first, &changed).unwrap();
        assert!(updated.starts_with(original));
        assert_eq!(updated.matches("# wallpaper-studio:begin").count(), 1);
        assert!(updated.contains("cursor #ffcc00\n"));
        assert!(!updated.contains("#65d9ff"));
        assert!(updated.contains("background #10131d\n"));
        assert!(updated.contains("foreground #f2f4ff\n"));
        assert!(updated.contains("color8 #81838e\n"));
        assert!(updated.contains("color1 #e06c75\n"));
        assert!(updated.contains("color2 #4d9a7a\n"));
        assert!(updated.contains("color4 #ffcc00\n"));
        assert!(updated.contains("color5 #bb9af7\n"));
        assert!(themed_kitty("# wallpaper-studio:begin\n", &palette).is_err());
    }

    #[test]
    fn kitty_is_reloaded_after_apply_and_reload_failure_is_reported() {
        for reload_fails in [false, true] {
            let (_dir, paths, image, palette) = fixture();
            fs::create_dir_all(paths.kitty.parent().unwrap()).unwrap();
            fs::write(&paths.kitty, "font_size 10.5\n").unwrap();
            let reloaded = std::cell::Cell::new(false);
            let warnings = apply_with(
                &paths,
                &image,
                &palette,
                &Transition::default(),
                |program, args| {
                    if program == "pkill" && args == ["-SIGUSR1", "-x", "kitty"] {
                        assert!(
                            fs::read_to_string(&paths.kitty)
                                .unwrap()
                                .contains("cursor #65d9ff")
                        );
                        reloaded.set(true);
                        if reload_fails {
                            bail!("reload failed");
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert!(reloaded.get());
            assert_eq!(warnings.len(), usize::from(reload_fails));
            if reload_fails {
                assert!(warnings[0].contains("recarga de Kitty"));
            }
            assert_eq!(
                paths.read_state().unwrap().current.as_deref(),
                image.to_str()
            );
        }
    }

    #[test]
    fn failed_backend_restores_config_and_does_not_claim_success() {
        for existing_hypr in [false, true] {
            let (_dir, paths, image, palette) = fixture();
            if existing_hypr {
                fs::create_dir_all(paths.hypr.parent().unwrap()).unwrap();
                fs::write(&paths.hypr, "# original hypr\n").unwrap();
                fs::create_dir_all(paths.control_style.parent().unwrap()).unwrap();
                fs::write(&paths.control_style, "/* original panel theme */\n").unwrap();
                fs::create_dir_all(paths.kitty.parent().unwrap()).unwrap();
                fs::write(&paths.kitty, "# original kitty\n").unwrap();
            }
            let original = fs::read(&paths.style).unwrap();
            let result = apply_with(
                &paths,
                &image,
                &palette,
                &Transition::default(),
                |program, _| {
                    assert_eq!(program, "awww");
                    bail!("daemon unavailable")
                },
            );
            assert!(result.is_err());
            assert_eq!(fs::read(&paths.style).unwrap(), original);
            assert_eq!(paths.hypr.exists(), existing_hypr);
            assert_eq!(paths.control_style.exists(), existing_hypr);
            assert_eq!(paths.kitty.exists(), existing_hypr);
            if existing_hypr {
                assert_eq!(
                    fs::read_to_string(&paths.kitty).unwrap(),
                    "# original kitty\n"
                );
                assert_eq!(
                    fs::read_to_string(&paths.control_style).unwrap(),
                    "/* original panel theme */\n"
                );
            }
            if existing_hypr {
                assert_eq!(
                    fs::read_to_string(&paths.hypr).unwrap(),
                    "# original hypr\n"
                );
            }
            assert!(paths.read_state().unwrap().current.is_none());
        }
    }

    #[test]
    fn invalid_inputs_do_not_execute_commands_or_overwrite_configuration() {
        let (_dir, paths, image, mut palette) = fixture();
        palette.accent = "#fff; invalid".into();
        assert!(
            apply_with(
                &paths,
                &image,
                &palette,
                &Transition::default(),
                |_, _| panic!("must not execute")
            )
            .is_err()
        );
        fs::create_dir_all(paths.state.parent().unwrap()).unwrap();
        fs::write(&paths.state, "invalid").unwrap();
        assert!(paths.update_state(|s| s.current = None).is_err());
        assert_eq!(fs::read_to_string(&paths.state).unwrap(), "invalid");
    }

    #[test]
    fn state_lock_prevents_lost_folder_updates() {
        let (_dir, paths, _image, _palette) = fixture();
        std::thread::scope(|scope| {
            for i in 0..8 {
                let paths = &paths;
                scope.spawn(move || {
                    paths
                        .update_state(|s| s.folders.push(format!("folder-{i}")))
                        .unwrap()
                });
            }
        });
        assert_eq!(paths.read_state().unwrap().folders.len(), 8);
    }

    #[test]
    fn old_state_and_extracted_palettes_remain_supported() {
        let (_dir, paths, _image, _palette) = fixture();
        fs::create_dir_all(paths.state.parent().unwrap()).unwrap();
        fs::write(&paths.state, r#"{"current":null,"palettes":{}}"#).unwrap();
        assert_eq!(paths.read_state().unwrap().transition.effect, "fade");
        let pixbuf = Pixbuf::new(gdk_pixbuf::Colorspace::Rgb, false, 8, 4, 4).unwrap();
        pixbuf.fill(0x123456ff);
        let palette = palette_from_pixbuf(&pixbuf);
        palette.validate().unwrap();
        assert_eq!(palette, palette_from_pixbuf(&pixbuf));
        assert_eq!(contrasting_text(&palette.background), "#ffffff");
        assert_ne!(palette.accent, "#123456"); // brighten dark dominant colors for text/icons
    }

    #[test]
    fn popup_theme_includes_controls_and_reloads_only_control_center() {
        let (_dir, paths, image, palette) = fixture();
        fs::create_dir_all(paths.control_style.parent().unwrap()).unwrap();
        fs::write(
            paths.control_style.with_file_name("eww.yuck"),
            ";; test config",
        )
        .unwrap();
        let calls = RefCell::new(Vec::new());
        let warnings = apply_with(
            &paths,
            &image,
            &palette,
            &Transition::default(),
            |program, args| {
                if program == "eww" {
                    let theme = fs::read_to_string(&paths.control_style).unwrap();
                    assert!(theme.contains("scale highlight"));
                    assert!(theme.contains("background: #65d9ff;"));
                    assert!(theme.contains(".cc-day.selected"));
                    assert!(theme.contains(".cc-pod-battery"));
                    assert!(theme.contains(".selected .cc-secondary"));
                    calls
                        .borrow_mut()
                        .push(args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>());
                    bail!("daemon unavailable");
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            *calls.borrow(),
            vec![vec![
                "--config".to_owned(),
                paths
                    .control_style
                    .parent()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                "reload".to_owned()
            ]]
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("paneles de Eww"));
        assert!(paths.read_state().unwrap().current.is_some());
    }

    #[test]
    fn live_border_failure_is_reported_after_saving_wallpaper() {
        let (_dir, paths, image, palette) = fixture();
        let warnings = apply_with(
            &paths,
            &image,
            &palette,
            &Transition::default(),
            |program, _| {
                if program == "hyprctl" {
                    bail!("no compositor");
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(warnings.len(), 1);
        assert!(paths.read_state().unwrap().current.is_some());
    }

    #[test]
    fn symlinked_configuration_is_rejected_before_backend_execution() {
        let (_dir, paths, image, palette) = fixture();
        fs::create_dir_all(paths.hypr.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&paths.style, &paths.hypr).unwrap();
        let original = fs::read(&paths.style).unwrap();
        assert!(
            apply_with(
                &paths,
                &image,
                &palette,
                &Transition::default(),
                |_, _| panic!("must not execute")
            )
            .is_err()
        );
        assert_eq!(fs::read(&paths.style).unwrap(), original);
    }
}
