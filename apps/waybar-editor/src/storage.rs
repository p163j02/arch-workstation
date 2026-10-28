use crate::model::Document;
use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub css: PathBuf,
    pub data: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct Bundle {
    version: u8,
    document: Document,
}

impl Paths {
    pub fn discover() -> Result<Self> {
        let home = env::var("HOME").context("No se encontró HOME")?;
        let config_home = PathBuf::from(
            env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config")),
        );
        let data_home = PathBuf::from(
            env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share")),
        );

        let base = config_home.join("waybar");
        let config_jsonc = base.join("config.jsonc");
        let config = if config_jsonc.exists() {
            config_jsonc
        } else {
            base.join("config")
        };

        Ok(Self {
            config,
            css: base.join("style.css"),
            data: data_home.join("waybar-editor"),
        })
    }

    pub fn load(&self) -> Result<Document> {
        ensure_distinct_active_files(&self.config, &self.css)?;
        ensure_not_symlink(&self.config)?;
        ensure_not_symlink(&self.css)?;

        let config = fs::read_to_string(&self.config)
            .with_context(|| format!("No se pudo leer {}", self.config.display()))?;
        let css = fs::read_to_string(&self.css)
            .with_context(|| format!("No se pudo leer {}", self.css.display()))?;
        let doc = Document { config, css };
        doc.validate()?;
        Ok(doc)
    }

    pub fn apply(&self, document: &Document, baseline: &Document) -> Result<PathBuf> {
        ensure_distinct_active_files(&self.config, &self.css)?;
        ensure_not_symlink(&self.config)?;
        ensure_not_symlink(&self.css)?;

        let current = self.load()?;
        if current.config != baseline.config || current.css != baseline.css {
            bail!(
                "Los archivos de Waybar cambiaron externamente desde la última carga. Recarga antes de aplicar."
            );
        }

        document.validate()?;
        self.ensure_save_dirs()?;
        let backup = Bundle {
            version: 1,
            document: current,
        };
        let backup_path = self.data.join("backups").join(format!(
            "{}-{}.json",
            unix_ts_nanos()?,
            std::process::id()
        ));
        write_bundle_atomic(&backup_path, &backup, true)?;

        write_text_atomic(&self.config, &document.config, false)?;
        if let Err(css_err) = write_text_atomic(&self.css, &document.css, false) {
            match write_text_atomic(&self.config, &baseline.config, false) {
                Ok(()) => {
                    return Err(anyhow!(
                        "Falló escritura de CSS; se revirtió config. Error CSS: {css_err}"
                    ));
                }
                Err(rb_err) => {
                    return Err(anyhow!(
                        "Falló escritura de CSS y rollback de config. Error CSS: {css_err}; rollback: {rb_err}"
                    ));
                }
            }
        }

        Ok(backup_path)
    }

