use anyhow::{Context, Result, bail};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn config_home() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config"))
}
pub fn state_home() -> PathBuf {
    env::var_os("XDG_STATE_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".local/state")
        })
        .join("pc-settings")
}
pub fn read_optional(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("No se pudo leer {}", path.display())),
    }
}
pub fn write(path: &Path, text: &str) -> Result<()> {
    crate::storage::write_text_atomic_with_mode(path, text, false, Some(0o600))
}
pub fn desktop_lock() -> Result<fs::File> {
    let path = state_home()
        .parent()
        .unwrap()
        .join("wallpaper-studio/state.lock");
    fs::create_dir_all(path.parent().unwrap())?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock().context(
        "Otra aplicación está guardando ajustes del escritorio; vuelve a intentarlo cuando termine",
    )?;
    Ok(file)
}
pub fn replace_block(text: &str, name: &str, content: &str, css: bool) -> Result<String> {
    let marker = |end: &str| {
        if css {
            format!("/* pc-settings:{name}:{end} */")
        } else {
            format!("# pc-settings:{name}:{end}")
        }
    };
    let start = marker("begin");
    let end = marker("end");
    let mut text = text.to_string();
    match (text.find(&start), text.find(&end)) {
        (None, None) => (),
        (Some(a), Some(b))
            if a < b && text.matches(&start).count() == 1 && text.matches(&end).count() == 1 =>
        {
            text.replace_range(a..b + end.len(), "");
        }
        _ => bail!("Bloque de PC Settings incompleto o duplicado: {name}"),
    }
    Ok(format!("{}\n{start}\n{content}\n{end}\n", text.trim_end()))
}
pub fn unique() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}
// Validate and back up every file before the first write. Roll back earlier files
// on failure. A forced shutdown can still interrupt a multi-file update.
pub fn commit(files: &[(PathBuf, Option<String>, String)]) -> Result<PathBuf> {
    let backup = state_home().join("backups").join(unique());
    commit_at(files, backup)
}
fn commit_at(files: &[(PathBuf, Option<String>, String)], backup: PathBuf) -> Result<PathBuf> {
    for (path, old, _) in files {
        if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            bail!("No se reemplazan enlaces: {}", path.display());
        }
        if &read_optional(path)? != old {
            bail!("Cambió {}. Recarga antes de aplicar.", path.display());
        }
    }
    let manifest: Vec<_> = files
        .iter()
        .map(|(p, old, _)| serde_json::json!({"path":p,"content":old}))
        .collect();
    write(
        &backup.join("files.json"),
        &serde_json::to_string_pretty(&manifest)?,
    )?;
    for (index, (path, _, new)) in files.iter().enumerate() {
        if let Err(error) = write(path, new) {
            let mut rollback_errors = Vec::new();
            for (p, old, _) in files[..index].iter().rev() {
                let result = match old {
                    Some(s) => write(p, s),
                    None => fs::remove_file(p).map_err(Into::into),
                };
                if let Err(e) = result {
                    rollback_errors.push(format!("{}: {e}", p.display()));
                }
            }
            bail!(
                "{error:#}; respaldo: {}. Errores de restauración: {}",
                backup.display(),
                rollback_errors.join("; ")
            );
        }
    }
    Ok(backup)
}
// Never leave the GTK thread waiting indefinitely for an unavailable daemon.
pub fn command(program: &str, args: &[&str]) -> Result<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("No se pudo ejecutar {program}"))?;
    // Drain pipes concurrently: monitor JSON may be larger than a pipe buffer.
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let read = |mut pipe: Box<dyn std::io::Read + Send>| {
        let mut data = String::new();
        let _ = pipe.read_to_string(&mut data);
        data
    };
    let out = std::thread::spawn(move || read(Box::new(stdout)));
    let err = std::thread::spawn(move || read(Box::new(stderr)));
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("{program} no respondió en 5 segundos");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let output = out.join().unwrap_or_default();
    let error = err.join().unwrap_or_default();
    if !status.success() {
        bail!("{program}: {} {}", error.trim(), output.trim());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_edits_are_preserved_before_any_write() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.json");
        fs::write(&path, "external").unwrap();
        assert!(
            commit_at(
                &[(path.clone(), Some("old".into()), "new".into())],
                temp.path().join("backup")
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(path).unwrap(), "external");
        assert!(!temp.path().join("backup").exists());
    }
    #[test]
    fn write_failure_restores_previous_files_and_keeps_backup() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("first");
        let blocked = temp.path().join("readonly");
        fs::write(&path, "before").unwrap();
        fs::create_dir(&blocked).unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o500)).unwrap();
        let backup = temp.path().join("backup");
        let result = commit_at(
            &[
                (path.clone(), Some("before".into()), "after".into()),
                (blocked.join("second"), None, "new".into()),
            ],
            backup.clone(),
        );
        fs::set_permissions(blocked, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "before");
        assert!(backup.join("files.json").is_file());
    }
    #[test]
    fn symbolic_links_are_rejected_without_replacing_targets() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target");
        let link = temp.path().join("link");
        fs::write(&target, "before").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(
            commit_at(
                &[(link, Some("before".into()), "after".into())],
                temp.path().join("backup")
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "before");
    }
    #[test]
    fn marked_blocks_preserve_user_settings_and_reject_damage() {
        let old = "# personal\nmonitor = DP-1, preferred, auto, 1\n";
        let once = replace_block(old, "monitors", "monitor = DP-2, disable", false).unwrap();
        let twice = replace_block(
            &once,
            "monitors",
            "monitor = DP-2, preferred, auto, 1",
            false,
        )
        .unwrap();
        assert!(twice.starts_with(old));
        assert!(!twice.contains("disable"));
        assert!(replace_block("# pc-settings:monitors:begin", "monitors", "", false).is_err());
    }
}
