use super::store;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub hz: f64,
    pub raw: String,
}
impl Mode {
    pub fn parse(raw: &str) -> Option<Self> {
        let value = raw.trim().trim_end_matches("Hz");
        let (size, hz) = value.split_once('@')?;
        let (w, h) = size.split_once('x')?;
        let mode = Self {
            width: w.parse().ok()?,
            height: h.parse().ok()?,
            hz: hz.parse().ok()?,
            raw: raw.trim().to_string(),
        };
        (mode.width > 0 && mode.height > 0 && mode.hz.is_finite() && mode.hz > 0.).then_some(mode)
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub refresh_rate: f64,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default = "one")]
    pub scale: f64,
    #[serde(default)]
    pub transform: u32,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub mirror_of: String,
    #[serde(default)]
    pub available_modes: Vec<String>,
}
fn one() -> f64 {
    1.0
}
impl Monitor {
    pub fn modes(&self) -> Vec<Mode> {
        let mut modes: Vec<_> = self
            .available_modes
            .iter()
            .filter_map(|s| Mode::parse(s))
            .collect();
        modes.sort_by(|a, b| {
            b.width
                .cmp(&a.width)
                .then(b.height.cmp(&a.height))
                .then(b.hz.total_cmp(&a.hz))
        });
        modes.dedup_by(|a, b| {
            a.width == b.width && a.height == b.height && (a.hz - b.hz).abs() < 0.001
        });
        modes
    }
    pub fn snapshot(&self) -> Rule {
        Rule {
            name: self.name.clone(),
            mode: format!("{}x{}@{:.6}", self.width, self.height, self.refresh_rate),
            enabled: !self.disabled,
            x: self.x,
            y: self.y,
            scale: self.scale,
            transform: self.transform,
            mirror: if self.mirror_of == "none" {
                String::new()
            } else {
                self.mirror_of.clone()
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub mode: String,
    pub enabled: bool,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: u32,
    pub mirror: String,
}
fn connector(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b))
}
impl Rule {
    pub fn validate(&self) -> Result<()> {
        if !connector(&self.name)
            || (!self.mirror.is_empty() && !connector(&self.mirror))
            || !self.scale.is_finite()
            || !(0.25..=8.).contains(&self.scale)
            || self.transform > 7
            || self.x.abs_diff(0) > 65536
            || self.y.abs_diff(0) > 65536
            || (self.enabled && Mode::parse(&self.mode).is_none())
        {
            bail!("Configuración inválida de {}", self.name);
        }
        Ok(())
    }
    pub fn lua(&self) -> Result<String> {
        self.validate()?;
        let quote = |s: &str| serde_json::to_string(s).unwrap();
        if !self.enabled {
            return Ok(format!(
                "hl.monitor({{output={}, disabled=true}})",
                quote(&self.name)
            ));
        }
        Ok(format!(
            "hl.monitor({{output={},disabled=false,mode={},position={},scale={},transform={},mirror={}}})",
            quote(&self.name),
            quote(self.mode.trim_end_matches("Hz")),
            quote(&format!("{}x{}", self.x, self.y)),
            self.scale,
            self.transform,
            quote(&self.mirror)
        ))
    }
    pub fn config(&self) -> Result<String> {
        self.validate()?;
        if !self.enabled {
            return Ok(format!("monitor = {}, disable", self.name));
        }
        let mirror = if self.mirror.is_empty() {
            String::new()
        } else {
            format!(", mirror, {}", self.mirror)
        };
        Ok(format!(
            "monitor = {}, {}, {}x{}, {}, transform, {}{mirror}",
            self.name,
            self.mode.trim_end_matches("Hz"),
            self.x,
            self.y,
            self.scale,
            self.transform
        ))
    }
}
pub fn monitors() -> Result<Vec<Monitor>> {
    let text = store::command("hyprctl", &["-j", "monitors", "all"])?;
    let monitors: Vec<Monitor> =
        serde_json::from_str(&text).context("Hyprland devolvió una lista de pantallas inválida")?;
    if monitors.is_empty() {
        bail!("Hyprland no informa pantallas conectadas");
    }
    Ok(monitors)
}
pub fn validate_plan(monitors: &[Monitor], rules: &[Rule]) -> Result<()> {
    if !rules.iter().any(|r| r.enabled) {
        bail!("Debe quedar al menos una pantalla encendida");
    }
    let mut names = std::collections::HashSet::new();
    for rule in rules {
        rule.validate()?;
        if !names.insert(&rule.name) {
            bail!("Pantalla duplicada: {}", rule.name);
        }
        let monitor = monitors
            .iter()
            .find(|m| m.name == rule.name)
            .context("Una pantalla se desconectó; vuelve a detectar")?;
        if rule.enabled {
            let requested = Mode::parse(&rule.mode).context("Modo de pantalla inválido")?;
            // An unadvertised current mode is allowed only unchanged (custom modes
            // installed outside this app). Never invent a 30/60/100/120 Hz mode.
            let current = monitor.snapshot();
            if rule.mode != current.mode
                && !monitor.modes().iter().any(|m| {
                    m.width == requested.width
                        && m.height == requested.height
                        && (m.hz - requested.hz).abs() < 0.001
                })
            {
                bail!("{} no anuncia el modo {}", rule.name, rule.mode);
            }
            if !rule.mirror.is_empty() && !rules.iter().any(|r| r.name == rule.mirror && r.enabled)
            {
                bail!("El destino del espejo ya no está activo");
            }
        }
    }
    if monitors.iter().any(|m| !names.contains(&m.name)) {
        bail!("Cambió la lista de pantallas; vuelve a detectar");
    }
    Ok(())
}
fn apply_rules(rules: &[Rule]) -> Result<()> {
    let mut commands = Vec::new();
    apply_rules_with(rules, |lua| {
        commands.push(lua.to_string());
        Ok(())
    })?;
    let out = store::command("hyprctl", &["eval", &commands.join("\n")])?;
    if out
        .lines()
        .any(|l| !l.trim().is_empty() && l.trim() != "ok")
    {
        bail!("Hyprland: {}", out.trim());
    }
    Ok(())
}
fn apply_rules_with(rules: &[Rule], mut command: impl FnMut(&str) -> Result<()>) -> Result<()> {
    // Enable outputs first, then disable: never turn off the last usable display
    // in the middle of a switch from one output to another.
    for rule in rules
        .iter()
        .filter(|r| r.enabled)
        .chain(rules.iter().filter(|r| !r.enabled))
    {
        let lua = rule.lua()?;
        command(&lua)?;
    }
    Ok(())
}
fn verify_effective(current: &[Monitor], rules: &[Rule]) -> Result<()> {
    for rule in rules {
        let monitor = current.iter().find(|m| m.name == rule.name);
        if !rule.enabled {
            if monitor.is_some_and(|m| !m.disabled) {
                bail!("{} sigue encendida", rule.name);
            }
            continue;
        }
        let monitor = monitor.context("Una pantalla se desconectó durante la prueba")?;
        let mode = Mode::parse(&rule.mode).context("Modo inválido")?;
        if monitor.disabled
            || monitor.width != mode.width
            || monitor.height != mode.height
            || (monitor.refresh_rate - mode.hz).abs() > 0.15
            || (monitor.scale - rule.scale).abs() > 0.01
        {
            bail!("{} no aceptó el modo o la escala solicitados", rule.name);
        }
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct Guard {
    previous: Vec<Rule>,
}
// The separate Rust watchdog restores the outputs even if the GTK process exits.
// Only the live layout changes during a trial; persistence happens on confirmation.
pub struct Trial {
    pending: PathBuf,
    previous: Vec<Rule>,
    desired: Vec<Rule>,
    active: bool,
    expires: Instant,
}
impl Trial {
    pub fn begin(desired: Vec<Rule>) -> Result<Self> {
        let current = monitors()?;
        validate_plan(&current, &desired)?;
        let previous: Vec<_> = current.iter().map(Monitor::snapshot).collect();
        let pending = store::state_home().join(format!("display-{}.pending", store::unique()));
        store::write(
            &pending,
            &serde_json::to_string(&Guard {
                previous: previous.clone(),
            })?,
        )?;
        let expires = Instant::now() + Duration::from_secs(25);
        let spawned = Command::new(std::env::current_exe()?)
            .arg("--display-guard")
            .arg(&pending)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Err(e) = spawned {
            let _ = fs::remove_file(&pending);
            return Err(e.into());
        }
        let mut trial = Self {
            pending,
            previous,
            desired,
            active: true,
            expires,
        };
        let applied = apply_rules(&trial.desired).and_then(|_| {
            // The compositor can acknowledge a rule before committing the mode.
            let mut result = verify_effective(&monitors()?, &trial.desired);
            for _ in 0..4 {
                if result.is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(150));
                result = verify_effective(&monitors()?, &trial.desired);
            }
            result
        });
        if let Err(error) = applied {
            let rollback = trial.revert();
            bail!("No se pudo probar: {error:#}. Restauración: {rollback:?}");
        }
        Ok(trial)
    }
    pub fn confirmation_seconds(&self) -> u64 {
        self.expires
            .saturating_duration_since(Instant::now() + Duration::from_secs(2))
            .as_secs()
            .min(15)
    }
    pub fn confirm(&mut self) -> Result<PathBuf> {
        let _lock = store::desktop_lock()?;
        let current = monitors()?;
        validate_plan(&current, &self.desired)?;
        verify_effective(&current, &self.desired)?;
        let path = store::config_home().join("hypr/local.conf");
        let old = store::read_optional(&path)?;
        let new = merge_saved(old.as_deref().unwrap_or_default(), &self.desired)?;
        // Claim the token before writing: the watchdog can't race confirmation.
        let claimed = self.pending.with_extension("confirmed");
        fs::rename(&self.pending, &claimed)
            .context("La prueba venció; se está restaurando la configuración anterior")?;
        self.active = false;
        let result = store::commit(&[(path, old, new)]);
        let _ = fs::remove_file(claimed);
        if let Err(error) = result {
            let rollback = apply_rules(&self.previous);
            bail!("No se pudo guardar: {error:#}. Restauración: {rollback:?}");
        }
        result
    }
    pub fn revert(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        let claimed = self.pending.with_extension("reverting-ui");
        if fs::rename(&self.pending, &claimed).is_err() {
            self.active = false;
            bail!("La prueba ya venció; el proceso de recuperación está restaurando las pantallas");
        }
        // Leave the token for the watchdog if the UI's recovery fails.
        match apply_rules(&self.previous) {
            Ok(()) => {
                self.active = false;
                let _ = fs::remove_file(claimed);
                Ok(())
            }
            Err(e) => {
                let _ = fs::rename(claimed, &self.pending);
                Err(e)
            }
        }
    }
}
impl Drop for Trial {
    fn drop(&mut self) {
        let _ = self.revert();
    }
}
pub fn guard(path: &Path) -> Result<()> {
    guard_with(path, Duration::from_secs(25), apply_rules)
}
fn guard_with(
    path: &Path,
    timeout: Duration,
    mut restore: impl FnMut(&[Rule]) -> Result<()>,
) -> Result<()> {
    let snapshot: Guard = serde_json::from_str(&fs::read_to_string(path)?)?;
    let start = Instant::now();
    let restoring_ui = path.with_extension("reverting-ui");
    while start.elapsed() < timeout {
        if !path.exists() && !restoring_ui.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let claimed = path.with_extension("reverting");
    let source = if path.exists() {
        path
    } else {
        restoring_ui.as_path()
    };
    if fs::rename(source, &claimed).is_ok() {
        // Retry transient IPC failures, keeping recovery data on final failure.
        let mut result = restore(&snapshot.previous);
        for _ in 0..2 {
            if result.is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
            result = restore(&snapshot.previous);
        }
        result?;
        fs::remove_file(claimed)?;
    }
    Ok(())
}
fn merge_saved(text: &str, rules: &[Rule]) -> Result<String> {
    let mut saved = std::collections::BTreeMap::new();
    if let Some((_, rest)) = text.split_once("# pc-settings:monitors:begin") {
        let (block, _) = rest
            .split_once("# pc-settings:monitors:end")
            .context("Bloque de pantallas incompleto")?;
        for line in block.lines().filter(|l| l.trim().starts_with("monitor = ")) {
            if let Some(name) = line
                .trim()
                .trim_start_matches("monitor = ")
                .split(',')
                .next()
            {
                saved.insert(name.trim().to_string(), line.trim().to_string());
            }
        }
    }
    for rule in rules {
        saved.insert(rule.name.clone(), rule.config()?);
    }
    store::replace_block(
        text,
        "monitors",
        &saved.into_values().collect::<Vec<_>>().join("\n"),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn monitor() -> Monitor {
        serde_json::from_str(r#"{"name":"DP-3","width":1920,"height":1080,"refreshRate":100.0,"availableModes":["1920x1080@100.00Hz","1920x1080@60.00Hz","1280x720@60.00Hz","invalid"]}"#).unwrap()
    }
    #[test]
    fn screen_switch_enables_destination_before_disabling_source() {
        let mut old = monitor().snapshot();
        old.enabled = false;
        let mut new = monitor().snapshot();
        new.name = "HDMI-A-1".into();
        let mut calls = Vec::new();
        apply_rules_with(&[old, new], |lua| {
            calls.push(lua.to_string());
            Ok(())
        })
        .unwrap();
        assert!(calls[0].contains("HDMI-A-1"));
        assert!(calls[1].contains("disabled=true"));
    }
    #[test]
    fn a_silent_compositor_fallback_is_not_saved_as_success() {
        let current = monitor();
        let mut desired = current.snapshot();
        desired.mode = "1280x720@60".into();
        assert!(verify_effective(std::slice::from_ref(&current), &[desired]).is_err());
        assert!(verify_effective(std::slice::from_ref(&current), &[current.snapshot()]).is_ok());
    }
    #[test]
    fn watchdog_restores_expired_trial_and_consumes_token() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("display.pending");
        let previous = vec![monitor().snapshot()];
        fs::write(
            &path,
            serde_json::to_vec(&Guard {
                previous: previous.clone(),
            })
            .unwrap(),
        )
        .unwrap();
        let mut restored = Vec::new();
        guard_with(&path, Duration::ZERO, |rules| {
            restored = rules.to_vec();
            Ok(())
        })
        .unwrap();
        assert_eq!(restored, previous);
        assert!(!path.exists());
        assert!(!path.with_extension("reverting").exists());
    }
    #[test]
    fn watchdog_leaves_confirmed_trial_untouched() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("display.pending");
        fs::write(
            &path,
            serde_json::to_vec(&Guard {
                previous: vec![monitor().snapshot()],
            })
            .unwrap(),
        )
        .unwrap();
        // Simulate confirmation after the watchdog has read the snapshot.
        let claim = path.clone();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            fs::rename(&claim, claim.with_extension("confirmed")).unwrap();
        });
        guard_with(&path, Duration::from_secs(1), |_| {
            panic!("confirmed trial must not revert")
        })
        .unwrap();
        worker.join().unwrap();
    }
    #[test]
    fn only_advertised_modes_and_one_active_output() {
        let m = monitor();
        assert_eq!(m.modes().len(), 3);
        let mut r = m.snapshot();
        r.mode = "1920x1080@120".into();
        assert!(validate_plan(std::slice::from_ref(&m), &[r.clone()]).is_err());
        r.mode = "1920x1080@60.00Hz".into();
        assert!(validate_plan(std::slice::from_ref(&m), &[r.clone()]).is_ok());
        r.enabled = false;
        assert!(validate_plan(&[m], &[r]).is_err());
    }
    #[test]
    fn inputs_cannot_inject_lua_or_hyprlang() {
        let mut r = monitor().snapshot();
        r.name = "DP-3\nexec = bad".into();
        assert!(r.lua().is_err());
        assert!(r.config().is_err());
        r = monitor().snapshot();
        r.mode = "preferred;exec".into();
        assert!(r.lua().is_err());
    }
    #[test]
    fn saved_outputs_preserve_disconnected_and_other_overrides() {
        let a = monitor().snapshot();
        let mut b = a.clone();
        b.name = "HDMI-A-1".into();
        let saved = merge_saved("# personal\n", &[a.clone(), b]).unwrap();
        let updated = merge_saved(&saved, &[a]).unwrap();
        assert!(updated.starts_with("# personal\n"));
        assert!(updated.contains("HDMI-A-1"));
        assert_eq!(updated.matches("monitor = DP-3").count(), 1);
    }
}
