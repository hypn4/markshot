//! Parser for the absolute `M`/`L`/`C` SVG path strings copied from the reference app (arrow outlines,
//! counter tails).

use crate::error::Error;
use crate::geom::{PathData, Seg, V2};

/// Parses absolute `M`, `L` and `C` commands with comma- or whitespace-separated numbers.
/// As in SVG, extra coordinate pairs after `M` are implicit `L`s and extra triplets after
/// `C` are further cubics.
///
/// # Errors
/// [`Error::Unsupported`] for any other command, a path not starting with `M`, a malformed
/// number, or a command without a whole number of coordinate groups.
pub fn parse_mlc(s: &str) -> Result<PathData, Error> {
    let mut tokens = Tokens { rest: s };
    let mut segs = Vec::new();
    while let Some(cmd) = tokens.command()? {
        if segs.is_empty() && cmd != 'M' {
            return Err(invalid(s, "must start with M"));
        }
        let mut first = true;
        while first || tokens.at_number() {
            let seg = match (cmd, first) {
                ('M', true) => Seg::Move(tokens.point(s)?),
                ('M' | 'L', _) => Seg::Line(tokens.point(s)?),
                _ => Seg::Cubic(tokens.point(s)?, tokens.point(s)?, tokens.point(s)?),
            };
            segs.push(seg);
            first = false;
        }
    }
    Ok(PathData(segs))
}

fn invalid(path: &str, why: &str) -> Error {
    Error::Unsupported(format!("SVG path {path:?}: {why}"))
}

struct Tokens<'a> {
    rest: &'a str,
}

impl Tokens<'_> {
    fn skip_separators(&mut self) {
        self.rest = self
            .rest
            .trim_start_matches(|c: char| c == ',' || c.is_ascii_whitespace());
    }

    fn at_number(&mut self) -> bool {
        self.skip_separators();
        self.rest
            .starts_with(|c: char| c.is_ascii_digit() || matches!(c, '+' | '-' | '.'))
    }

    /// The next command letter, `None` at the end of the input.
    fn command(&mut self) -> Result<Option<char>, Error> {
        self.skip_separators();
        let mut chars = self.rest.chars();
        let Some(c) = chars.next() else {
            return Ok(None);
        };
        if !matches!(c, 'M' | 'L' | 'C') {
            return Err(Error::Unsupported(format!(
                "SVG path command {c:?} (only absolute M, L, C)"
            )));
        }
        self.rest = chars.as_str();
        Ok(Some(c))
    }

    fn point(&mut self, path: &str) -> Result<V2, Error> {
        Ok(V2 {
            x: self.number(path)?,
            y: self.number(path)?,
        })
    }

    /// `[+-]? digits? (. digits?)? ([eE] [+-]? digits)?` with at least one mantissa digit.
    fn number(&mut self, path: &str) -> Result<f64, Error> {
        self.skip_separators();
        let bytes = self.rest.as_bytes();
        let digits_from =
            |i: usize| i + bytes[i..].iter().take_while(|b| b.is_ascii_digit()).count();
        let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
        let int_end = digits_from(end);
        let mut mantissa_digits = int_end - end;
        end = int_end;
        if bytes.get(end) == Some(&b'.') {
            let frac_end = digits_from(end + 1);
            mantissa_digits += frac_end - end - 1;
            end = frac_end;
        }
        if mantissa_digits == 0 {
            return Err(invalid(path, "expected a number"));
        }
        if matches!(bytes.get(end), Some(b'e' | b'E')) {
            let sign = usize::from(matches!(bytes.get(end + 1), Some(b'+' | b'-')));
            let exp_end = digits_from(end + 1 + sign);
            if exp_end == end + 1 + sign {
                return Err(invalid(path, "malformed exponent"));
            }
            end = exp_end;
        }
        let (number, rest) = self.rest.split_at(end);
        self.rest = rest;
        number
            .parse()
            .map_err(|_| invalid(path, "malformed number"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Seg, V2};

    fn v(x: f64, y: f64) -> V2 {
        V2 { x, y }
    }

    #[test]
    fn parse_mlc_segments() {
        let p = parse_mlc("M0,0 L10,0 C10,5 5,10 0,10").unwrap();
        assert_eq!(
            p.0,
            [
                Seg::Move(v(0.0, 0.0)),
                Seg::Line(v(10.0, 0.0)),
                Seg::Cubic(v(10.0, 5.0), v(5.0, 10.0), v(0.0, 10.0)),
            ]
        );
    }

    #[test]
    fn parses_signs_decimals_and_exponents() {
        let p = parse_mlc("M66.6199422,-20.4497473 L2.84217094e-14,-4 L-.5,+1.5E2").unwrap();
        assert_eq!(
            p.0,
            [
                Seg::Move(v(66.619_942_2, -20.449_747_3)),
                Seg::Line(v(2.842_170_94e-14, -4.0)),
                Seg::Line(v(-0.5, 150.0)),
            ]
        );
    }

    #[test]
    fn repeated_coordinates_continue_the_command() {
        // After M, extra pairs are implicit L (SVG rules); extra C triplets are more cubics.
        let p = parse_mlc("M0 0 1 1 C1 2 3 4 5 6 7 8 9 10 11 12").unwrap();
        assert_eq!(
            p.0,
            [
                Seg::Move(v(0.0, 0.0)),
                Seg::Line(v(1.0, 1.0)),
                Seg::Cubic(v(1.0, 2.0), v(3.0, 4.0), v(5.0, 6.0)),
                Seg::Cubic(v(7.0, 8.0), v(9.0, 10.0), v(11.0, 12.0)),
            ]
        );
    }

    #[test]
    fn parses_reference_counter_tail() {
        let s = "M25,50 C25,36.1928813 36.1928813,25 50,25 C63.8071187,25 75,36.1928813 75,50 \
                 C75,69.9607329 55,73.75 52.5,85 C52.0565531,86.9955109 51.0150513,87.5 50,87.5 \
                 C48.9849487,87.5 47.9371276,86.9670742 47.5,85 C45,73.75 25,69.998403 25,50";
        let p = parse_mlc(s).unwrap();
        assert_eq!(p.0.len(), 7);
        assert_eq!(
            p.0[6],
            Seg::Cubic(v(45.0, 73.75), v(25.0, 69.998_403), v(25.0, 50.0))
        );
    }

    #[test]
    fn rejects_malformed_paths() {
        for bad in [
            "L1,1",          // must start with M
            "M1",            // odd coordinate count
            "M1,1 C1,1 2,2", // incomplete cubic
            "M1,1 Q1,1 2,2", // unsupported command
            "M1,1 l2,2",     // relative commands are unsupported
            "M1,x",
            "M1,1e",
            "M1,1 L",
        ] {
            assert!(parse_mlc(bad).is_err(), "{bad:?} should be rejected");
        }
        assert_eq!(parse_mlc("").unwrap().0, []);
    }
}
