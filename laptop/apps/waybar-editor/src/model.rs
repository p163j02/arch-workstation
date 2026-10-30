use anyhow::{Result, anyhow, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

const MANAGED_BEGIN: &str = "/* waybar-editor:begin */";
const MANAGED_END: &str = "/* waybar-editor:end */";

pub fn launcher_path() -> Result<std::path::PathBuf> {
    let config_home = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(value) if !value.is_empty() => std::path::PathBuf::from(value),
        _ => std::path::PathBuf::from(
            std::env::var_os("HOME").ok_or_else(|| anyhow!("No se encontró HOME"))?,
        )
        .join(".config"),
    };
    Ok(config_home.join("hypr/launch-in-workspace.py"))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub config: String,
    pub css: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BorderAppearance {
    pub color: String,
    pub width: f64,
    pub neon: bool,
    pub glow: f64,
}

impl Default for BorderAppearance {
    fn default() -> Self {
        Self {
            color: "#65d9ff".to_string(),
            width: 0.0,
            neon: false,
            glow: 60.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub background: String,
    pub foreground: String,
    #[serde(default = "default_module_colors")]
    pub module_colors: bool,
    pub accent: String,
    pub font: String,
    pub font_size: f64,
    pub radius: f64,
    pub padding: f64,
    #[serde(default)]
    pub border: BorderAppearance,
}

fn default_module_colors() -> bool {
    true
}

impl Document {
    pub fn validate(&self) -> Result<()> {
        let value = self.value()?;
        if value.is_array() {
            bail!(
                "No se admite configuración de múltiples barras (raíz JSON como arreglo). Usa una sola barra (objeto raíz)."
            );
        }
        if !value.is_object() {
            bail!("La configuración de Waybar debe tener un objeto JSON en la raíz.");
        }
        validate_css_braces(&self.css)?;
        Ok(())
    }

    pub fn value(&self) -> Result<Value> {
        let normalized = normalize_jsonc(&self.config)?;
        let parsed: Value = serde_json::from_str(&normalized)
            .map_err(|e| anyhow!("No se pudo parsear config JSONC: {e}"))?;
        Ok(parsed)
    }

    pub fn modules(&self, zone: &str) -> Result<Vec<String>> {
        ensure_zone(zone)?;
        let root = self.root_object_value()?;
        match root.get(zone) {
            None => Ok(Vec::new()),
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| anyhow!("{zone} debe ser un arreglo de strings"))
                })
                .collect(),
            Some(_) => bail!("{zone} debe ser un arreglo de strings"),
        }
    }

    pub fn all_modules(&self) -> Result<Vec<String>> {
        let mut set = BTreeSet::new();
        for zone in ["modules-left", "modules-center", "modules-right"] {
            for m in self.modules(zone)? {
                set.insert(m);
            }
        }
        let root = self.root_object_value()?;
        for (k, v) in root {
            if v.is_object() && looks_like_module_key(&k) {
                set.insert(k.to_string());
            }
        }
        Ok(set.into_iter().collect())
    }

    /// Modules reachable from zones, including nested groups (cycle-safe).
    pub fn visible_modules(&self) -> Result<BTreeSet<String>> {
        let root = self.root_object_value()?;
        let mut pending = Vec::new();
        for zone in ["modules-left", "modules-center", "modules-right"] {
            pending.extend(self.modules(zone)?);
        }
        let mut visible = BTreeSet::new();
        while let Some(module) = pending.pop() {
            if !visible.insert(module.clone()) {
                continue;
            }
            if module.starts_with("group/")
                && let Some(members) = root
                    .get(&module)
                    .and_then(|value| value.get("modules"))
                    .and_then(Value::as_array)
            {
                pending.extend(members.iter().filter_map(Value::as_str).map(str::to_owned));
            }
        }
        Ok(visible)
    }

    /// Configured modules absent from both zones and reachable groups.
    pub fn hidden_modules(&self) -> Result<Vec<String>> {
        let visible = self.visible_modules()?;
        Ok(self
            .all_modules()?
            .into_iter()
            .filter(|module| !visible.contains(module))
            .collect())
    }

    pub fn hide_module(&mut self, module: &str) -> Result<()> {
        if !self.all_modules()?.iter().any(|name| name == module) {
            bail!("El módulo no pertenece a esta configuración.");
        }
        let root = self.root_object_value()?;
        if let Some(options) = root.get(module) {
            if !options.is_object() {
                bail!(
                    "Las opciones de '{module}' no son un objeto. Corrígelas antes de ocultarlo."
                );
            }
        } else {
            if !looks_like_module_key(module) {
                bail!("El identificador del módulo no es válido: {module}");
            }
            // Defaults have no object yet. Keep their identity in presets/reloads
            // without inventing options or overwriting existing configuration.
            self.set_property(None, module, serde_json::json!({}))?;
        }
        for zone in ["modules-left", "modules-center", "modules-right"] {
            let modules = self
                .modules(zone)?
                .into_iter()
                .filter(|name| name != module)
                .collect::<Vec<_>>();
            self.set_modules(zone, &modules)?;
        }
        Ok(())
    }

    pub fn restore_module(&mut self, module: &str, zone: &str) -> Result<()> {
        ensure_zone(zone)?;
        if !self.hidden_modules()?.iter().any(|name| name == module) {
            bail!("El módulo ya está visible o no pertenece a este borrador.");
        }
        let mut modules = self.modules(zone)?;
        modules.push(module.to_string());
        self.set_modules(zone, &modules)
    }

    pub fn set_modules(&mut self, zone: &str, modules: &[String]) -> Result<()> {
        ensure_zone(zone)?;
        ensure_single_bar_root(&self.config)?;
        let root_span = root_object_span(&self.config)?;
        let props = object_properties(&self.config, root_span)?;
        let replacement = serde_json::to_string(modules)?;
        if let Some(prop) = props.iter().find(|p| p.key == zone) {
            self.config
                .replace_range(prop.value_start..prop.value_end, &replacement);
        } else {
            insert_property_into_object(&mut self.config, root_span, zone, &replacement)?;
        }
        Ok(())
    }

    pub fn set_property(&mut self, module: Option<&str>, key: &str, value: Value) -> Result<()> {
        if key.trim().is_empty() {
            bail!("La clave no puede estar vacía");
        }
        ensure_single_bar_root(&self.config)?;
        let root_span = root_object_span(&self.config)?;
        let value_text = serde_json::to_string(&value)?;

        let target_span = if let Some(module_key) = module {
            let props = object_properties(&self.config, root_span)?;
            if let Some(prop) = props.iter().find(|p| p.key == module_key) {
                let raw = &self.config[prop.value_start..prop.value_end];
                if trimmed_non_comment_start(raw)? != Some('{') {
                    bail!("El módulo '{module_key}' debe ser un objeto para editar propiedades");
                }
                (prop.value_start, prop.value_end)
            } else {
                let module_obj = "{}";
                insert_property_into_object(&mut self.config, root_span, module_key, module_obj)?;
                let updated_root = root_object_span(&self.config)?;
                let props2 = object_properties(&self.config, updated_root)?;
                let p = props2
                    .iter()
                    .find(|p| p.key == module_key)
                    .ok_or_else(|| anyhow!("No se pudo crear el módulo '{module_key}'"))?;
                (p.value_start, p.value_end)
            }
        } else {
            root_span
        };

        let props = object_properties(&self.config, target_span)?;
        if let Some(existing) = props.iter().find(|p| p.key == key) {
            self.config
                .replace_range(existing.value_start..existing.value_end, &value_text);
        } else {
            insert_property_into_object(&mut self.config, target_span, key, &value_text)?;
        }
        Ok(())
    }

    pub fn add_app(&mut self, module: &str, name: &str, icon: &str, command: &str) -> Result<()> {
        if !module.starts_with("custom/") {
            bail!("El módulo para apps debe comenzar con 'custom/'");
        }

        let args = shell_split(command)?;
        if args.is_empty() {
            bail!("El comando de la aplicación no puede estar vacío");
        }

        let mut launch = vec![launcher_path()?.to_string_lossy().into_owned()];
        launch.extend(args);
        let on_click = launch
            .iter()
            .map(|a| shell_quote(a))
            .collect::<Vec<_>>()
            .join(" ");

        let mut staged = self.clone();
        let mut center = staged.modules("modules-center")?;
        if !center.iter().any(|m| m == module) {
            center.push(module.to_string());
            staged.set_modules("modules-center", &center)?;
        }

        staged.set_property(Some(module), "format", Value::String(icon.to_string()))?;
        staged.set_property(
            Some(module),
            "tooltip-format",
            Value::String(name.to_string()),
        )?;
        staged.set_property(Some(module), "tooltip", Value::Bool(true))?;
        staged.set_property(Some(module), "on-click", Value::String(on_click))?;

        *self = staged;
        Ok(())
    }

    pub fn appearance(&self) -> Appearance {
        if let Some(a) = parse_managed_appearance(&self.css) {
            return a;
        }

        let css = strip_css_comments(&self.css).unwrap_or_else(|_| self.css.clone());
        let background = selector_property_any(
            &css,
            &[".modules-left", ".modules-center", ".modules-right"],
            &["background", "background-color"],
        )
        .unwrap_or_else(|| "rgba(30, 30, 46, 0.9)".to_string());
        let foreground = selector_property(&css, &["#clock"], &["color"])
            .unwrap_or_else(|| "#cdd6f4".to_string());
        let accent = selector_property(
            &css,
            &["#workspaces", "button.active"],
            &["background", "background-color"],
        )
        .unwrap_or_else(|| "#89b4fa".to_string());

        let font = selector_property(&css, &["*"], &["font-family"])
            .or_else(|| selector_property(&css, &["window#waybar"], &["font-family"]))
            .map(|v| first_font_family(&v))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "sans-serif".to_string());

        let font_size = selector_property(&css, &["*"], &["font-size"])
            .or_else(|| selector_property(&css, &["window#waybar"], &["font-size"]))
            .and_then(|v| first_px_number(&v))
            .unwrap_or(12.0);

        let radius = selector_property_any(
            &css,
            &[".modules-left", ".modules-center", ".modules-right"],
            &["border-radius"],
        )
        .and_then(|v| first_px_number(&v))
        .unwrap_or(20.0);

        let padding = selector_property(&css, &[".module"], &["padding"])
            .and_then(|v| horizontal_padding_px(&v))
            .unwrap_or(10.0);
        let border = infer_border_from_css(&css);

        Appearance {
            background,
            foreground,
            module_colors: true,
            accent,
            font,
            font_size,
            radius,
            padding,
            border,
        }
    }

    /// Base color for the schematic only: do not pretend to evaluate live state selectors.
    pub fn module_base_color(&self, module: &str) -> Option<String> {
        let id = format!("#{}", module.replace('/', "-"));
        let anchored = format!("window#waybar {id}");
        let mut source = self.css.clone();
        if let Some((start, end)) = managed_block_span(&source) {
            source.replace_range(start..end, "");
        }
        let source = strip_css_comments(&source).ok()?;
        let blocks = Regex::new(r"(?s)([^{}]+)\{([^}]*)\}").ok()?;
        let property = Regex::new(r"(?:^|;)\s*color\s*:\s*([^;]+);").ok()?;
        let mut color = None;
        for cap in blocks.captures_iter(&source) {
            if cap[1]
                .split(',')
                .any(|s| s.trim() == id || s.trim() == anchored)
                && let Some(value) = property.captures(&cap[2])
                && validate_color(value[1].trim()).is_ok()
            {
                color = Some(value[1].trim().to_string());
            }
        }
        color
    }

    pub fn set_appearance(&mut self, appearance: &Appearance) -> Result<()> {
        validate_appearance(appearance)?;
        let block = render_managed_block(appearance, &self.css);
        if let Some((start, end)) = managed_block_span(&self.css) {
            self.css.replace_range(start..end, &block);
        } else {
            if !self.css.ends_with('\n') {
                self.css.push('\n');
            }
            self.css.push_str(&block);
            self.css.push('\n');
        }
        self.css = crate::settings::widgets::rebase_bar(&self.css)?;
        Ok(())
    }

    fn root_object_value(&self) -> Result<serde_json::Map<String, Value>> {
        let value = self.value()?;
        match value {
            Value::Object(map) => Ok(map),
            Value::Array(_) => bail!(
                "No se admite configuración de múltiples barras (raíz JSON como arreglo). Usa una sola barra (objeto raíz)."
            ),
            _ => bail!("La configuración de Waybar debe tener un objeto JSON en la raíz."),
        }
    }
}

