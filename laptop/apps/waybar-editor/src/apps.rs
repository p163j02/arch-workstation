use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub command: String,
}

pub fn installed_apps() -> Vec<InstalledApp> {
    #[derive(Debug)]
    enum UserEntry {
        App(InstalledApp),
        Blocked,
    }

    let mut user_entries: HashMap<String, UserEntry> = HashMap::new();
    for path in user_desktop_dirs() {
        for file in find_desktop_files(&path) {
            if let Some((id, parsed)) = parse_desktop_file(&file, &path) {
                match parsed {
                    DesktopParse::App(app) => {
                        user_entries.insert(id, UserEntry::App(app));
                    }
                    DesktopParse::Hidden | DesktopParse::Skip => {
                        user_entries.insert(id, UserEntry::Blocked);
                    }
                }
            }
        }
    }

    let blocked: HashSet<String> = user_entries
        .iter()
        .filter_map(|(id, v)| match v {
            UserEntry::Blocked => Some(id.clone()),
            UserEntry::App(_) => None,
        })
        .collect();

    let mut merged: BTreeMap<String, InstalledApp> = BTreeMap::new();
    for (id, entry) in user_entries {
        if let UserEntry::App(app) = entry {
            merged.insert(id, app);
        }
    }

    for path in system_desktop_dirs() {
        for file in find_desktop_files(&path) {
            if let Some((id, parsed)) = parse_desktop_file(&file, &path) {
                if merged.contains_key(&id) || blocked.contains(&id) {
                    continue;
                }
                if let DesktopParse::App(app) = parsed {
                    merged.insert(id, app);
                }
            }
        }
    }

    let mut apps = merged.into_values().collect::<Vec<_>>();
    apps.sort_by_cached_key(|app| app.name.to_lowercase());
    apps
}

#[derive(Debug)]
enum DesktopParse {
    App(InstalledApp),
    Hidden,
    Skip,
}

fn parse_desktop_file(path: &Path, base_dir: &Path) -> Option<(String, DesktopParse)> {
    let id = desktop_id(path, base_dir)?;
    let raw = fs::read_to_string(path).ok()?;
    let entry = parse_desktop_entry(&raw);
    if entry.section != "Desktop Entry" {
        return Some((id, DesktopParse::Skip));
    }

    let hidden = entry.bool("Hidden") || entry.bool("NoDisplay");
    if hidden {
        return Some((id, DesktopParse::Hidden));
    }
    if entry.get("Type") != Some("Application") {
        return Some((id, DesktopParse::Skip));
    }

    let name = entry
        .localized_name()
        .or_else(|| entry.get("Name").map(|s| s.to_string()))?;
    let icon = entry.get("Icon").unwrap_or("").to_string();

    let exec_raw = entry.get("Exec").map(|s| s.to_string());
    let dbus = entry.bool("DBusActivatable");
    let exec_raw = match (exec_raw, dbus) {
        (None, true) => return Some((id, DesktopParse::Skip)),
        (None, false) => return Some((id, DesktopParse::Skip)),
        (Some(e), _) => e,
    };

    if let Some(try_exec) = entry.get("TryExec")
        && !command_available(try_exec)
    {
        return Some((id, DesktopParse::Skip));
    }

    let command = match process_exec(&exec_raw, &name, &icon, path) {
        Some(c) => {
            if entry.bool("Terminal") {
                format!("kitty -e {c}")
            } else {
                c
            }
        }
        None => return Some((id, DesktopParse::Skip)),
    };

    Some((
        id.clone(),
        DesktopParse::App(InstalledApp {
            id,
            name,
            icon,
            command,
        }),
    ))
}

#[derive(Default)]
struct DesktopEntry {
    section: String,
    values: HashMap<String, String>,
}

impl DesktopEntry {
    fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }
    fn bool(&self, key: &str) -> bool {
        self.get(key)
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }
    fn localized_name(&self) -> Option<String> {
        self.get("Name[es_CL]")
            .or_else(|| self.get("Name[es]"))
            .or_else(|| self.get("Name"))
            .map(|s| s.to_string())
    }
}

fn parse_desktop_entry(content: &str) -> DesktopEntry {
    let mut out = DesktopEntry::default();
    let mut current = String::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current = line[1..line.len() - 1].to_string();
            if out.section.is_empty() {
                out.section = current.clone();
            }
            continue;
        }
        if current != "Desktop Entry" {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let k = line[..eq].trim().to_string();
            let v = line[eq + 1..].trim().to_string();
            out.values.insert(k, v);
        }
    }
    out
}

fn process_exec(exec: &str, name: &str, icon: &str, desktop_path: &Path) -> Option<String> {
    let tokens = shell_split(exec)?;
    let mut out = Vec::new();
    for tok in tokens {
        match tok.as_str() {
            "%f" | "%F" | "%u" | "%U" => continue,
            "%i" => {
                if !icon.is_empty() {
                    out.push("--icon".to_string());
                    out.push(icon.to_string());
                }
                continue;
            }
            "%c" => {
                out.push(name.to_string());
                continue;
            }
            "%k" => {
                out.push(desktop_path.to_string_lossy().to_string());
                continue;
            }
            _ => {}
        }

        let replaced = if tok.contains('%') {
            replace_codes_in_token(&tok, name, icon, desktop_path)?
        } else {
            tok
        };

        if replaced.is_empty() {
            continue;
        }
        out.push(replaced);
    }
    if out.is_empty() {
        return None;
    }

    let cmd = out
        .iter()
        .map(|s| shell_quote(s))
        .collect::<Vec<_>>()
        .join(" ");

    Some(cmd)
}

