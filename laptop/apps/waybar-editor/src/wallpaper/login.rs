use super::theme::{Palette, Paths, write_atomic_with_mode};
use anyhow::{Context, Result, bail};
use gdk_pixbuf::Pixbuf;
use std::fs;
use std::path::Path;

pub(super) fn sync(paths: &Paths, image: &Path, palette: &Palette) -> Result<()> {
    palette.validate()?;
    // Login and lock screens use a bounded, static frame even for animated GIFs.
    let frame = Pixbuf::from_file_at_scale(image, 1920, 1080, true)?;
    let png = frame.save_to_bufferv("png", &[])?;
    publish(paths, &png, palette)
}

fn publish(paths: &Paths, png: &[u8], palette: &Palette) -> Result<()> {
    palette.validate()?;
    let directory = paths
        .state
        .with_file_name("login-theme")
        .canonicalize()
        .context("Falta configurar la integración de SDDM/Hyprlock")?;
    let background = directory.join("background.png");
    let background = background.to_str().context("Ruta de inicio inválida")?;
    if !background
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        bail!("La ruta compartida de inicio contiene caracteres no admitidos");
    }
    let lock = paths.hypr.with_file_name("hyprlock.conf");
    let original = fs::read_to_string(&lock)?;
    if !original.contains("path = $ws_wallpaper") {
        bail!("Hyprlock no está vinculado a Wallpaper Studio; ejecuta setup-login-theme.sh");
    }
    let content = format!(
        "$ws_wallpaper = {background}\n$ws_accent = rgba({}ee)\n$ws_secondary = rgba({}ee)\n$ws_background = rgba({}ee)\n$ws_text = rgba({}ff)",
        &palette.accent[1..],
        &palette.secondary[1..],
        &palette.background[1..],
        &palette.foreground[1..]
    );
    let start = "# wallpaper-studio:begin";
    let end = "# wallpaper-studio:end";
    let managed = regex::Regex::new(r"(?s)# wallpaper-studio:begin.*?# wallpaper-studio:end")?;
    if original.matches(start).count() != 1
        || original.matches(end).count() != 1
        || !managed.is_match(&original)
    {
        bail!("Bloque de colores de Hyprlock incompleto");
    }
    let block = format!("{start}\n{content}\n{end}");
    let themed_lock = managed.replace(&original, regex::NoExpand(&block));
    let config = format!(
        "[General]\nbackground={background}\nprimaryColor={}\naccentColor={}\nbackgroundColor={}\ntextColor={}\nautoColor=false\n",
        palette.accent, palette.secondary, palette.background, palette.foreground
    );
    // Only these exported assets are public; the private wallpaper/state stays private.
    for file in [
        directory.join("background.png"),
        directory.join("theme.conf"),
        lock.clone(),
    ] {
        if fs::symlink_metadata(&file).is_ok_and(|m| m.file_type().is_symlink()) {
            bail!("No se reemplazan enlaces simbólicos: {}", file.display());
        }
    }
    write_atomic_with_mode(Path::new(background), png, 0o644)?;
    write_atomic_with_mode(&directory.join("theme.conf"), config.as_bytes(), 0o644)?;
    write_atomic_with_mode(&lock, themed_lock.as_bytes(), 0o600)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn publishes_only_login_assets_and_preserves_lock_layout() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let paths = Paths {
            wallpapers: root.join("images"),
            state: root.join("state.json"),
            style: root.join("style.css"),
            hypr: root.join("local.conf"),
            control_style: root.join("control.scss"),
            kitty: root.join("kitty/kitty.conf"),
            fastfetch: root.join("fastfetch"),
        };
        fs::create_dir(root.join("public")).unwrap();
        symlink(root.join("public"), root.join("login-theme")).unwrap();
        let lock = root.join("hyprlock.conf");
        let original = include_str!("../../../../hypr/hyprlock.conf");
        fs::write(&lock, original).unwrap();
        let palette = Palette {
            accent: "#123456".into(),
            secondary: "#abcdef".into(),
            background: "#101020".into(),
            foreground: "#fafafa".into(),
        };
        publish(&paths, b"frame", &palette).unwrap();
        let config = fs::read_to_string(root.join("public/theme.conf")).unwrap();
        assert!(config.contains("primaryColor=#123456\naccentColor=#abcdef"));
        assert!(config.contains("autoColor=false"));
        let updated = fs::read_to_string(&lock).unwrap();
        assert!(updated.contains("$ws_accent = rgba(123456ee)"));
        assert_eq!(
            original.split("# wallpaper-studio:end").nth(1),
            updated.split("# wallpaper-studio:end").nth(1)
        );
        assert_eq!(
            fs::metadata(root.join("public/background.png"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
        assert_eq!(
            fs::metadata(&lock).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let private = root.join("private");
        fs::write(&private, "untouched").unwrap();
        fs::remove_file(root.join("public/background.png")).unwrap();
        symlink(&private, root.join("public/background.png")).unwrap();
        assert!(publish(&paths, b"new frame", &palette).is_err());
        assert_eq!(fs::read_to_string(private).unwrap(), "untouched");
        let mut invalid = palette;
        invalid.accent = "#123456\ncommand=bad".into();
        assert!(publish(&paths, b"frame", &invalid).is_err());
    }
}