const MANAGED_META_PREFIX: &str = "waybar-editor:appearance";

pub fn border_style(border: &BorderAppearance) -> (String, String) {
    let width = border.width.clamp(0.0, 6.0);
    let glow = border.glow.clamp(0.0, 100.0);
    let border_color = if validate_color(&border.color).is_ok() {
        border.color.trim().to_string()
    } else {
        "#65d9ff".to_string()
    };
    let border_css = if width <= 0.0 {
        "none".to_string()
    } else {
        format!("{width}px solid {border_color}")
    };
    let box_shadow_css = if !border.neon || width <= 0.0 {
        "none".to_string()
    } else {
        let alpha = (glow / 100.0).clamp(0.0, 1.0);
        if alpha <= 0.0 {
            "none".to_string()
        } else {
            let tint = with_alpha(&border_color, alpha)
                .unwrap_or_else(|| format!("rgba(101, 217, 255, {alpha:.2})"));
            format!("0 0 6px {tint}, inset 0 0 4px {tint}")
        }
    };
    (border_css, box_shadow_css)
}

fn render_managed_block(a: &Appearance, source_css: &str) -> String {
    // The visual field edits the primary font, not the existing fallback stack.
    let source = strip_css_comments(source_css).unwrap_or_default();
    let existing_fonts = selector_property(&source, &["*"], &["font-family"])
        .or_else(|| selector_property(&source, &["window#waybar"], &["font-family"]));
    let primary = css_font_family_value(&a.font);
    let fonts = existing_fonts
        .as_deref()
        .and_then(|s| s.split_once(','))
        .map(|(_, fallbacks)| format!("{primary}, {}", fallbacks.trim()))
        .unwrap_or(primary);
    let meta = serde_json::to_string(a).unwrap_or_else(|_| "{}".to_string());
    let (border_css, box_shadow_css) = border_style(&a.border);
    // Inherit the fallback from the window, never impose it on each module in
    // preservation mode. Uniform mode deliberately overrides state/hover colors.
    let uniform = if a.module_colors {
        String::new()
    } else {
        format!(
            "window#waybar#waybar#waybar * {{\n  color: {};\n}}\n",
            a.foreground
        )
    };
    let active_color = if a.module_colors {
        String::new()
    } else {
        format!("  color: {};\n", a.foreground)
    };
    format!(
        "{MANAGED_BEGIN}\n/* {MANAGED_META_PREFIX} {meta} */\nwindow#waybar {{\n  color: {fg};\n}}\nwindow#waybar,\nwindow#waybar * {{\n  font-family: {font};\n  font-size: {font_size}px;\n}}\n{uniform}window#waybar .modules-left,\nwindow#waybar .modules-center,\nwindow#waybar .modules-right {{\n  background: {bg};\n  border-radius: {radius}px;\n  border: {border_css};\n  box-shadow: {box_shadow_css};\n}}\nwindow#waybar #workspaces button.active {{\n  background: {accent};\n{active_color}  border-radius: {radius}px;\n}}\nwindow#waybar .module {{\n  padding: 0 {padding}px;\n  border-radius: {radius}px;\n}}\n{MANAGED_END}",
        bg = a.background,
        fg = a.foreground,
        accent = a.accent,
        font = fonts,
        font_size = a.font_size,
        radius = a.radius,
        padding = a.padding,
        border_css = border_css,
        box_shadow_css = box_shadow_css,
    )
}

