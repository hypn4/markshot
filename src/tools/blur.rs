//! The blur tool: the rect, cut to the image, painted over with a mosaic of mean colours
//! (`pixelate`) or with ramps between its border pixels (`erase`). Both read the original
//! image only, its bytes as they are, never other annotations.
//!
//! Every mosaic draw comes from one [`Rng`] stream seeded by the blur's `seed`: the
//! partition's, then one per block. A unit draw `U` is [`Rng::next_unit`], drawn again while
//! it is 1, and a draw below `n` is `⌊n·U⌋`.

use crate::canvas::Canvas;
use crate::math::pow;
use crate::resolve::ResolvedBlur;
use crate::rng::Rng;
use crate::spec::{BlurMode, Rect};

/// A block of the mosaic: `(x, y, w, h)` in pixels, y up, `(x, y)` its bottom-left corner.
pub type Block = (u32, u32, u32, u32);

/// A y-up rect `(x, y, w, h)` in whole pixels, `(x, y)` its bottom-left corner.
pub type UpRect = (f64, f64, f64, f64);

/// Draws `b` on the main surface: the part of its rect inside the image replaced by the
/// pixelated or erased original. A rect that only touches the image has no such part and
/// draws nothing.
pub fn draw(c: &mut Canvas, b: &ResolvedBlur) {
    let Some(rect) = clip(c, b.rect) else {
        return;
    };
    let rgba = match b.mode {
        BlurMode::Pixelate => pixelate(c, rect, b.size, &mut Rng::new(b.seed)),
        BlurMode::Erase => erase(c, rect),
    };
    let (x0, y0, width, height) = rect;
    let corner = |v: f64| i32::try_from(int(v)).unwrap_or(i32::MAX);
    c.put_rgba(corner(x0), corner(y0), int(width), int(height), &rgba);
}

/// The mosaic's block size in px for a `w` × `h` rect,
/// `round(min(round(min(w, h)/6), 5·dpr)·1.35^size)`.
#[must_use]
pub fn block_size(w: f64, h: f64, size: u8, dpr: f64) -> u32 {
    let base = (w.min(h) / 6.0).round().min(5.0 * dpr);
    int((base * pow(1.35, f64::from(size))).round())
}

/// A random partition of a `w` × `h` rect (y up, from its bottom-left corner) into blocks
/// `max(s, 2)` to `3·max(s, 2)` px a side. Sides up to twice that least are never cut, so a
/// rect thinner than the least gives blocks as thin as it is.
#[must_use]
pub fn partition(w: u32, h: u32, s: u32, rng: &mut Rng) -> Vec<Block> {
    let least = s.max(2);
    let most = 3 * least;
    // Cells of about 6 × least, their edges rounded to whole pixels.
    let count = |len: u32| (f64::from(len) / f64::from(most) * 0.5).ceil();
    let edge = |len: u32, count: f64, i: u32| int((f64::from(len) / count * f64::from(i)).round());
    let (cols, rows) = (count(w), count(h));
    let mut stack = Vec::new();
    for i in 0..int(cols) {
        let (x0, x1) = (edge(w, cols, i), edge(w, cols, i + 1));
        for j in 0..int(rows) {
            let (y0, y1) = (edge(h, rows, j), edge(h, rows, j + 1));
            stack.push((x0, y0, x1 - x0, y1 - y0));
        }
    }
    let mut blocks = Vec::new();
    while let Some((bx, by, bw, bh)) = stack.pop() {
        let first_w = first_side(bw, least, rng);
        let first_h = first_side(bh, least, rng);
        blocks.push((bx, by, first_w, first_h));
        if first_w != bw {
            stack.push((bx + first_w, by, bw - first_w, first_h));
        }
        if first_h != bh {
            stack.push((bx, by + first_h, first_w, bh - first_h));
            if first_w != bw {
                stack.push((bx + first_w, by + first_h, bw - first_w, bh - first_h));
            }
        }
    }
    blocks
}

/// The side of the block cut from the corner of a block `len` px long: all of it up to
/// `2·least`, else `least + round((len − 2·least)·U)`, at most `3·least`.
fn first_side(len: u32, least: u32, rng: &mut Rng) -> u32 {
    if len <= 2 * least {
        return len;
    }
    let extra = int((f64::from(len - 2 * least) * unit(rng)).round());
    (least + extra).min(3 * least)
}