    pub fn list_presets(&self) -> Result<Vec<String>> {
        let dir = self.data.join("presets");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json")
                && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
            {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn save_preset(&self, name: &str, document: &Document) -> Result<()> {
        let safe = validate_name(name)?;
        document.validate()?;
        self.ensure_save_dirs()?;
        let path = self.data.join("presets").join(format!("{safe}.json"));
        if path.exists() {
            bail!("Ya existe un preset con ese nombre");
        }
        let bundle = Bundle {
            version: 1,
            document: document.clone(),
        };
        write_bundle_atomic(&path, &bundle, true)
    }

    pub fn load_preset(&self, name: &str) -> Result<Document> {
        let safe = validate_name(name)?;
        let path = self.data.join("presets").join(format!("{safe}.json"));
        load_bundle(&path).map(|b| b.document)
    }

    pub fn rename_preset(&self, old: &str, new: &str) -> Result<()> {
        let old = validate_name(old)?;
        let new = validate_name(new)?;
        let from = self.data.join("presets").join(format!("{old}.json"));
        let to = self.data.join("presets").join(format!("{new}.json"));
        if !from.exists() {
            bail!("El preset de origen no existe");
        }
        if to.exists() {
            bail!("Ya existe un preset con el nombre destino");
        }
        self.ensure_save_dirs()?;

        let bundle = load_bundle(&from)?;
        write_bundle_atomic(&to, &bundle, true)?;
        fs::remove_file(&from)?;
        Ok(())
    }

    pub fn delete_preset(&self, name: &str) -> Result<()> {
        let safe = validate_name(name)?;
        let path = self.data.join("presets").join(format!("{safe}.json"));
        if !path.exists() {
            bail!("El preset no existe");
        }
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn export_preset(&self, name: &str, path: &Path) -> Result<()> {
        let doc = self.load_preset(name)?;
        let bundle = Bundle {
            version: 1,
            document: doc,
        };
        if path.exists() {
            bail!("El destino de exportación ya existe");
        }
        ensure_not_symlink(path)?;
        write_bundle_atomic(path, &bundle, true)
    }

    pub fn import_preset(&self, path: &Path, name: &str) -> Result<()> {
        let bundle = load_bundle(path)?;
        bundle.document.validate()?;
        let safe = validate_name(name)?;
        self.ensure_save_dirs()?;
        let dst = self.data.join("presets").join(format!("{safe}.json"));
        if dst.exists() {
            bail!("Ya existe un preset con ese nombre");
        }
        ensure_not_symlink(&dst)?;
        write_bundle_atomic(&dst, &bundle, true)
    }

    pub fn list_backups(&self) -> Result<Vec<PathBuf>> {
        let dir = self.data.join("backups");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for e in fs::read_dir(dir)? {
            let e = e?;
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("json") {
                let m = e.metadata()?.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                entries.push((m, p));
            }
        }
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        Ok(entries.into_iter().map(|(_, p)| p).collect())
    }

    pub fn load_backup(&self, path: &Path) -> Result<Document> {
        ensure_not_symlink(path)?;
        load_bundle(path).map(|b| b.document)
    }

    fn ensure_save_dirs(&self) -> Result<()> {
        fs::create_dir_all(self.data.join("presets"))?;
        fs::create_dir_all(self.data.join("backups"))?;
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        bail!("Nombre inválido. No puede estar vacío");
    }
    if trimmed.starts_with('.') {
        bail!("Nombre inválido. No puede iniciar con '.'");
    }
    if trimmed.chars().count() > 64 {
        bail!("Nombre inválido. Máximo 64 caracteres");
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        bail!("Nombre inválido. No puede contener separadores de ruta");
    }
    if trimmed
        .chars()
        .any(|c| c.is_control() || matches!(c, '\n' | '\r' | '\t'))
    {
        bail!("Nombre inválido. No puede contener controles");
    }
    Ok(trimmed.to_string())
}

fn load_bundle(path: &Path) -> Result<Bundle> {
    ensure_not_symlink(path)?;
    let raw = fs::read_to_string(path)
        .with_context(|| format!("No se pudo leer bundle {}", path.display()))?;
    let bundle: Bundle = serde_json::from_str(&raw)
        .with_context(|| format!("Bundle inválido en {}", path.display()))?;
    if bundle.version != 1 {
        bail!("Versión de bundle no soportada: {}", bundle.version);
    }
    bundle.document.validate()?;
    Ok(bundle)
}

fn ensure_not_symlink(path: &Path) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path)
        && meta.file_type().is_symlink()
    {
        bail!("No se permite escribir/leer mediante enlaces simbólicos");
    }
    Ok(())
}

fn ensure_distinct_active_files(config: &Path, css: &Path) -> Result<()> {
    if config == css {
        bail!("Las rutas de config y CSS deben ser distintas");
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(a), Ok(b)) = (fs::metadata(config), fs::metadata(css))
            && a.dev() == b.dev()
            && a.ino() == b.ino()
        {
            bail!("Las rutas de config y CSS deben apuntar a archivos distintos");
        }
    }