fn parse_managed_appearance(css: &str) -> Option<Appearance> {
    let (start, end) = managed_block_span(css)?;
    let block = &css[start..end];

    let meta_re = Regex::new(&format!(
        r#"(?s)/\*\s*{}\s+(.*?)\s*\*/"#,
        regex::escape(MANAGED_META_PREFIX)
    ))
    .ok()?;
    if let Some(c) = meta_re.captures(block)
        && let Some(raw) = c.get(1)
        && let Ok(value) = serde_json::from_str::<Value>(raw.as_str())
    {
        let has_border = value.get("border").is_some();
        let has_module_colors = value.get("module_colors").is_some();
        if let Ok(mut a) = serde_json::from_value::<Appearance>(value) {
            if !has_border {
                a.border = infer_border_from_css(css);
            }
            if !has_module_colors {
                // Legacy presets explicitly imposed a uniform color; retain that choice.
                a.module_colors = false;
            }
            return Some(a);
        }
    }

    let bg = selector_property_any(
        block,
        &[".modules-left", ".modules-center", ".modules-right"],
        &["background", "background-color"],
    )?;
    let fg = selector_property(block, &["window#waybar"], &["color"])?;
    let accent = selector_property(
        block,
        &["#workspaces", "button.active"],
        &["background", "background-color"],
    )
    .unwrap_or_else(|| "#89b4fa".to_string());
    let font = selector_property(block, &["window#waybar *"], &["font-family"])
        .map(|f| first_font_family(&f))?;
    let font_size = selector_property(block, &["window#waybar *"], &["font-size"])
        .and_then(|v| first_px_number(&v))?;
    let radius = selector_property_any(
        block,
        &[".modules-left", ".modules-center", ".modules-right"],
        &["border-radius"],
    )
    .and_then(|v| first_px_number(&v))?;
    let padding = selector_property(block, &[".module"], &["padding"])
        .and_then(|v| horizontal_padding_px(&v))?;

    Some(Appearance {
        background: bg,
        foreground: fg,
        module_colors: selector_property(block, &["window#waybar *"], &["color"]).is_none(),
        accent,
        font,
        font_size,
        radius,
        padding,
        border: infer_border_from_css(block),
    })
}

fn managed_block_span(css: &str) -> Option<(usize, usize)> {
    let start = css.find(MANAGED_BEGIN)?;
    let end_marker = css[start..].find(MANAGED_END)? + start;
    let end = end_marker + MANAGED_END.len();
    Some((start, end))
}

fn css_font_family_value(font: &str) -> String {
    let escaped = font.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn strip_css_comments(input: &str) -> Result<String> {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' | b'"' => {
                let end = skip_string(input, i)?;
                out.push_str(&input[i..end]);
                i = end;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 >= bytes.len() {
                    bail!("Comentario CSS sin cerrar");
                }
                i += 2;
            }
            _ => {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    let b = bytes[i];
                    if matches!(b, b'\'' | b'"')
                        || (b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*')
                    {
                        break;
                    }
                    i += 1;
                }
                out.push_str(&input[start..i]);
            }
        }
    }
    Ok(out)
}

fn selector_property(css: &str, selector_needles: &[&str], properties: &[&str]) -> Option<String> {
    let blocks = Regex::new(r"(?s)([^{}]+)\{([^}]*)\}").ok()?;
    for cap in blocks.captures_iter(css) {
        let selectors = cap.get(1)?.as_str();
        if !selector_needles.iter().all(|n| selectors.contains(n)) {
            continue;
        }
        let body = cap.get(2)?.as_str();
        for prop in properties {
            let prop_re =
                Regex::new(&format!(r"(?m){}\s*:\s*([^;]+);", regex::escape(prop))).ok()?;
            if let Some(pc) = prop_re.captures(body) {
                return pc.get(1).map(|m| m.as_str().trim().to_string());
            }
        }
    }
    None
}

fn selector_property_any(css: &str, selectors: &[&str], properties: &[&str]) -> Option<String> {
    for s in selectors {
        if let Some(v) = selector_property(css, &[*s], properties) {
            return Some(v);
        }
    }
    None
}

fn first_px_number(value: &str) -> Option<f64> {
    let re = Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*px").ok()?;
    re.captures(value)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<f64>().ok())
}

fn horizontal_padding_px(value: &str) -> Option<f64> {
    let re = Regex::new(r"([0-9]+(?:\.[0-9]+)?)\s*px").ok()?;
    let nums = re
        .captures_iter(value)
        .filter_map(|c| c.get(1).and_then(|m| m.as_str().parse::<f64>().ok()))
        .collect::<Vec<_>>();
    match nums.as_slice() {
        [] => None,
        [one] => Some(*one),
        [_, horizontal, ..] => Some(*horizontal),
    }
}