/// A draw in `[0, 1)`: [`Rng::next_unit`], drawn again while it is 1.
fn unit(rng: &mut Rng) -> f64 {
    loop {
        let u = rng.next_unit();
        if u < 1.0 {
            return u;
        }
    }
}

/// A draw below `n`: `⌊n·U⌋`.
fn below(n: u32, rng: &mut Rng) -> u32 {
    int((f64::from(n) * unit(rng)).floor())
}

/// The y-up part of `rect` (y down) inside the image, at least 1 × 1, or `None` when no part
/// is inside, as when the rect only touches the image. Spec rects are whole pixels.
fn clip(c: &Canvas, rect: Rect) -> Option<UpRect> {
    let (left, bottom, width, height) = c.up_rect(rect);
    let (x0, y0) = (left.max(0.0), bottom.max(0.0));
    let x1 = (left + width).min(f64::from(c.width));
    let y1 = (bottom + height).min(f64::from(c.height));
    (x1 > x0 && y1 > y0).then(|| (x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0)))
}

/// `rect` (inside the image) as a mosaic: the [`partition`] blocks, each filled with the mean
/// original colour under it, or, when the rect is small for its blocks (`min(w, h)/7 < s`),
/// under the block past a random side of it. Returns the `w × h` RGBA bitmap, rows top-down.
fn pixelate(c: &Canvas, rect: UpRect, size: u8, rng: &mut Rng) -> Vec<u8> {
    let (x0, y0, width, height) = rect;
    let s = block_size(width, height, size, c.dpr);
    let blocks = partition(int(width), int(height), s, rng);
    let shuffle = width.min(height) / 7.0 < f64::from(s);
    let mut out = Bitmap::new(int(width), int(height));
    for &block in &blocks {
        // A direction is drawn for every block, shuffling or not.
        let direction = below(5, rng);
        let source = if shuffle {
            neighbour(&blocks, block, direction)
        } else {
            block
        };
        out.fill(block, mean(c, x0, y0, source));
    }
    out.rgba
}

/// The block whose colour a shuffled `block` takes: for `direction` 0 to 3, the last of
/// `blocks` strictly containing the point 1 px past the middle of its top, right, bottom or
/// left edge (y up); `block` itself for direction 4 or when none does.
fn neighbour(blocks: &[Block], block: Block, direction: u32) -> Block {
    let [left, bottom, width, height] = [block.0, block.1, block.2, block.3].map(f64::from);
    let (mid_x, mid_y) = (left + width / 2.0, bottom + height / 2.0);
    let (probe_x, probe_y) = match direction {
        0 => (mid_x, bottom + height + 1.0),
        1 => (left + width + 1.0, mid_y),
        2 => (mid_x, bottom - 1.0),
        3 => (left - 1.0, mid_y),
        _ => return block,
    };
    let holds = |&&(bx, by, bw, bh): &&Block| {
        let (bx, by) = (f64::from(bx), f64::from(by));
        bx < probe_x && probe_x < bx + f64::from(bw) && by < probe_y && probe_y < by + f64::from(bh)
    };
    blocks.iter().rev().find(holds).copied().unwrap_or(block)
}

/// The mean original RGB, each channel truncated, of `block` in the rect at `(x0, y0)` (y up).
fn mean(c: &Canvas, x0: f64, y0: f64, (bx, by, bw, bh): Block) -> [u8; 3] {
    let mut sum = [0_u64; 3];
    for row in by..by + bh {
        for col in bx..bx + bw {
            let rgb = c.sample(x0 + f64::from(col), y0 + f64::from(row) + 1.0);
            for (total, v) in sum.iter_mut().zip(rgb) {
                *total += u64::from(v);
            }
        }
    }
    let count = u64::from(bw) * u64::from(bh);
    sum.map(|total| u8::try_from(total / count).expect("a mean of bytes is a byte"))
}

