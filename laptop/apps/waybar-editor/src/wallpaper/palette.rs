use super::theme::Palette;

#[derive(Clone, Copy, Debug, Default)]
struct Rgb([f64; 3]);

impl Rgb {
    fn from_bytes(rgb: [u8; 3]) -> Self {
        Self(rgb.map(|v| f64::from(v) / 255.0))
    }

    fn hex(self) -> String {
        let [r, g, b] = self.0.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    fn hue_saturation_value(self) -> (f64, f64, f64) {
        let [r, g, b] = self.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let chroma = max - min;
        let hue = if chroma < 1e-9 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / chroma).rem_euclid(6.0)
        } else if max == g {
            60.0 * ((b - r) / chroma + 2.0)
        } else {
            60.0 * ((r - g) / chroma + 4.0)
        };
        (hue, if max < 1e-9 { 0.0 } else { chroma / max }, max)
    }

    fn hsl(self) -> Hsl {
        let (hue, _, max) = self.hue_saturation_value();
        let min = self.0.into_iter().fold(1.0_f64, f64::min);
        let lightness = (max + min) / 2.0;
        let denominator = 1.0 - (2.0 * lightness - 1.0).abs();
        Hsl {
            hue,
            saturation: if denominator < 1e-9 {
                0.0
            } else {
                (max - min) / denominator
            },
            lightness,
        }
    }

    fn luminance(self) -> f64 {
        let [r, g, b] = self.0.map(|v| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
        r * 0.2126 + g * 0.7152 + b * 0.0722
    }
}

#[derive(Clone, Copy)]
struct Hsl {
    hue: f64,
    saturation: f64,
    lightness: f64,
}

impl Hsl {
    fn rgb(self) -> Rgb {
        let chroma = (1.0 - (2.0 * self.lightness - 1.0).abs()) * self.saturation;
        let h = self.hue.rem_euclid(360.0) / 60.0;
        let x = chroma * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
        let rgb = match h as usize {
            0 => [chroma, x, 0.0],
            1 => [x, chroma, 0.0],
            2 => [0.0, chroma, x],
            3 => [0.0, x, chroma],
            4 => [x, 0.0, chroma],
            _ => [chroma, 0.0, x],
        };
        let offset = self.lightness - chroma / 2.0;
        Rgb(rgb.map(|v| v + offset))
    }