fn first_font_family(value: &str) -> String {
    let first = value.split(',').next().unwrap_or(value).trim();
    first
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

fn extract_first_color_token(value: &str) -> Option<String> {
    let re = Regex::new(r"(?i)(#[0-9a-f]{8}|#[0-9a-f]{6}|#[0-9a-f]{3}|rgba?\([^)]*\))").ok()?;
    let token = re
        .captures(value)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())?;
    if validate_color(&token).is_ok() {
        Some(token)
    } else {
        None
    }
}

pub(crate) fn parse_rgba_components(color: &str) -> Option<(u8, u8, u8, f64)> {
    let c = color.trim();
    if let Some(hex) = c.strip_prefix('#') {
        return match hex.len() {
            3 => {
                let mut chars = hex.chars();
                let r = chars.next()?.to_digit(16)? as u8;
                let g = chars.next()?.to_digit(16)? as u8;
                let b = chars.next()?.to_digit(16)? as u8;
                Some((r * 17, g * 17, b * 17, 1.0))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some((r, g, b, 1.0))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()? as f64 / 255.0;
                Some((r, g, b, a))
            }
            _ => None,
        };
    }

    let rgb_re = Regex::new(r"^rgba?\(([^)]+)\)$").ok()?;
    let caps = rgb_re.captures(c)?;
    let parts = caps
        .get(1)?
        .as_str()
        .split(',')
        .map(|s| s.trim())
        .collect::<Vec<_>>();
    let expected_channels = if c.starts_with("rgba(") { 4 } else { 3 };
    if parts.len() != expected_channels {
        return None;
    }

    let r = parts[0].parse::<u8>().ok()?;
    let g = parts[1].parse::<u8>().ok()?;
    let b = parts[2].parse::<u8>().ok()?;
    let a = if parts.len() == 4 {
        parts[3].parse::<f64>().ok()?
    } else {
        1.0
    };
    Some((r, g, b, a))
}

fn with_alpha(color: &str, alpha: f64) -> Option<String> {
    let (r, g, b, color_alpha) = parse_rgba_components(color)?;
    let alpha = color_alpha * alpha;
    Some(format!("rgba({r}, {g}, {b}, {alpha:.3})"))
}

fn box_shadow_alpha_percent(value: &str) -> Option<f64> {
    if value.trim().eq_ignore_ascii_case("none") {
        return None;
    }
    let color = extract_first_color_token(value)?;
    let (_, _, _, alpha) = parse_rgba_components(&color)?;
    Some((alpha * 100.0).clamp(0.0, 100.0))
}

fn infer_border_from_css(css: &str) -> BorderAppearance {
    let mut border = BorderAppearance::default();
    let selectors = [".modules-left", ".modules-center", ".modules-right"];

    if let Some(border_decl) = selector_property_any(css, &selectors, &["border"]) {
        if border_decl.trim().eq_ignore_ascii_case("none") {
            border.width = 0.0;
        } else {
            if let Some(width) = first_px_number(&border_decl) {
                border.width = width;
            }
            if let Some(color) = extract_first_color_token(&border_decl) {
                border.color = color;
            }
        }
    }

    if let Some(width) =
        selector_property_any(css, &selectors, &["border-width"]).and_then(|v| first_px_number(&v))
    {
        border.width = width;
    }

    if let Some(color) = selector_property_any(css, &selectors, &["border-color"])
        .and_then(|v| extract_first_color_token(&v))
    {
        border.color = color;
    }

    if let Some(shadow) = selector_property_any(css, &selectors, &["box-shadow"])
        && !shadow.trim().eq_ignore_ascii_case("none")
    {
        border.neon = true;
        if let Some(glow) = box_shadow_alpha_percent(&shadow) {
            border.glow = glow;
        }
    }

    border.width = border.width.clamp(0.0, 6.0);
    border.glow = border.glow.clamp(0.0, 100.0);
    border
}

pub fn validate_color(color: &str) -> Result<()> {
    let c = color.trim();
    if let Some(hex) = c.strip_prefix('#') {
        if [3, 6, 8].contains(&hex.len()) && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return Ok(());
        }
        bail!("Color hexadecimal inválido: {color}");
    }

    let rgb_re = Regex::new(r"^rgba?\(([^)]+)\)$").unwrap();
    let caps = rgb_re
        .captures(c)
        .ok_or_else(|| anyhow!("Color inválido: {color}"))?;
    let parts = caps
        .get(1)
        .map(|m| m.as_str())
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim())
        .collect::<Vec<_>>();

    let expected_channels = if c.starts_with("rgba(") { 4 } else { 3 };
    if parts.len() != expected_channels {
        bail!("Color RGB(A) inválido: {color}");
    }

    for p in &parts[0..3] {
        let v: i32 = p
            .parse()
            .map_err(|_| anyhow!("Canal RGB inválido en {color}"))?;
        if !(0..=255).contains(&v) {
            bail!("Canal RGB fuera de rango en {color}");
        }
    }

    if parts.len() == 4 {
        let a: f64 = parts[3]
            .parse()
            .map_err(|_| anyhow!("Canal alpha inválido en {color}"))?;
        if !(0.0..=1.0).contains(&a) {
            bail!("Canal alpha fuera de rango en {color}");
        }
    }
    Ok(())
}

fn validate_appearance(a: &Appearance) -> Result<()> {
    for c in [&a.background, &a.foreground, &a.accent, &a.border.color] {
        if c.contains("/*")
            || c.contains("*/")
            || c.contains(';')
            || c.contains('{')
            || c.contains('}')
        {
            bail!("Color inválido o inseguro: {c}");
        }
        validate_color(c)?;
    }

    if a.font.trim().is_empty()
        || a.font.contains(['"', '\\', ';', '{', '}'])
        || a.font.contains("/*")
        || a.font.contains("*/")
        || a.font.chars().any(char::is_control)
    {
        bail!("Fuente inválida o insegura");
    }

    for (name, v) in [
        ("font_size", a.font_size),
        ("radius", a.radius),
        ("padding", a.padding),
        ("border.width", a.border.width),
        ("border.glow", a.border.glow),
    ] {
        let range_ok = match name {
            "border.width" => (0.0..=6.0).contains(&v),
            "border.glow" => (0.0..=100.0).contains(&v),
            _ => (0.0..=200.0).contains(&v),
        };
        if !v.is_finite() || !range_ok {
            bail!("Valor inválido para {name}");
        }
    }
    Ok(())
}