/// The y-up `rect` (whole pixels, at least 1 × 1) erased: each row a 1-px linear ramp between
/// its outermost pixels, or each column one between its bottom and top pixels, whichever the
/// differences along the rect's border pixels favour. Returns the `w × h` RGBA bitmap, rows
/// top-down.
#[must_use]
pub fn erase(c: &Canvas, rect: UpRect) -> Vec<u8> {
    let (x0, y0, width, height) = rect;
    // `sample(x, y)` reads column ⌊x⌋ of y-up row ⌈y⌉ − 1: at `bottom` the rect's bottom row,
    // at `y1` its top row.
    let pixel = |x: f64, y: f64| c.sample(x, y);
    let diff = |a, b| f64::from(dist(a, b));
    let (x1, y1) = (x0 + width, y0 + height);
    let (right, bottom) = (x1 - 1.0, y0 + 1.0);
    // Left vs right border, row by row.
    let s_lr: f64 = (int(y0) + 1..=int(y1))
        .map(|i| {
            let y = f64::from(i);
            diff(pixel(x0, y), pixel(right, y))
        })
        .sum();
    // Steps along the bottom and the top row.
    let s_h: f64 = (int(x0)..int(right))
        .map(|i| {
            let (x, next) = (f64::from(i), f64::from(i + 1));
            diff(pixel(x, bottom), pixel(next, bottom)) + diff(pixel(x, y1), pixel(next, y1))
        })
        .sum();
    // Bottom vs top border, column by column.
    let s_tb: f64 = (int(x0)..int(x1))
        .map(|i| {
            let x = f64::from(i);
            diff(pixel(x, bottom), pixel(x, y1))
        })
        .sum();
    // Intentional: the steps along the left and the right column are added to the
    // bottom-vs-top sum, which is never reset, so that sum counts twice for columns.
    let s_v = s_tb
        + (int(y0) + 1..=int(y1 - 1.0))
            .map(|i| {
                let (y, next) = (f64::from(i), f64::from(i + 1));
                diff(pixel(x0, y), pixel(x0, next)) + diff(pixel(right, y), pixel(right, next))
            })
            .sum::<f64>();
    let cost_rows = s_lr / height + s_h / width * 0.5;
    let cost_cols = s_tb / width + s_v / height * 0.5;
    let (cols, rows) = (int(width), int(height));
    let mut out = Bitmap::new(cols, rows);
    if cost_cols < cost_rows {
        for col in 0..cols {
            let x = x0 + f64::from(col);
            let (from, to) = (pixel(x, bottom), pixel(x, y1));
            for row in 0..rows {
                out.set(col, row, ramp(from, to, row, rows));
            }
        }
    } else {
        for row in 0..rows {
            let y = bottom + f64::from(row);
            let (from, to) = (pixel(x0, y), pixel(right, y));
            for col in 0..cols {
                out.set(col, row, ramp(from, to, col, cols));
            }
        }
    }
    out.rgba
}

/// `|ΔR| + |ΔG| + |ΔB|`.
fn dist(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.iter()
        .zip(b)
        .map(|(&a, b)| u32::from(a.abs_diff(b)))
        .sum()
}

/// Pixel `index` of a 1-px linear gradient `len` px long from `from` to `to`: the colour at
/// its centre, `(index + 0.5)/len` of the way, rounded. There is no dithering.
fn ramp(from: [u8; 3], to: [u8; 3], index: u32, len: u32) -> [u8; 3] {
    let along = (f64::from(index) + 0.5) / f64::from(len);
    std::array::from_fn(|channel| {
        let (start, end) = (f64::from(from[channel]), f64::from(to[channel]));
        byte(start + (end - start) * along)
    })
}

/// A whole, non-negative coordinate or size as an integer.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "whole pixels, 0 or more, the size of an image at most"
)]
fn int(v: f64) -> u32 {
    v as u32
}

/// `v`, between 0 and 255, rounded.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a value between two bytes"
)]
fn byte(v: f64) -> u8 {
    v.round() as u8
}

/// An RGBA bitmap, transparent at first, rows top-down in memory but addressed y up.
struct Bitmap {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Bitmap {
    fn new(width: u32, height: u32) -> Self {
        let (width, height) = (width as usize, height as usize);
        Self {
            width,
            height,
            rgba: vec![0; width * height * 4],
        }
    }

    /// Paints the pixel in column `col` of y-up row `row` opaque `rgb`.
    fn set(&mut self, col: u32, row: u32, [red, green, blue]: [u8; 3]) {
        let at = ((self.height - 1 - row as usize) * self.width + col as usize) * 4;
        self.rgba[at..at + 4].copy_from_slice(&[red, green, blue, 255]);
    }