    #[cfg(not(unix))]
    {
        if let (Ok(a), Ok(b)) = (fs::canonicalize(config), fs::canonicalize(css)) {
            if a == b {
                bail!("Las rutas de config y CSS deben apuntar a archivos distintos");
            }
        }
    }

    Ok(())
}

fn write_bundle_atomic(path: &Path, bundle: &Bundle, no_overwrite: bool) -> Result<()> {
    let content = serde_json::to_string_pretty(bundle)?;
    write_text_atomic_with_mode(path, &content, no_overwrite, Some(0o600))
}

fn write_text_atomic(path: &Path, content: &str, no_overwrite: bool) -> Result<()> {
    write_text_atomic_with_mode(path, content, no_overwrite, None)
}

fn write_text_atomic_with_mode(
    path: &Path,
    content: &str,
    no_overwrite: bool,
    forced_mode: Option<u32>,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Ruta sin directorio padre: {}", path.display()))?;
    fs::create_dir_all(parent)?;

    ensure_not_symlink(path)?;

    let ts = unix_ts_nanos()?;
    let pid = std::process::id();
    let tmp = parent.join(format!(".tmp-{}-{}.tmp", pid, ts));

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    let mut file = opts
        .open(&tmp)
        .with_context(|| format!("No se pudo crear temporal {}", tmp.display()))?;

    #[cfg(unix)]
    {
        let mode = if let Some(m) = forced_mode {
            m
        } else {
            fs::metadata(path)
                .map(|m| m.permissions().mode() & 0o7777)
                .unwrap_or(0o644)
        };
        fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
    }

    if let Err(e) = file.write_all(content.as_bytes()) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    if let Err(e) = file.sync_all() {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    drop(file);

    ensure_not_symlink(path)?;
    if no_overwrite {
        match fs::hard_link(&tmp, path) {
            Ok(()) => {
                fs::remove_file(&tmp)?;
                return Ok(());
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&tmp);
                bail!("El archivo destino ya existe: {}", path.display());
            }
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                return Err(e.into());
            }
        }
    }

    let rename_res = fs::rename(&tmp, path);
    if let Err(e) = rename_res {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

fn unix_ts_nanos() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Reloj del sistema inválido")?
        .as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn temp_paths() -> (TempDir, Paths) {
        let td = TempDir::new().unwrap();
        let root = td.path();
        let p = Paths {
            config: root.join("cfg/config"),
            css: root.join("cfg/style.css"),
            data: root.join("data"),
        };
        fs::create_dir_all(p.config.parent().unwrap()).unwrap();
        fs::write(&p.config, "{}").unwrap();
        fs::write(&p.css, "").unwrap();
        (td, p)
    }

    #[test]
    fn preset_conflict_and_unicode_name_validation() {
        let (_td, p) = temp_paths();
        let d = Document {
            config: "{}".into(),
            css: "".into(),
        };
        p.save_preset("ok", &d).unwrap();
        assert!(p.save_preset("ok", &d).is_err());

        p.save_preset("Tema Ñandú Azul", &d).unwrap();
        assert_eq!(p.load_preset("Tema Ñandú Azul").unwrap(), d);

        assert!(p.save_preset("../bad", &d).is_err());
        assert!(p.save_preset(".oculto", &d).is_err());
        assert!(p.save_preset("", &d).is_err());
    }

    #[test]
    fn apply_detects_external_changes() {
        let (_td, p) = temp_paths();
        let baseline = p.load().unwrap();
        fs::write(&p.config, "{\"x\":1}").unwrap();
        let new_doc = Document {
            config: "{}".into(),
            css: "".into(),
        };
        assert!(p.apply(&new_doc, &baseline).is_err());
    }

    #[test]
    fn backup_list_latest_first() {
        let (_td, p) = temp_paths();
        let d1 = Document {
            config: "{}".into(),
            css: "".into(),
        };
        p.save_preset("a", &d1).unwrap();
        let b = p.load().unwrap();
        p.apply(
            &Document {
                config: "{\"a\":1}".into(),
                css: "".into(),
            },
            &b,
        )
        .unwrap();
        let b2 = p.load().unwrap();
        p.apply(
            &Document {
                config: "{\"a\":2}".into(),
                css: "".into(),
            },
            &b2,
        )
        .unwrap();
        let backups = p.list_backups().unwrap();
        assert!(backups.len() >= 2);
    }

    #[test]
    fn import_export_bundle_version_checked() {
        let (_td, p) = temp_paths();
        let d = Document {
            config: "{}".into(),
            css: "".into(),
        };
        p.save_preset("exp", &d).unwrap();
        let out = p.data.join("export.json");
        p.export_preset("exp", &out).unwrap();
        assert!(p.export_preset("exp", &out).is_err());
        p.import_preset(&out, "imp").unwrap();
        assert_eq!(p.load_preset("imp").unwrap(), d);
    }

    #[test]
    fn reject_invalid_document_in_presets_and_import() {
        let (_td, p) = temp_paths();
        let invalid = Document {
            config: "[{}]".into(),
            css: "".into(),
        };
        assert!(p.save_preset("invalido", &invalid).is_err());

        let bad_bundle = p.data.join("bad.json");
        fs::create_dir_all(p.data.as_path()).unwrap();
        fs::write(
            &bad_bundle,
            r#"{"version":1,"document":{"config":"[{}]","css":""}}"#,
        )
        .unwrap();
        assert!(p.import_preset(&bad_bundle, "x").is_err());
    }

    #[test]
    fn load_rejects_identical_config_and_css_paths() {
        let (_td, mut p) = temp_paths();
        p.css = p.config.clone();
        let err = p.load().unwrap_err().to_string();
        assert!(err.contains("rutas de config y CSS"));
    }

    #[test]
    fn write_text_atomic_no_overwrite_keeps_existing_content() {
        let td = TempDir::new().unwrap();
        let target = td.path().join("x.json");
        fs::write(&target, "primero").unwrap();

        let err = write_text_atomic(&target, "segundo", true)
            .unwrap_err()
            .to_string();
        assert!(err.contains("ya existe"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "primero");
    }

    #[cfg(unix)]
    #[test]
    fn apply_rolls_back_config_when_css_write_fails() {
        let td = TempDir::new().unwrap();
        let config_dir = td.path().join("cfg-a");
        let css_dir = td.path().join("cfg-b");
        let data_dir = td.path().join("data");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&css_dir).unwrap();

        let p = Paths {
            config: config_dir.join("config"),
            css: css_dir.join("style.css"),
            data: data_dir,
        };
        fs::write(&p.config, "{}").unwrap();
        fs::write(&p.css, "").unwrap();

        let baseline = p.load().unwrap();
        fs::set_permissions(&css_dir, fs::Permissions::from_mode(0o500)).unwrap();

        let res = p.apply(
            &Document {
                config: "{\"x\":1}".into(),
                css: "window#waybar {}".into(),
            },
            &baseline,
        );

        fs::set_permissions(&css_dir, fs::Permissions::from_mode(0o700)).unwrap();

        assert!(res.is_err());
        assert_eq!(fs::read_to_string(&p.config).unwrap(), baseline.config);
        assert_eq!(p.list_backups().unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn apply_backup_written_with_private_permissions() {
        let (_td, p) = temp_paths();
        let baseline = p.load().unwrap();
        let backup = p
            .apply(
                &Document {
                    config: "{\"x\":1}".into(),
                    css: "".into(),
                },
                &baseline,
            )
            .unwrap();
        let mode = fs::metadata(backup).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn load_rejects_symlinked_active_files() {
        let (td, p) = temp_paths();
        let real = td.path().join("real-config");
        fs::write(&real, "{}").unwrap();
        fs::remove_file(&p.config).unwrap();
        symlink(&real, &p.config).unwrap();
        assert!(p.load().is_err());
    }
}
