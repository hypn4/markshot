//! Annotation colors: sRGB components in 0–1, hex parsing, and the color math the reference app applies.

use crate::error::Error;
use crate::math::pow;

/// Linear sRGB → linear Display P3 (both D65).
const SRGB_TO_P3: [[f64; 3]; 3] = [
    [0.822_462_1, 0.177_538_0, 0.000_000_0],
    [0.033_194_1, 0.966_805_8, 0.000_000_0],
    [0.017_082_7, 0.072_397_4, 0.910_519_9],
];

/// An sRGB color, each component in 0–1.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rgb {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

/// Parses `#RRGGBB` (case-insensitive).
///
/// # Errors
/// [`Error::Spec`] at path `color` when `s` is not exactly `#` followed by six hex digits.
pub fn parse_hex(s: &str) -> Result<Rgb, Error> {
    let invalid = || Error::Spec {
        path: "color".to_owned(),
        message: format!("expected a #RRGGBB hex color, got {s:?}"),
    };
    let digits = s
        .strip_prefix('#')
        .filter(|d| d.len() == 6 && d.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(invalid)?;
    let v = u32::from_str_radix(digits, 16).map_err(|_| invalid())?;
    let channel = |shift: u32| f64::from((v >> shift) & 0xFF) / 255.0;
    Ok(Rgb {
        r: channel(16),
        g: channel(8),
        b: channel(0),
    })
}

impl Rgb {
    /// `#RRGGBB` in upper case; components are clamped to 0–1 and rounded to 8 bits.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!(
            "#{:02X}{:02X}{:02X}",
            to_byte(self.r),
            to_byte(self.g),
            to_byte(self.b)
        )
    }

    /// Rec. 601 luma of the (gamma-encoded) components, as the reference app uses it.
    #[must_use]
    pub fn luminance(self) -> f64 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }

    /// the reference app's full saturation (Highlighter.md §4): min → 0, max → 1,
    /// mid → (mid − min) / max(max − min, 0.01). Ties resolve in r, g, b order.
    #[must_use]
    pub fn saturate(self) -> Rgb {
        let mut c = [self.r, self.g, self.b];
        let mut order = [0, 1, 2];
        order.sort_by(|&i, &j| c[i].total_cmp(&c[j]));
        let [lo, mid, hi] = order;
        let (min, max) = (c[lo], c[hi]);
        c[mid] = (c[mid] - min) / (max - min).max(0.01);
        c[lo] = 0.0;
        c[hi] = 1.0;
        Rgb {
            r: c[0],
            g: c[1],
            b: c[2],
        }
    }

    /// The same color expressed in Display P3 (which shares the sRGB transfer curve),
    /// clamped to 0–1.
    #[must_use]
    pub fn srgb_to_display_p3(self) -> Rgb {
        let linear = [self.r, self.g, self.b].map(srgb_decode);
        let [r, g, b] = SRGB_TO_P3.map(|row| {
            let l = row[0] * linear[0] + row[1] * linear[1] + row[2] * linear[2];
            srgb_encode(l).clamp(0.0, 1.0)
        });
        Rgb { r, g, b }
    }
}

fn srgb_decode(c: f64) -> f64 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        pow((c + 0.055) / 1.055, 2.4)
    }
}

fn srgb_encode(l: f64) -> f64 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * pow(l, 1.0 / 2.4) - 0.055
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is clamped to 0–255 first"
)]
fn to_byte(c: f64) -> u8 {
    (c.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rgb_near(actual: Rgb, expected: [f64; 3], tolerance: f64) {
        let got = [actual.r, actual.g, actual.b];
        for (g, e) in got.iter().zip(expected) {
            assert!((g - e).abs() <= tolerance, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn hex() {
        assert_eq!(parse_hex("#FF0C01").unwrap().to_hex(), "#FF0C01");
        assert!(parse_hex("FF0C01").is_err());
    }

    #[test]
    fn hex_accepts_lowercase_and_rejects_malformed() {
        assert_eq!(parse_hex("#ff0c01").unwrap().to_hex(), "#FF0C01");
        assert_rgb_near(
            parse_hex("#FF0C01").unwrap(),
            [1.0, 12.0 / 255.0, 1.0 / 255.0],
            0.0,
        );
        for bad in ["#FF0C0", "#FF0C011", "#GG0000", "#+FFFFF", "", "#"] {
            assert!(parse_hex(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn to_hex_clamps_out_of_range() {
        let c = Rgb {
            r: 1.2,
            g: -0.1,
            b: 0.5,
        };
        assert_eq!(c.to_hex(), "#FF0080");
    }

    #[test]
    fn saturate_matches_reference() {
        let s = parse_hex("#FFF68D").unwrap().saturate();
        assert_rgb_near(s, [1.0, 0.92105, 0.0], 1e-4);
    }

    #[test]
    fn saturate_floors_the_spread_of_near_greys() {
        let s = Rgb {
            r: 0.5,
            g: 0.505,
            b: 0.502,
        }
        .saturate();
        // max − min = 0.005 < 0.01, so the middle channel is (0.502 − 0.5) / 0.01.
        assert_rgb_near(s, [0.0, 1.0, 0.2], 1e-12);
    }

    #[test]
    fn p3_conversion() {
        let p = Rgb {
            r: 1.0,
            g: 0.0,
            b: 0.0,
        }
        .srgb_to_display_p3();
        assert_rgb_near(p, [0.9175, 0.2003, 0.1386], 1e-3);
    }

    #[test]
    fn p3_conversion_keeps_white_and_black_in_range() {
        let white = Rgb {
            r: 1.0,
            g: 1.0,
            b: 1.0,
        }
        .srgb_to_display_p3();
        assert_rgb_near(white, [1.0, 1.0, 1.0], 1e-6);
        assert!(
            white.r <= 1.0 && white.g <= 1.0 && white.b <= 1.0,
            "{white:?}"
        );
        let black = Rgb {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        };
        assert_eq!(black.srgb_to_display_p3(), black);
    }

    #[test]
    fn luminance_weights() {
        let l = parse_hex("#FF0C01").unwrap().luminance();
        assert!((l - 0.3271).abs() < 1e-3, "{l}");
    }
}