fn validate_css_braces(css: &str) -> Result<()> {
    let mut i = 0usize;
    let bytes = css.as_bytes();
    let mut depth = 0i64;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => {
                i = skip_string(css, i)?;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 >= bytes.len() {
                    bail!("Comentario CSS sin cerrar");
                }
                i += 2;
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth < 0 {
                    bail!("Llave de cierre CSS inesperada");
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    if depth != 0 {
        bail!("Llaves CSS desbalanceadas");
    }
    Ok(())
}

fn ensure_zone(zone: &str) -> Result<()> {
    if ["modules-left", "modules-center", "modules-right"].contains(&zone) {
        Ok(())
    } else {
        bail!("Zona inválida: {zone}");
    }
}

fn looks_like_module_key(key: &str) -> bool {
    if key.starts_with("modules-") {
        return false;
    }
    static NON_MODULE_ROOT_KEYS: &[&str] = &[
        "layer",
        "position",
        "height",
        "width",
        "spacing",
        "margin",
        "margin-top",
        "margin-bottom",
        "margin-left",
        "margin-right",
        "name",
        "mode",
        "output",
        "reload_style_on_change",
        "gtk-layer-shell",
        "ipc",
    ];
    if NON_MODULE_ROOT_KEYS.contains(&key) {
        return false;
    }
    Regex::new(r"^[A-Za-z0-9_.-]+(?:[/#][A-Za-z0-9_.-]+)*$")
        .unwrap()
        .is_match(key)
}

fn ensure_single_bar_root(config: &str) -> Result<()> {
    match trimmed_non_comment_start(config)? {
        Some('[') => bail!(
            "No se admite configuración de múltiples barras (raíz JSON como arreglo). Usa una sola barra (objeto raíz)."
        ),
        Some('{') => Ok(()),
        _ => bail!("La configuración debe iniciar con un objeto JSONC válido."),
    }
}

fn trimmed_non_comment_start(s: &str) -> Result<Option<char>> {
    let mut i = 0;
    skip_ws_and_comments(s, &mut i)?;
    Ok(s[i..].chars().next())
}

fn normalize_jsonc(input: &str) -> Result<String> {
    let without_comments = strip_json_comments(input)?;
    remove_trailing_commas(&without_comments)
}

fn strip_json_comments(input: &str) -> Result<String> {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                let end = skip_string(input, i)?;
                out.push_str(&input[i..end]);
                i = end;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 >= bytes.len() {
                    bail!("Comentario sin cerrar en JSONC");
                }
                i += 2;
            }
            _ => {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
    }
    Ok(out)
}

fn remove_trailing_commas(input: &str) -> Result<String> {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let end = skip_string(input, i)?;
            out.push_str(&input[i..end]);
            i = end;
            continue;
        }
        if bytes[i] == b',' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && (bytes[j] == b']' || bytes[j] == b'}') {
                i += 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Ok(out)
}

#[derive(Clone, Debug)]
struct PropertySpan {
    key: String,
    value_start: usize,
    value_end: usize,
}

fn root_object_span(s: &str) -> Result<(usize, usize)> {
    let mut i = 0usize;
    skip_ws_and_comments(s, &mut i)?;
    if i >= s.len() {
        bail!("Configuración vacía");
    }
    if s.as_bytes()[i] == b'[' {
        bail!(
            "No se admite configuración de múltiples barras (raíz JSON como arreglo). Usa una sola barra (objeto raíz)."
        );
    }
    if s.as_bytes()[i] != b'{' {
        bail!("La raíz de configuración debe ser un objeto JSONC");
    }
    let end = find_matching_brace(s, i)?;
    Ok((i, end + 1))
}

fn object_properties(s: &str, object_span: (usize, usize)) -> Result<Vec<PropertySpan>> {
    let (start, end) = object_span;
    if s.as_bytes()[start] != b'{' || s.as_bytes()[end - 1] != b'}' {
        bail!("Se esperaba un objeto JSON");
    }
    let mut props = Vec::new();
    let mut i = start + 1;
    loop {
        skip_ws_and_comments(s, &mut i)?;
        if i >= end - 1 {
            break;
        }
        if s.as_bytes()[i] == b'}' {
            break;
        }
        if s.as_bytes()[i] != b'"' {
            bail!("Clave JSON inválida en posición {i}");
        }
        let key_end = skip_string(s, i)?;
        let key_raw = &s[i..key_end];
        let key: String = serde_json::from_str(key_raw)
            .map_err(|e| anyhow!("Clave JSON inválida en posición {i}: {e}"))?;
        i = key_end;
        skip_ws_and_comments(s, &mut i)?;
        if i >= end - 1 || s.as_bytes()[i] != b':' {
            bail!("Se esperaba ':' después de la clave '{key}'");
        }
        i += 1;
        skip_ws_and_comments(s, &mut i)?;
        let value_start = i;
        let value_end = parse_value_end(s, i)?;
        props.push(PropertySpan {
            key,
            value_start,
            value_end,
        });
        i = value_end;
        skip_ws_and_comments(s, &mut i)?;
        if i < end - 1 && s.as_bytes()[i] == b',' {
            i += 1;
        }
    }
    Ok(props)
}

fn parse_value_end(s: &str, start: usize) -> Result<usize> {
    let bytes = s.as_bytes();
    if start >= bytes.len() {
        bail!("Valor JSON inesperadamente vacío");
    }
    match bytes[start] {
        b'"' => skip_string(s, start),
        b'{' => Ok(find_matching_brace(s, start)? + 1),
        b'[' => Ok(find_matching_bracket(s, start)? + 1),
        _ => {
            let mut i = start;
            while i < bytes.len() {
                match bytes[i] {
                    b',' | b'}' | b']' | b'/' => break,
                    b if (b as char).is_whitespace() => break,
                    _ => i += 1,
                }
            }
            Ok(i)
        }
    }
}

fn insert_property_into_object(
    source: &mut String,
    object_span: (usize, usize),
    key: &str,
    value_json: &str,
) -> Result<()> {
    let (_, end) = object_span;
    let insert_at = end - 1;
    let props = object_properties(source, object_span)?;
    let trailing_comma = last_significant_char_before(source, insert_at)? == Some(',');
    let mut insertion = String::new();
    if !props.is_empty() && !trailing_comma {
        insertion.push(',');
    }
    insertion.push_str(&format!(
        "\n  {}: {}\n",
        serde_json::to_string(key)?,
        value_json
    ));
    source.insert_str(insert_at, &insertion);
    Ok(())
}

fn last_significant_char_before(s: &str, before: usize) -> Result<Option<char>> {
    let mut i = 0usize;
    let mut last = None;
    while i < before {
        let b = s.as_bytes()[i];
        match b {
            b'"' => {
                i = skip_string(s, i)?;
            }
            b'/' if i + 1 < before && s.as_bytes()[i + 1] == b'/' => {
                i += 2;
                while i < before && s.as_bytes()[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < before && s.as_bytes()[i + 1] == b'*' => {
                i += 2;
                while i + 1 < before && !(s.as_bytes()[i] == b'*' && s.as_bytes()[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < before {
                    i += 2;
                } else {
                    break;
                }
            }
            _ if (b as char).is_whitespace() => i += 1,
            _ => {
                last = Some(b as char);
                i += 1;
            }
        }
    }
    Ok(last)
}

fn skip_ws_and_comments(s: &str, i: &mut usize) -> Result<()> {
    let bytes = s.as_bytes();
    while *i < bytes.len() {
        if (bytes[*i] as char).is_whitespace() {
            *i += 1;
            continue;
        }
        if bytes[*i] == b'/' && *i + 1 < bytes.len() && bytes[*i + 1] == b'/' {
            *i += 2;
            while *i < bytes.len() && bytes[*i] != b'\n' {
                *i += 1;
            }
            continue;
        }
        if bytes[*i] == b'/' && *i + 1 < bytes.len() && bytes[*i + 1] == b'*' {
            *i += 2;
            while *i + 1 < bytes.len() && !(bytes[*i] == b'*' && bytes[*i + 1] == b'/') {
                *i += 1;
            }
            if *i + 1 >= bytes.len() {
                bail!("Comentario sin cerrar");
            }
            *i += 2;
            continue;
        }
        break;
    }
    Ok(())
}

fn skip_string(s: &str, start: usize) -> Result<usize> {
    let bytes = s.as_bytes();
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            return Ok(i + 1);
        }
        i += 1;
    }
    bail!("String sin cerrar en posición {start}")
}

fn find_matching_brace(s: &str, start: usize) -> Result<usize> {
    find_matching_delim(s, start, b'{', b'}')
}

fn find_matching_bracket(s: &str, start: usize) -> Result<usize> {
    find_matching_delim(s, start, b'[', b']')
}

fn find_matching_delim(s: &str, start: usize, open: u8, close: u8) -> Result<usize> {
    let bytes = s.as_bytes();
    if bytes[start] != open {
        bail!("Delimitador inicial inválido en posición {start}");
    }
    let mut i = start;
    let mut depth = 0i64;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => i = skip_string(s, i)?,
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 >= bytes.len() {
                    bail!("Comentario sin cerrar");
                }
                i += 2;
            }
            b if b == open => {
                depth += 1;
                i += 1;
            }
            b if b == close => {
                depth -= 1;
                if depth == 0 {
                    return Ok(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    bail!("Delimitador sin cierre")
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

fn shell_split(input: &str) -> Result<Vec<String>> {
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
                } else {
                    bail!("Comando con escape final incompleto");
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
        bail!("Comando con comillas sin cerrar");
    }
    if saw_token || !cur.is_empty() {
        args.push(cur);
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_modules_ignores_hidden_battery_and_follows_named_instances_in_groups() {
        let mut document = Document {
            config: serde_json::json!({
                "modules-right": ["group/status"],
                "group/status": {"modules": ["clock"]},
                "battery": {},
            })
            .to_string(),
            css: String::new(),
        };
        assert!(!document.visible_modules().unwrap().contains("battery"));
        document
            .set_property(
                Some("group/status"),
                "modules",
                serde_json::json!(["battery#main"]),
            )
            .unwrap();
        assert!(document.visible_modules().unwrap().contains("battery#main"));
    }

    #[test]
    fn hidden_modules_excludes_visible_nested_groups_and_cycles() {
        let document = Document {
            config: serde_json::json!({
                "modules-left": ["group/tools", "battery"],
                "modules-right": ["battery"],
                "group/tools": {"modules": ["cpu", "group/nested"]},
                "group/nested": {"modules": ["memory", "group/tools"]},
                "cpu": {}, "memory": {}, "battery": {},
                "custom/hidden#second": {"exec": "never run"},
                "clock": {}, "ipc": {}, "output": {},
            })
            .to_string(),
            css: String::new(),
        };
        assert_eq!(
            document.hidden_modules().unwrap(),
            ["clock", "custom/hidden#second"]
        );
    }

    #[test]
    fn hide_and_restore_preserve_options_comments_and_css() {
        let mut document = Document {
            config: r#"{
                // Keep this comment and the complete module object.
                "modules-left": ["custom/app#one", "clock"],
                "modules-right": ["custom/app#one"],
                "custom/app#one": {"exec":"script untouched", "unknown":{"ñ":42}},
                "position":"top",
            }"#
            .into(),
            css: "/* CSS untouched */".into(),
        };
        let original = document.value().unwrap()["custom/app#one"].clone();
        document.hide_module("custom/app#one").unwrap();
        assert_eq!(document.hidden_modules().unwrap(), ["custom/app#one"]);
        assert_eq!(document.modules("modules-left").unwrap(), ["clock"]);
        assert!(document.modules("modules-right").unwrap().is_empty());
        document
            .restore_module("custom/app#one", "modules-center")
            .unwrap();
        assert!(document.hidden_modules().unwrap().is_empty());
        assert_eq!(
            document.modules("modules-center").unwrap(),
            ["custom/app#one"]
        );
        assert_eq!(document.value().unwrap()["custom/app#one"], original);
        assert!(document.config.contains("// Keep this comment"));
        assert_eq!(document.css, "/* CSS untouched */");
    }

    #[test]
    fn hidden_defaults_survive_serialization_without_inventing_options() {
        let mut document = Document {
            config: r#"{"modules-left":["clock#secondary"]}"#.into(),
            css: String::new(),
        };
        document.hide_module("clock#secondary").unwrap();
        let mut reloaded: Document =
            serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();
        assert_eq!(reloaded.hidden_modules().unwrap(), ["clock#secondary"]);
        assert_eq!(
            reloaded.value().unwrap()["clock#secondary"],
            serde_json::json!({})
        );
        reloaded
            .restore_module("clock#secondary", "modules-right")
            .unwrap();
        assert_eq!(
            reloaded.modules("modules-right").unwrap(),
            ["clock#secondary"]
        );
    }

    #[test]
    fn restore_rejects_visible_unknown_or_invalid_destination_without_changes() {
        let mut document = Document {
            config: r#"{"modules-left":["clock"],"clock":{},"cpu":{}}"#.into(),
            css: String::new(),
        };
        let original = document.clone();
        assert!(document.restore_module("clock", "modules-right").is_err());
        assert!(
            document
                .restore_module("nonexistent", "modules-right")
                .is_err()
        );
        assert!(document.restore_module("cpu", "invalid-zone").is_err());
        assert!(document.hide_module("nonexistent").is_err());
        assert_eq!(document, original);
    }

    #[test]
    fn color_function_requires_matching_channel_count() {
        assert!(validate_color("rgb(0, 0, 0, 0.5)").is_err());
        assert!(validate_color("rgba(0, 0, 0)").is_err());
        assert!(validate_color("rgba(0, 0, 0, 0.5)").is_ok());
    }

    #[test]
    fn font_cannot_close_metadata_comment() {
        let document = Document {
            config: "{}".into(),
            css: String::new(),
        };
        let mut appearance = document.appearance();
        appearance.font = "unsafe */ font".into();
        assert!(validate_appearance(&appearance).is_err());
    }
    use serde_json::json;

    #[test]
    fn preserves_comments_when_set_modules() {
        let mut d = Document {
            config: r#"{
  // inicio
  "modules-left": ["clock",], // trailing
  "clock": { /* keep */ "format": "x" }
}"#
            .to_string(),
            css: String::new(),
        };
        d.set_modules("modules-left", &["cpu".into(), "clock".into()])
            .unwrap();
        assert!(d.config.contains("// inicio"));
        assert!(d.config.contains("/* keep */"));
        assert!(d.config.contains("\"modules-left\": [\"cpu\",\"clock\"]"));
    }

    #[test]
    fn set_property_in_module_preserves_unknown_keys() {
        let mut d = Document {
            config: r#"{
  "custom/app": {
    // me quedo
    "a": 1,
    "b": 2,
  }
}"#
            .to_string(),
            css: String::new(),
        };
        d.set_property(Some("custom/app"), "on-click", json!("run"))
            .unwrap();
        assert!(d.config.contains("// me quedo"));
        assert!(d.config.contains("\"a\": 1"));
        assert!(d.config.contains("\"on-click\": \"run\""));
    }

    #[test]
    fn reject_multi_bar_array() {
        let d = Document {
            config: "[{\"modules-left\":[]}]".into(),
            css: "".into(),
        };
        let e = d.validate().unwrap_err().to_string();
        assert!(e.contains("múltiples barras"));
    }

    #[test]
    fn appearance_managed_block_replace_preserves_unmanaged_css() {
        let mut d = Document {
            config: "{}".into(),
            css: "#waybar { color: #fff; }\n".into(),
        };
        let a = Appearance {
            background: "#111111".into(),
            foreground: "#eeeeee".into(),
            module_colors: true,
            accent: "#00ff00".into(),
            font: "JetBrains Mono".into(),
            font_size: 13.0,
            radius: 9.0,
            padding: 7.0,
            border: BorderAppearance::default(),
        };
        d.set_appearance(&a).unwrap();
        assert!(d.css.contains("#waybar { color: #fff; }"));
        assert!(d.css.contains(MANAGED_BEGIN));
        d.set_appearance(&a).unwrap();
        assert_eq!(d.css.matches(MANAGED_BEGIN).count(), 1);
    }

    #[test]
    fn parse_jsonc_with_comments_and_trailing() {
        let d = Document {
            config: "{\n//x\n\"a\":1,\n\"b\":[1,2,],\n}".into(),
            css: String::new(),
        };
        let v = d.value().unwrap();
        assert_eq!(v["b"][1], 2);
    }

    #[test]
    fn add_app_adds_center_once_and_only_static_keys() {
        let mut d = Document {
            config: r#"{"modules-center": ["clock"]}"#.into(),
            css: String::new(),
        };
        d.add_app(
            "custom/firefox",
            "Firefox",
            "",
            "env MOZ=1 firefox \"\" %u",
        )
        .unwrap();
        let center = d.modules("modules-center").unwrap();
        assert_eq!(center.iter().filter(|m| *m == "custom/firefox").count(), 1);

        let v = d.value().unwrap();
        let m = &v["custom/firefox"];
        assert_eq!(m["format"], "");
        assert_eq!(m["tooltip-format"], "Firefox");
        assert_eq!(m["tooltip"], true);
        assert!(m.get("exec").is_none());
        assert!(m.get("interval").is_none());
        assert!(m.get("escape").is_none());
        assert!(m["on-click"].as_str().unwrap().contains("''"));
        assert_eq!(
            shell_split(m["on-click"].as_str().unwrap()).unwrap()[0],
            launcher_path().unwrap().to_string_lossy()
        );
    }

    #[test]
    fn appearance_from_real_selectors_ignores_comments() {
        let d = Document {
            config: "{}".into(),
            css: r#"
/* window#waybar .modules-left { background: #ffffff; } */
window#waybar .modules-left,
window#waybar .modules-center,
window#waybar .modules-right {
  background: rgba(26, 27, 38, 0.80);
  border-radius: 20px;
}
#clock { color: #c0caf5; }
#workspaces button.active { background: #7aa2f7; color: #1a1b26; }
* { font-family: "JetBrains Mono", "Noto Sans"; font-size: 13px; }
window#waybar .module { padding: 0 10px; }
"#
            .into(),
        };
        let a = d.appearance();
        assert_eq!(a.background, "rgba(26, 27, 38, 0.80)");
        assert_eq!(a.foreground, "#c0caf5");
        assert_eq!(a.accent, "#7aa2f7");
        assert_eq!(a.font, "JetBrains Mono");
        assert_eq!(a.radius, 20.0);
        assert_eq!(a.padding, 10.0);
    }

    #[test]
    fn detects_current_neon_border_from_css() {
        let d = Document {
            config: "{}".into(),
            css: r#"
window#waybar .modules-left,
window#waybar .modules-center,
window#waybar .modules-right {
  border: 1px solid #65d9ff;
  box-shadow: 0 0 6px rgba(101,217,255,0.60), inset 0 0 4px rgba(101,217,255,0.60);
}
"#
            .into(),
        };
        let a = d.appearance();
        assert_eq!(a.border.color, "#65d9ff");
        assert_eq!(a.border.width, 1.0);
        assert!(a.border.neon);
        assert!((a.border.glow - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn disabling_neon_emits_explicit_none_without_changing_border() {
        let mut d = Document {
            config: "{}".into(),
            css: String::new(),
        };
        let mut a = d.appearance();
        a.border = BorderAppearance {
            color: "#65d9ff".into(),
            width: 1.0,
            neon: true,
            glow: 60.0,
        };
        d.set_appearance(&a).unwrap();
        a.border.neon = false;
        d.set_appearance(&a).unwrap();
        assert!(d.css.contains("border: 1px solid #65d9ff;"));
        assert!(d.css.contains("box-shadow: none;"));
    }

    #[test]
    fn border_zero_disables_shadow() {
        let mut d = Document {
            config: "{}".into(),
            css: String::new(),
        };
        let mut a = d.appearance();
        a.border = BorderAppearance {
            color: "#65d9ff".into(),
            width: 0.0,
            neon: true,
            glow: 100.0,
        };
        d.set_appearance(&a).unwrap();
        assert!(d.css.contains("border: none;"));
        assert!(d.css.contains("box-shadow: none;"));
    }

    #[test]
    fn appearance_roundtrip_via_managed_metadata() {
        let mut d = Document {
            config: "{}".into(),
            css: "window#waybar { color: #fff; }\n".into(),
        };
        let a = Appearance {
            background: "rgba(26, 27, 38, 0.8)".into(),
            foreground: "#c0caf5".into(),
            module_colors: true,
            accent: "#7aa2f7".into(),
            font: "JetBrains Mono".into(),
            font_size: 13.0,
            radius: 20.0,
            padding: 10.0,
            border: BorderAppearance {
                color: "#65d9ff".into(),
                width: 1.0,
                neon: true,
                glow: 60.0,
            },
        };
        d.set_appearance(&a).unwrap();
        assert!(d.css.contains("waybar-editor:appearance"));
        assert_eq!(d.appearance(), a);
    }

    #[test]
    fn legacy_metadata_without_border_infers_from_css() {
        let d = Document {
            config: "{}".into(),
            css: format!(
                "{MANAGED_BEGIN}\n/* {MANAGED_META_PREFIX} {{\"background\":\"#111111\",\"foreground\":\"#eeeeee\",\"accent\":\"#00ff00\",\"font\":\"JetBrains Mono\",\"font_size\":13.0,\"radius\":9.0,\"padding\":7.0}} */\nwindow#waybar .modules-left,\nwindow#waybar .modules-center,\nwindow#waybar .modules-right {{\n  border: 1px solid #65d9ff;\n  box-shadow: 0 0 6px rgba(101,217,255,0.60), inset 0 0 4px rgba(101,217,255,0.60);\n}}\n{MANAGED_END}\n"
            ),
        };
        let a = d.appearance();
        assert_eq!(a.border.width, 1.0);
        assert_eq!(a.border.color, "#65d9ff");
        assert!(a.border.neon);
        assert!((a.border.glow - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn legacy_metadata_defaults_border_when_none_present() {
        let d = Document {
            config: "{}".into(),
            css: format!(
                "{MANAGED_BEGIN}\n/* {MANAGED_META_PREFIX} {{\"background\":\"#111111\",\"foreground\":\"#eeeeee\",\"accent\":\"#00ff00\",\"font\":\"JetBrains Mono\",\"font_size\":13.0,\"radius\":9.0,\"padding\":7.0}} */\n{MANAGED_END}\n"
            ),
        };
        let a = d.appearance();
        assert_eq!(a.border, BorderAppearance::default());
    }

    #[test]
    fn invalid_border_input_rejected() {
        let d = Document {
            config: "{}".into(),
            css: String::new(),
        };
        let mut a = d.appearance();
        a.border.color = "#65d9ff; background:red".into();
        assert!(validate_appearance(&a).is_err());

        let mut b = d.appearance();
        b.border.width = 7.0;
        assert!(validate_appearance(&b).is_err());

        let mut c = d.appearance();
        c.border.glow = 120.0;
        assert!(validate_appearance(&c).is_err());
    }

    #[test]
    fn set_property_primitive_preserves_trailing_comment() {
        let mut d = Document {
            config: "{\n  \"x\": true // keep\n}".into(),
            css: String::new(),
        };
        d.set_property(None, "x", json!(false)).unwrap();
        assert!(d.config.contains("// keep"));
    }

    #[test]
    fn set_property_preserves_block_comment_span() {
        let mut d = Document {
            config: "{\n  \"x\": true /* keep block */,\n  \"y\": 1\n}".into(),
            css: String::new(),
        };
        d.set_property(None, "x", json!(false)).unwrap();
        assert!(d.config.contains("/* keep block */"));
        assert!(d.config.contains("\"y\": 1"));
    }

    #[test]
    fn strip_css_comments_preserves_utf8() {
        let css = "módulo { color: á; } /* коментарий */";
        let stripped = strip_css_comments(css).unwrap();
        assert_eq!(stripped, "módulo { color: á; } ");
    }

    #[test]
    fn module_colors_toggle_preserves_original_states_and_roundtrips() {
        let css = "#battery { color: #9ece6a; }\n#battery.warning:not(.charging) { color: #e0af68; }\n#battery.critical:not(.charging) { color: #f7768e; }\n#custom-power { color: #f7768e; }\n#custom-power:hover { color: #ff9e9e; }\n";
        let mut doc = Document {
            config: "{}".into(),
            css: css.into(),
        };
        let mut a = doc.appearance();
        assert!(a.module_colors);
        doc.set_appearance(&a).unwrap();
        assert!(doc.css.starts_with(css));
        let block = &doc.css[managed_block_span(&doc.css).unwrap().0..];
        assert!(!block.contains("* {\n  color:"));
        assert_eq!(doc.appearance(), a);
        assert_eq!(doc.module_base_color("battery").as_deref(), Some("#9ece6a"));
        assert_eq!(
            doc.module_base_color("custom/power").as_deref(),
            Some("#f7768e")
        );
        a.module_colors = false;
        doc.set_appearance(&a).unwrap();
        assert!(
            doc.css
                .contains("window#waybar#waybar#waybar * {\n  color:")
        );
        assert_eq!(doc.appearance(), a);
        a.module_colors = true;
        doc.set_appearance(&a).unwrap();
        assert!(doc.css.starts_with(css));
        assert!(!doc.css.contains("window#waybar#waybar#waybar"));
        assert_eq!(doc.appearance(), a);
    }

    #[test]
    fn legacy_uniform_presets_keep_their_mode() {
        let mut doc = Document {
            config: "{}".into(),
            css: String::new(),
        };
        let a = doc.appearance();
        doc.set_appearance(&a).unwrap();
        doc.css = doc.css.replace("\"module_colors\":true,", "");
        assert!(!doc.appearance().module_colors);
    }

    #[test]
    fn preview_does_not_guess_live_state_colors() {
        let doc = Document {
            config: "{}".into(),
            css: "#battery.critical { color: #ff0000; } #battery-other { color: #00ff00; }".into(),
        };
        assert!(doc.module_base_color("battery").is_none());
    }

    #[test]
    fn css_urls_are_not_mistaken_for_line_comments() {
        let css = "* { background-image: url(file:///home/user/background.png); } /* note */\n.modules-left { background-color: #123456; }";
        let stripped = strip_css_comments(css).unwrap();
        assert!(stripped.contains("file:///home/user/background.png"));
        validate_css_braces(css).unwrap();
        let doc = Document {
            config: "{}".into(),
            css: css.into(),
        };
        assert_eq!(doc.appearance().background, "#123456");
    }

    #[test]
    fn appearance_retains_font_fallbacks_and_border_alpha() {
        let mut doc = Document {
            config: "{}".into(),
            css:
                "* { font-family: \"JetBrainsMono Nerd Font\", \"Noto Color Emoji\", sans-serif; }"
                    .into(),
        };
        let mut appearance = doc.appearance();
        appearance.font = "Noto Sans".into();
        appearance.border = BorderAppearance {
            color: "rgba(101, 217, 255, 0.5)".into(),
            width: 1.0,
            neon: true,
            glow: 80.0,
        };
        doc.set_appearance(&appearance).unwrap();
        assert!(
            doc.css
                .contains("font-family: \"Noto Sans\", \"Noto Color Emoji\", sans-serif;")
        );
        assert!(doc.css.contains("rgba(101, 217, 255, 0.400)"));
    }

    #[test]
    fn shell_split_rejects_trailing_escape() {
        let err = shell_split("echo hola\\").unwrap_err().to_string();
        assert!(err.contains("escape final"));
    }
}
