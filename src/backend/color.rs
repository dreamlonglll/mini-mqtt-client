//! Subscription colour marks: `#RRGGBB`, `#RGB`, `#RRGGBBAA`, `rgb(..)` and
//! `rgba(..)` (the previous version's colour picker could store either form).

/// The swatches offered by the subscription dialog (same as before).
pub const PRESET_COLORS: [&str; 8] = [
    "#F56C6C", "#E6A23C", "#F2D849", "#67C23A", "#409EFF", "#9B59B6", "#FF69B4", "#00CED1",
];

/// `[r, g, b, a]` in `0.0..=1.0`.
pub fn parse_color(value: &str) -> Option<[f32; 4]> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let digits: Vec<u8> = hex
            .chars()
            .map(|c| c.to_digit(16).map(|d| d as u8))
            .collect::<Option<_>>()?;
        let byte = |hi: u8, lo: u8| (hi * 16 + lo) as f32 / 255.0;
        return match digits.as_slice() {
            [r, g, b] => Some([byte(*r, *r), byte(*g, *g), byte(*b, *b), 1.0]),
            [r1, r2, g1, g2, b1, b2] => Some([byte(*r1, *r2), byte(*g1, *g2), byte(*b1, *b2), 1.0]),
            [r1, r2, g1, g2, b1, b2, a1, a2] => Some([byte(*r1, *r2), byte(*g1, *g2), byte(*b1, *b2), byte(*a1, *a2)]),
            _ => None,
        };
    }
    let lower = value.to_ascii_lowercase();
    let inner = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let parts: Vec<f32> = inner
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok())
        .collect::<Option<_>>()?;
    let channel = |v: f32| (v / 255.0).clamp(0.0, 1.0);
    match parts.as_slice() {
        [r, g, b] => Some([channel(*r), channel(*g), channel(*b), 1.0]),
        [r, g, b, a] => Some([channel(*r), channel(*g), channel(*b), a.clamp(0.0, 1.0)]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_rgb_colors() {
        assert_eq!(parse_color("#409EFF"), Some([64.0 / 255.0, 158.0 / 255.0, 1.0, 1.0]));
        assert_eq!(parse_color("#fff"), Some([1.0, 1.0, 1.0, 1.0]));
        assert_eq!(parse_color("#00000080").map(|c| (c[3] * 255.0).round()), Some(128.0));
        assert_eq!(
            parse_color("rgba(64, 158, 255, 0.5)"),
            Some([64.0 / 255.0, 158.0 / 255.0, 1.0, 0.5])
        );
        assert_eq!(parse_color("rgb(255,0,0)"), Some([1.0, 0.0, 0.0, 1.0]));
        for bad in ["", "red", "#12", "#GGGGGG", "rgb(1,2)", "rgba(1,2,3,x)"] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
        assert!(PRESET_COLORS.iter().all(|c| parse_color(c).is_some()));
    }
}
