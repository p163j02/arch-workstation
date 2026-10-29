use serde_json::Value;
use std::sync::OnceLock;

const FEATURED: &[&str] = &[
    "md-application",
    "md-apps",
    "md-web",
    "md-folder",
    "md-folder_open",
    "md-code_braces",
    "md-console",
    "md-firefox",
    "md-google_chrome",
    "md-chat",
    "md-email",
    "md-music",
    "md-play",
    "md-image",
    "md-video",
    "md-home",
    "md-cog",
    "md-rocket",
    "md-lightning_bolt",
    "md-monitor",
    "md-laptop",
    "md-gamepad",
    "md-steam",
    "md-spotify",
    "fa-discord",
    "fa-telegram",
    "fa-whatsapp",
    "dev-archlinux",
    "linux-archlinux",
    "md-download",
    "cod-terminal",
    "md-microsoft_windows",
    "md-cellphone",
    "md-bookmark",
    "md-camera",
    "md-palette",
];

#[derive(Clone, Debug)]
pub struct Glyph {
    pub name: String,
    pub character: String,
    pub code: String,
}

pub fn catalog() -> &'static [Glyph] {
    static CATALOG: OnceLock<Vec<Glyph>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let source: Value = serde_json::from_str(include_str!("nerd-font-glyphs.json"))
                .expect("bundled Nerd Fonts glyph catalog must be valid JSON");
            let mut glyphs = source
                .as_object()
                .expect("Nerd Fonts catalog must be a JSON object")
                .iter()
                .filter_map(|(name, entry)| {
                    if name == "METADATA" {
                        return None;
                    }
                    let character = entry.get("char")?.as_str()?;
                    let code = entry.get("code")?.as_str()?;
                    Some(Glyph {
                        name: name.clone(),
                        character: character.to_owned(),
                        code: code.to_owned(),
                    })
                })
                .collect::<Vec<_>>();
            glyphs.sort_by(|a, b| a.name.cmp(&b.name));
            glyphs
        })
        .as_slice()
}

/// Returns matching glyphs and the total number of matches before applying the display limit.
/// An empty query in the "all" family starts with a hand-picked set for quick discovery.
pub fn search(query: &str, family_prefix: &str, limit: usize) -> (Vec<&'static Glyph>, usize) {
    let query = query.trim().to_lowercase().replace([' ', '_'], "-");
    let all = catalog();

    if query.is_empty() && family_prefix.is_empty() {
        let featured = FEATURED
            .iter()
            .filter_map(|name| all.iter().find(|glyph| glyph.name == *name))
            .take(limit)
            .collect::<Vec<_>>();
        return (featured, FEATURED.len());
    }

    let matches = all
        .iter()
        .filter(|glyph| {
            (family_prefix.is_empty()
                || family_prefix
                    .split('|')
                    .any(|prefix| glyph.name.starts_with(prefix)))
                && (query.is_empty()
                    || glyph.name.to_lowercase().replace('_', "-").contains(&query))
        })
        .collect::<Vec<_>>();
    let total = matches.len();
    (matches.into_iter().take(limit).collect(), total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundles_the_full_searchable_nerd_fonts_catalog() {
        assert!(catalog().len() > 10_000);
        assert!(catalog().iter().all(|glyph| !glyph.character.is_empty()));
    }

    #[test]
    fn search_matches_names_and_family_prefixes() {
        let (firefox, total) = search("FIREFOX", "", 20);
        assert_eq!(total, 4);
        assert!(firefox.iter().any(|glyph| glyph.name == "md-firefox"));

        let (font_awesome, total) = search("firefox", "fa-", 20);
        assert_eq!(total, 1);
        assert_eq!(font_awesome[0].name, "fa-firefox");

        let (folder, total) = search("folder open", "md-", 20);
        assert!(total > 0);
        assert!(folder.iter().any(|glyph| glyph.name == "md-folder_open"));
    }

    #[test]
    fn empty_search_starts_with_varied_featured_icons() {
        let (featured, total) = search("", "", 100);
        assert_eq!(total, FEATURED.len());
        assert!(featured.len() > 30);
        assert!(featured.iter().any(|glyph| glyph.name == "fa-discord"));
        assert!(featured.iter().any(|glyph| glyph.name == "md-firefox"));
    }
}