    // Raise lightness while retaining the source hue and saturation. Mixing
    // navy with white directly was turning most dark wallpapers into gray-blue.
    fn readable(mut self, background: Rgb) -> Rgb {
        for _ in 0..200 {
            let rgb = self.rgb();
            // Leave a small margin for rounding the final 8-bit channels.
            if contrast(rgb, background) >= 4.6 || self.lightness >= 0.98 {
                return rgb;
            }
            self.lightness = (self.lightness + 0.005).min(0.98);
        }
        self.rgb()
    }
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let a = a.luminance();
    let b = b.luminance();
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn hue_distance(a: f64, b: f64) -> f64 {
    let difference = (a - b).abs().rem_euclid(360.0);
    difference.min(360.0 - difference)
}

#[derive(Clone, Copy, Default)]
struct Bucket {
    mass: f64,
    count: usize,
    rgb: [f64; 3],
}

#[derive(Clone, Copy)]
struct Candidate {
    score: f64,
    color: Rgb,
    hue: f64,
}

pub(super) fn extract(samples: &[[u8; 3]]) -> Palette {
    let mut histogram = [Bucket::default(); 36];
    let mut sums = [0.0; 3];
    for &sample in samples {
        let rgb = Rgb::from_bytes(sample);
        for (sum, value) in sums.iter_mut().zip(rgb.0) {
            *sum += value;
        }
        let (hue, saturation, value) = rgb.hue_saturation_value();
        // Near-black pixels and neutral surfaces must not drown out the image's
        // colorful details. Bright, saturated areas contribute more to each hue.
        if saturation < 0.20 || value < 0.14 {
            continue;
        }
        let weight = saturation.powi(2) * value.powi(2);
        let bucket = &mut histogram[(hue / 10.0) as usize % 36];
        bucket.mass += weight;
        bucket.count += 1;
        for (sum, channel) in bucket.rgb.iter_mut().zip(rgb.0) {
            *sum += channel * weight;
        }
    }
    let mut candidates = Vec::new();
    for i in 0..36 {
        // Merge adjacent hue bins, including red at the 0°/360° boundary.
        let nearby = [
            histogram[(i + 35) % 36],
            histogram[i],
            histogram[(i + 1) % 36],
        ];
        let mass: f64 = nearby.iter().map(|b| b.mass).sum();
        let count: usize = nearby.iter().map(|b| b.count).sum();
        if mass <= 0.0 || count < ((samples.len() as f64 * 0.002).ceil() as usize).max(2) {
            continue;
        }
        let color = Rgb(std::array::from_fn(|channel| {
            nearby.iter().map(|b| b.rgb[channel]).sum::<f64>() / mass
        }));
        let (hue, saturation, value) = color.hue_saturation_value();
        // Compress population differences so a small neon/pink/orange detail
        // can compete with a large patch of dark sky of the same image.
        let score = mass.powf(0.35) * saturation.powf(0.8) * value.powf(0.4);
        candidates.push(Candidate { score, color, hue });
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.hue.total_cmp(&b.hue)));
    let mean = Rgb(sums.map(|sum| sum / samples.len().max(1) as f64));
    let primary = candidates
        .first()
        .map(|candidate| candidate.color)
        .unwrap_or(mean);
    let base = mean.hsl();
    let colorful = !candidates.is_empty();
    let background = Hsl {
        hue: if base.saturation < 0.08 {
            primary.hsl().hue
        } else {
            base.hue
        },
        saturation: if colorful {
            base.saturation.clamp(0.20, 0.48)
        } else {
            base.saturation
        },
        lightness: (0.055 + base.lightness * 0.12).clamp(0.065, 0.12),
    }
    .rgb();
    let accent = primary.hsl().readable(background);
    let secondary = candidates
        .iter()
        .filter(|candidate| hue_distance(candidate.hue, primary.hsl().hue) >= 45.0)
        .max_by(|a, b| {
            let score = |c: &Candidate| {
                c.score * (0.5 + hue_distance(c.hue, primary.hsl().hue).min(120.0) / 120.0)
            };
            score(a).total_cmp(&score(b))
        })
        .map(|candidate| candidate.color.hsl())
        .unwrap_or_else(|| {
            // A monochrome source gets a second shade of its own color,
            // rather than a fixed purple unrelated to the wallpaper.
            let mut shade = accent.hsl();
            shade.lightness = if shade.lightness < 0.74 {
                (shade.lightness + 0.16).min(0.90)
            } else {
                (shade.lightness - 0.16).max(0.45)
            };
            shade
        })
        .readable(background);
    let foreground = Hsl {
        hue: base.hue,
        saturation: if colorful { 0.30 } else { 0.0 },
        lightness: 0.94,
    }
    .rgb();
    Palette {
        accent: accent.hex(),
        secondary: secondary.hex(),
        background: background.hex(),
        foreground: foreground.hex(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(hex: &str) -> Rgb {
        Rgb::from_bytes(std::array::from_fn(|i| {
            u8::from_str_radix(&hex[1 + i * 2..3 + i * 2], 16).unwrap()
        }))
    }

    fn scene(areas: &[(usize, [u8; 3])]) -> Vec<[u8; 3]> {
        areas
            .iter()
            .flat_map(|&(count, rgb)| std::iter::repeat_n(rgb, count))
            .collect()
    }

    #[test]
    fn dark_sky_keeps_blue_and_smaller_pink_highlights() {
        let samples = scene(&[
            (8000, [3, 8, 24]),
            (1500, [15, 60, 145]),
            (500, [235, 65, 145]),
        ]);
        let palette = extract(&samples);
        let accent = rgb(&palette.accent);
        let secondary = rgb(&palette.secondary);
        assert!(hue_distance(accent.hsl().hue, 219.0) < 12.0, "{palette:?}");
        assert!(
            hue_distance(secondary.hsl().hue, 332.0) < 12.0,
            "{palette:?}"
        );
        assert!(accent.hsl().saturation > 0.70, "{palette:?}");
        assert!(secondary.hsl().saturation > 0.65, "{palette:?}");
    }

    #[test]
    fn galaxy_selects_purple_and_orange_instead_of_neutral_background() {
        let samples = scene(&[
            (8500, [18, 17, 22]),
            (1000, [130, 25, 165]),
            (500, [240, 125, 30]),
        ]);
        let palette = extract(&samples);
        let hues = [
            rgb(&palette.accent).hsl().hue,
            rgb(&palette.secondary).hsl().hue,
        ];
        assert!(
            hues.iter().any(|h| hue_distance(*h, 285.0) < 12.0),
            "{palette:?}"
        );
        assert!(
            hues.iter().any(|h| hue_distance(*h, 27.0) < 12.0),
            "{palette:?}"
        );
    }

    #[test]
    fn isolated_color_noise_does_not_replace_a_real_secondary() {
        let samples = scene(&[
            (9000, [3, 8, 24]),
            (800, [15, 60, 145]),
            (199, [235, 65, 145]),
            (1, [0, 255, 0]),
        ]);
        let palette = extract(&samples);
        for color in [&palette.accent, &palette.secondary] {
            assert!(hue_distance(rgb(color).hsl().hue, 120.0) > 20.0);
        }
    }

    #[test]
    fn monochrome_images_do_not_invent_an_unrelated_hue() {
        for color in [[8, 40, 95], [15, 130, 65], [165, 35, 15], [70, 70, 70]] {
            let palette = extract(&vec![color; 200]);
            for extracted in [&palette.accent, &palette.secondary] {
                let got = rgb(extracted).hsl();
                let source = Rgb::from_bytes(color).hsl();
                if source.saturation > 0.1 {
                    assert!(hue_distance(got.hue, source.hue) < 3.0, "{palette:?}");
                } else {
                    assert!(got.saturation < 0.02, "{palette:?}");
                }
            }
            assert_ne!(palette.accent, palette.secondary);
        }
    }

    #[test]
    fn readable_text_accents_and_tinted_surfaces_are_deterministic() {
        for samples in [
            scene(&[(100, [0, 0, 255])]),
            scene(&[(100, [255, 0, 0])]),
            scene(&[(100, [0, 255, 0])]),
            scene(&[(100, [255, 255, 255])]),
            scene(&[(100, [0, 0, 0])]),
            Vec::new(),
        ] {
            let palette = extract(&samples);
            assert_eq!(palette, extract(&samples));
            let background = rgb(&palette.background);
            for color in [&palette.accent, &palette.secondary] {
                assert!(contrast(rgb(color), background) >= 4.5, "{palette:?}");
            }
            assert!(
                contrast(rgb(&palette.foreground), background) >= 7.0,
                "{palette:?}"
            );
        }
    }

    #[test]
    fn distinct_sources_produce_distinct_accents_backgrounds_and_text_tints() {
        let blue = extract(&vec![[10, 40, 130]; 200]);
        let orange = extract(&vec![[220, 95, 20]; 200]);
        assert_ne!(blue.accent, orange.accent);
        assert_ne!(blue.secondary, orange.secondary);
        assert_ne!(blue.background, orange.background);
        assert_ne!(blue.foreground, orange.foreground);
    }
}