    fn fill(&mut self, (bx, by, bw, bh): Block, rgb: [u8; 3]) {
        for row in by..by + bh {
            for col in bx..bx + bw {
                self.set(col, row, rgb);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_io::{ColorSpace, SourceImage, decode_png};
    use crate::spec::{BlurMode, Rect};

    const BLACK: [u8; 3] = [0, 0, 0];
    const WHITE: [u8; 3] = [255, 255, 255];
    const MAGENTA: [u8; 3] = [255, 0, 255];

    /// Three ramps of 11 px: each line's two end pixels, the colour between them, and the
    /// ramp erase draws from the first end to the last (`(k + 0.5)/11` of the way, rounded).
    const LINES: [([u8; 3], [u8; 3], [u8; 3]); 3] = [
        (BLACK, BLACK, [110, 0, 220]),
        ([110, 50, 0], MAGENTA, [0, 50, 110]),
        ([220, 0, 110], WHITE, BLACK),
    ];
    const RAMPS: [[[u8; 3]; 11]; 3] = [
        [
            [5, 0, 10],
            [15, 0, 30],
            [25, 0, 50],
            [35, 0, 70],
            [45, 0, 90],
            [55, 0, 110],
            [65, 0, 130],
            [75, 0, 150],
            [85, 0, 170],
            [95, 0, 190],
            [105, 0, 210],
        ],
        [
            [105, 50, 5],
            [95, 50, 15],
            [85, 50, 25],
            [75, 50, 35],
            [65, 50, 45],
            [55, 50, 55],
            [45, 50, 65],
            [35, 50, 75],
            [25, 50, 85],
            [15, 50, 95],
            [5, 50, 105],
        ],
        [
            [210, 0, 105],
            [190, 0, 95],
            [170, 0, 85],
            [150, 0, 75],
            [130, 0, 65],
            [110, 0, 55],
            [90, 0, 45],
            [70, 0, 35],
            [50, 0, 25],
            [30, 0, 15],
            [10, 0, 5],
        ],
    ];

    /// A `width` × `height` canvas at dpr 2 whose pixel `(x, y)`, y up, is `color(x, y)`.
    fn canvas(width: u32, height: u32, color: impl Fn(u32, u32) -> [u8; 3]) -> Canvas {
        let mut rgba = Vec::new();
        for y in (0..height).rev() {
            for x in 0..width {
                rgba.extend(color(x, y));
                rgba.push(255);
            }
        }
        let src = SourceImage {
            width,
            height,
            rgba,
            dpi: None,
            icc: None,
            color_space: ColorSpace::Srgb,
        };
        Canvas::new(src, 2.0)
    }

    /// Busy, irregular colours.
    fn texture(x: u32, y: u32) -> [u8; 3] {
        let byte = |v: u32| u8::try_from(v % 256).unwrap();
        [byte(x * 3 + y * 7), byte(x * y), byte(x * x + y)]
    }

    fn blur(mode: BlurMode, rect: Rect, seed: u64) -> ResolvedBlur {
        ResolvedBlur {
            id: "blur-1".to_owned(),
            size: 2,
            seed,
            rect,
            mode,
        }
    }

    /// The canvas as RGB pixels by row, top-down.
    fn rows(c: Canvas) -> Vec<Vec<[u8; 3]>> {
        let width = c.width as usize;
        decode_png(&c.into_png().unwrap())
            .unwrap()
            .rgba
            .as_chunks::<4>()
            .0
            .chunks(width)
            .map(|row| row.iter().map(|&[r, g, b, _]| [r, g, b]).collect())
            .collect()
    }

    /// An RGBA bitmap as pixels by row, top-down.
    fn bitmap_rows(rgba: &[u8], width: usize) -> Vec<Vec<[u8; 4]>> {
        rgba.as_chunks::<4>()
            .0
            .chunks(width)
            .map(<[_]>::to_vec)
            .collect()
    }

    fn opaque([r, g, b]: [u8; 3]) -> [u8; 4] {
        [r, g, b, 255]
    }

    /// The truncated mean of `texture` over the `w` × `h` block at `(x, y)`, y up.
    fn texture_mean((x, y, w, h): Block) -> [u8; 3] {
        let mut sum = [0; 3];
        for j in y..y + h {
            for i in x..x + w {
                for (s, v) in sum.iter_mut().zip(texture(i, j)) {
                    *s += u32::from(v);
                }
            }
        }
        sum.map(|s| u8::try_from(s / (w * h)).unwrap())
    }

    #[test]
    fn block_sizes() {
        // A large rect at dpr 2 and at dpr 1, and one 24 px tall.
        let sizes = |w, h, dpr| -> Vec<u32> { (0..=7).map(|s| block_size(w, h, s, dpr)).collect() };
        assert_eq!(sizes(300.0, 200.0, 2.0), [10, 14, 18, 25, 33, 45, 61, 82]);
        assert_eq!(sizes(300.0, 200.0, 1.0), [5, 7, 9, 12, 17, 22, 30, 41]);
        assert_eq!(sizes(24.0, 100.0, 2.0), [4, 5, 7, 10, 13, 18, 24, 33]);
    }

    #[test]
    fn partition_covers_exactly_once() {
        // 300 × 200 at s 18: cells of 100 × 100, cut into blocks 18 to 54 px a side.
        for seed in 1..=20 {
            let blocks = partition(300, 200, 18, &mut Rng::new(seed));
            let mut hits = vec![0_u8; 300 * 200];
            for &(x, y, w, h) in &blocks {
                assert!(
                    (18..=54).contains(&w) && (18..=54).contains(&h),
                    "seed {seed}: {w} × {h}"
                );
                assert!(x + w <= 300 && y + h <= 200, "seed {seed}: {x}, {y}");
                for j in y..y + h {
                    for i in x..x + w {
                        hits[(j * 300 + i) as usize] += 1;
                    }
                }
            }
            assert!(hits.iter().all(|&n| n == 1), "seed {seed}");
        }
    }

    #[test]
    fn partition_keeps_small_rects_whole() {
        // Sides up to 2·max(s, 2) are not cut, however small.
        let mut rng = Rng::new(1);
        assert_eq!(partition(30, 10, 18, &mut rng), [(0, 0, 30, 10)]);
        assert_eq!(partition(1, 1, 0, &mut rng), [(0, 0, 1, 1)]);
        assert_eq!(partition(4, 3, 0, &mut rng), [(0, 0, 4, 3)]);
    }

    #[test]
    fn partition_seeded() {
        let run = |seed| partition(300, 200, 18, &mut Rng::new(seed));
        assert_eq!(run(3), run(3));
        assert_ne!(run(3), run(4));
    }

    #[test]
    fn pixelate_blocks_are_mean_of_original() {
        // 300 × 200 at dpr 2, size 2: s = 18, and 200/7 ≥ 18, so no block takes a
        // neighbour's colour. The rect is at (10, 10) either way up on the 320 × 220 image.
        let mut c = canvas(320, 220, texture);
        draw(
            &mut c,
            &blur(BlurMode::Pixelate, [10.0, 10.0, 300.0, 200.0], 9),
        );
        let out = rows(c);
        let blocks = partition(300, 200, 18, &mut Rng::new(9));
        assert!(blocks.len() > 30, "{}", blocks.len());
        for (x, y, w, h) in blocks {
            let (x, y) = (x + 10, y + 10);
            let mean = texture_mean((x, y, w, h));
            for j in y..y + h {
                for i in x..x + w {
                    assert_eq!(out[(219 - j) as usize][i as usize], mean, "({i}, {j})");
                }
            }
        }
    }

    /// Whether blocks `a` and `b` share part of an edge.
    fn adjacent((ax, ay, aw, ah): Block, (bx, by, bw, bh): Block) -> bool {
        let side_by_side = ay < by + bh && by < ay + ah && (ax + aw == bx || bx + bw == ax);
        let stacked = ax < bx + bw && bx < ax + aw && (ay + ah == by || by + bh == ay);
        side_by_side || stacked
    }

    #[test]
    fn shuffled_blocks_take_an_adjacent_blocks_mean() {
        // 100 × 40 at size 2: s = 13 and 40/7 < 13, so each block shows the mean of itself or
        // of a block it shares an edge with.
        let mut borrowed = 0;
        for seed in 1..=5 {
            let mut c = canvas(100, 40, texture);
            let rect = [0.0, 0.0, 100.0, 40.0];
            draw(&mut c, &blur(BlurMode::Pixelate, rect, seed));
            let out = rows(c);
            let blocks = partition(100, 40, 13, &mut Rng::new(seed));
            for &block @ (x, y, w, h) in &blocks {
                let shown = out[(39 - y) as usize][x as usize];
                for j in y..y + h {
                    for i in x..x + w {
                        assert_eq!(out[(39 - j) as usize][i as usize], shown, "seed {seed}");
                    }
                }
                if shown != texture_mean(block) {
                    borrowed += 1;
                    assert!(
                        blocks
                            .iter()
                            .any(|&b| adjacent(block, b) && texture_mean(b) == shown),
                        "seed {seed}: {block:?} shows {shown:?}"
                    );
                }
            }
        }
        assert!(borrowed > 0);
    }

    #[test]
    fn erase_is_linear_ramp() {
        // Each row's ends differ more than its top and bottom rows do, so every row becomes
        // the ramp between its own outermost pixels, whatever lies between them.
        let c = canvas(11, 3, |x, y| {
            let (first, inner, last) = LINES[y as usize];
            match x {
                0 => first,
                10 => last,
                _ => inner,
            }
        });
        let out = bitmap_rows(&erase(&c, (0.0, 0.0, 11.0, 3.0)), 11);
        let want: Vec<Vec<[u8; 4]>> = RAMPS
            .iter()
            .rev()
            .map(|ramp| ramp.map(opaque).to_vec())
            .collect();
        assert_eq!(out, want);
    }

    #[test]
    fn erase_ramps_columns_when_top_and_bottom_match_better() {
        // The rows test turned on its side: each column ramps from its bottom pixel to its top.
        let c = canvas(3, 11, |x, y| {
            let (first, inner, last) = LINES[x as usize];
            match y {
                0 => first,
                10 => last,
                _ => inner,
            }
        });
        let out = bitmap_rows(&erase(&c, (0.0, 0.0, 3.0, 11.0)), 3);
        let want: Vec<Vec<[u8; 4]>> = (0..11)
            .rev()
            .map(|y| RAMPS.iter().map(|ramp| opaque(ramp[y])).collect())
            .collect();
        assert_eq!(out, want);
    }

    #[test]
    fn erase_keeps_the_unreset_accumulator() {
        // Greys, bottom row 0 200 0 200 0, top row 0 200 200 0 0. Rows cost 360; columns
        // 240, or 540 with the bottom-vs-top sum counted again, as `erase` does. So rows win:
        // ramps from black to black.
        let bottom = [0, 200, 0, 200, 0];
        let top = [0, 200, 200, 0, 0];
        let c = canvas(5, 2, |x, y| {
            let v = if y == 0 { bottom } else { top }[x as usize];
            [v; 3]
        });
        let out = erase(&c, (0.0, 0.0, 5.0, 2.0));
        assert_eq!(out, [0, 0, 0, 255].repeat(10));
    }

    #[test]
    fn blur_samples_the_original_under_other_blurs() {
        // An erase over a mosaic ramps between the original's border pixels, not the mosaic's.
        let mosaic = blur(BlurMode::Pixelate, [0.0, 0.0, 60.0, 60.0], 1);
        let eraser = blur(BlurMode::Erase, [30.0, 30.0, 50.0, 50.0], 1);
        let mut both = canvas(100, 100, texture);
        draw(&mut both, &mosaic);
        draw(&mut both, &eraser);
        let mut alone = canvas(100, 100, texture);
        draw(&mut alone, &eraser);
        let (both, alone) = (rows(both), rows(alone));
        for y in 30..80 {
            assert_eq!(both[y][30..80], alone[y][30..80], "row {y}");
        }
        assert_ne!(both[10][..30], alone[10][..30], "the mosaic is drawn");
    }

    #[test]
    fn one_pixel_image_is_left_as_it_is() {
        // A 1 × 1 image is one block of its own colour, and its own ramp.
        for mode in [BlurMode::Pixelate, BlurMode::Erase] {
            let mut c = canvas(1, 1, |_, _| [10, 20, 30]);
            draw(&mut c, &blur(mode, [0.0, 0.0, 1.0, 1.0], 1));
            assert_eq!(rows(c), [[[10, 20, 30]]], "{mode:?}");
        }
    }

    #[test]
    fn skips_rects_that_only_touch_the_image() {
        // Resolve passes such a rect on as partly outside, but none of it is inside the
        // image: neither mode may paint a 1-px strip along the edge it touches (y down).
        let untouched = rows(canvas(40, 30, texture));
        let mut changed = Vec::new();
        for (edge, rect) in [
            ("left", [-10.0, 10.0, 10.0, 10.0]),
            ("top", [10.0, -10.0, 10.0, 10.0]),
            ("right", [40.0, 10.0, 10.0, 10.0]),
            ("bottom", [10.0, 30.0, 10.0, 10.0]),
        ] {
            for mode in [BlurMode::Pixelate, BlurMode::Erase] {
                let mut c = canvas(40, 30, texture);
                draw(&mut c, &blur(mode, rect, 1));
                if rows(c) != untouched {
                    changed.push(format!("{edge} {mode:?}"));
                }
            }
        }
        assert!(changed.is_empty(), "drawn on the image: {changed:?}");
    }
}