fn replace_codes_in_token(
    tok: &str,
    name: &str,
    icon: &str,
    desktop_path: &Path,
) -> Option<String> {
    let mut out = String::new();
    let chars = tok.chars().collect::<Vec<_>>();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' {
            if i + 1 >= chars.len() {
                return None;
            }
            match chars[i + 1] {
                '%' => {
                    out.push('%');
                    i += 2;
                }
                'c' => {
                    out.push_str(name);
                    i += 2;
                }
                'k' => {
                    out.push_str(&desktop_path.to_string_lossy());
                    i += 2;
                }
                'i' => {
                    if !icon.is_empty() {
                        out.push_str("--icon ");
                        out.push_str(icon);
                    }
                    i += 2;
                }
                'f' | 'F' | 'u' | 'U' => return None,
                _ => return None,
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    Some(out)
}

fn shell_split(input: &str) -> Option<Vec<String>> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut saw_token = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
                saw_token = true;
            }
            '"' if !in_single => {
                in_double = !in_double;
                saw_token = true;
            }
            '\\' if !in_single => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                    saw_token = true;
                }
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if saw_token || !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                    saw_token = false;
                }
            }
            _ => {
                cur.push(c);
                saw_token = true;
            }
        }
    }
    if in_single || in_double {
        return None;
    }
    if saw_token || !cur.is_empty() {
        args.push(cur);
    }
    Some(args)
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".into();
    }
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._/:=@".contains(c))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn command_available(cmd: &str) -> bool {
    if cmd.is_empty() {
        return false;
    }
    if cmd.starts_with('/') {
        return fs::metadata(cmd)
            .map(|m| !m.permissions().readonly())
            .unwrap_or(false);
    }
    if cmd.contains('/') {
        return fs::metadata(cmd).is_ok();
    }
    if cmd.contains('=') {
        return true;
    }
    let path = env::var("PATH").unwrap_or_default();
    for p in path.split(':') {
        let full = Path::new(p).join(cmd);
        if fs::metadata(full).is_ok() {
            return true;
        }
    }
    false
}

fn desktop_id(path: &Path, base_dir: &Path) -> Option<String> {
    let rel = path.strip_prefix(base_dir).ok()?;
    let file = rel.file_name()?.to_string_lossy();
    if !file.ends_with(".desktop") {
        return None;
    }

    let parent = rel.parent().unwrap_or_else(|| Path::new(""));
    let parent_text = parent.to_string_lossy().trim_matches('/').to_string();
    if parent_text.is_empty() || parent_text == "." {
        Some(file.to_string())
    } else {
        Some(format!("{}-{}", parent_text.replace('/', "-"), file))
    }
}

fn find_desktop_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for e in entries.flatten() {
            let p = e.path();
            if let Ok(ft) = e.file_type() {
                if ft.is_dir() {
                    stack.push(p);
                } else if ft.is_file() && p.extension().and_then(|x| x.to_str()) == Some("desktop")
                {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn user_desktop_dirs() -> Vec<PathBuf> {
    let home = env::var("HOME").unwrap_or_else(|_| String::new());
    let data_home = env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"));
    vec![PathBuf::from(data_home).join("applications")]
}

fn system_desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let xdg =
        env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for d in xdg.split(':') {
        if !d.is_empty() {
            dirs.push(PathBuf::from(d).join("applications"));
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_processing() {
        let p = Path::new("/tmp/a.desktop");
        let cmd = process_exec("app %f --name=%c %% %k %i", "Mi App", "icon-x", p).unwrap();
        assert!(cmd.contains("app"));
        assert!(
            cmd.contains("--name=Mi App")
                || cmd.contains(r"--name=Mi\ App")
                || cmd.contains("'--name=Mi App'")
        );
        assert!(cmd.contains("%"));
        assert!(cmd.contains("/tmp/a.desktop"));
        assert!(cmd.contains("icon-x"));
    }

    #[test]
    fn parse_localized_name() {
        let raw = r#"
[Desktop Entry]
Type=Application
Name=App
Name[es]=Aplicación
Exec=app
"#;
        let e = parse_desktop_entry(raw);
        assert_eq!(e.localized_name().unwrap(), "Aplicación");
    }

    #[test]
    fn split_exec_with_quotes() {
        let v = shell_split("env A=1 app --title 'Hola Mundo'").unwrap();
        assert_eq!(v[0], "env");
        assert_eq!(v[3], "--title");
        assert_eq!(v[4], "Hola Mundo");
    }

    #[test]
    fn split_exec_preserves_empty_quoted_args() {
        let v = shell_split("app \"\" '' fin").unwrap();
        assert_eq!(v, vec!["app", "", "", "fin"]);
    }

    #[test]
    fn reject_embedded_file_field_codes() {
        let p = Path::new("/tmp/a.desktop");
        assert!(process_exec("app --open=%f", "App", "", p).is_none());
    }

    #[test]
    fn desktop_id_uses_base_dir_hierarchy() {
        let base = Path::new("/x/share/applications");
        let nested = Path::new("/x/share/applications/kde/foo.desktop");
        assert_eq!(desktop_id(nested, base).unwrap(), "kde-foo.desktop");
    }
}
