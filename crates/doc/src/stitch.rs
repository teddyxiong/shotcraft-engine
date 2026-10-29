//! Joins successive frames of a scrolling region into one tall image.
//!
//! Successive frames are matched using textured patches and broad pixel verification. Rows
//! that stay put at the top and bottom are excluded from matching; the header is kept once
//! from the first frame and the footer once from the last.

use image::RgbaImage;

const MIN_SCORE: f32 = 0.90;
const MIN_COMPARED_ROWS: usize = 6;

#[derive(Debug, PartialEq, Eq)]
pub enum Push {
    /// `n` new rows were appended.
    Appended(usize),
    /// The frame matches the previous one (not scrolled yet, or the end was reached).
    Unchanged,
    /// No reliable offset; the frame was ignored.
    NoMatch,
}

/// Evidence from the latest push, including the best candidate below the acceptance threshold.
#[derive(Debug, Default)]
pub struct MatchDiagnostic {
    pub reason: &'static str,
    pub header: usize,
    pub footer: usize,
    pub best_dy: Option<usize>,
    pub compared: usize,
    pub matched: usize,
    /// One minus the normalized pixel difference on textured rows (not an exact row-match ratio).
    pub score: f32,
}

pub struct Stitcher {
    width: u32,
    height: u32,
    /// Output rows so far (RGBA), excluding the sticky footer.
    out: Vec<u8>,
    last: Option<RgbaImage>,
    footer: Option<usize>,
    max_height: u32,
    diagnostic: MatchDiagnostic,
}

impl Stitcher {
    pub fn new(max_height: u32) -> Self {
        Self { width: 0, height: 0, out: Vec::new(), last: None, footer: None, max_height, diagnostic: MatchDiagnostic::default() }
    }

    pub fn diagnostic(&self) -> &MatchDiagnostic {
        &self.diagnostic
    }

    pub fn height(&self) -> u32 {
        let footer = self.footer.unwrap_or(0) as u32;
        (self.out.len() / (self.width.max(1) as usize * 4)) as u32 + footer
    }

    pub fn is_full(&self) -> bool {
        self.height() >= self.max_height
    }

    pub fn push(&mut self, img: RgbaImage) -> Push {
        self.diagnostic = MatchDiagnostic::default();
        // A display that shrank mid-capture can crop to nothing; there are no rows to match.
        if img.width() == 0 || img.height() == 0 {
            self.diagnostic.reason = "empty frame";
            return Push::NoMatch;
        }
        let Some(prev) = &self.last else {
            self.diagnostic.reason = "initial frame";
            self.width = img.width();
            self.height = img.height();
            self.out = img.as_raw().clone();
            self.last = Some(img);
            return Push::Appended(self.height as usize);
        };
        if img.dimensions() != (self.width, self.height) {
            self.diagnostic.reason = "frame dimensions changed";
            return Push::NoMatch;
        }
        if img == *prev {
            self.diagnostic.reason = "identical frame";
            return Push::Unchanged;
        }
        let h = self.height as usize;
        let row = self.width as usize * 4;
        let columns = sample_rows(self.width as usize / 20, self.width as usize - self.width as usize / 20 - 1, 64);
        let active = active_columns(prev, &img, &columns);
        let header = stable_edge(prev, &img, &active, true);
        let initial_footer = stable_edge(prev, &img, &active, false);
        let known_footer = self.footer;
        let footer = known_footer.unwrap_or(initial_footer);
        self.diagnostic.header = header;
        self.diagnostic.footer = footer;
        let Some((dy, detected_footer, refresh_from)) = find_offset(prev, &img, header, known_footer, &active, &mut self.diagnostic) else {
            return Push::NoMatch;
        };
        if self.footer.is_none() {
            // First match: the first frame was appended whole, but its footer belongs at the very end.
            self.footer = Some(detected_footer);
            self.out.truncate((h - detected_footer) * row);
        }
        let room = (self.max_height as usize).saturating_sub(self.height() as usize);
        let take = dy.min(room);
        let start = h - self.footer.unwrap_or(footer) - dy;
        // Refresh the overlap so transient status bars and frame edges do not remain stitched in.
        if refresh_from < start {
            let source_start = refresh_from * row;
            let source_end = start * row;
            if let Some(source) = img.as_raw().get(source_start..source_end)
                && let Some(target_start) = self.out.len().checked_sub(source.len())
                && let Some(target) = self.out.get_mut(target_start..)
            {
                target.copy_from_slice(source);
            }
        }
        self.out.extend_from_slice(&img.as_raw()[start * row..(start + take) * row]);
        self.last = Some(img);
        Push::Appended(take)
    }

    /// The most recent `rows` stitched rows, for a live preview.
    pub fn tail(&self, rows: u32) -> Option<RgbaImage> {
        let row = self.width as usize * 4;
        let total = self.out.len() / row.max(1);
        let n = (rows as usize).min(total);
        if n == 0 {
            return None;
        }
        RgbaImage::from_raw(self.width, n as u32, self.out[(total - n) * row..].to_vec())
    }

    /// The stitched image, with the latest footer re-attached.
    pub fn finish(self) -> Option<RgbaImage> {
        let last = self.last?;
        let mut out = self.out;
        let row = self.width as usize * 4;
        let h = self.height as usize;
        if let Some(footer) = self.footer {
            out.extend_from_slice(&last.as_raw()[(h - footer) * row..h * row]);
        }
        let rows = (out.len() / row) as u32;
        RgbaImage::from_raw(self.width, rows, out)
    }
}

fn rgb_diff(a: &image::Rgba<u8>, b: &image::Rgba<u8>) -> f32 {
    ((a[0].abs_diff(b[0]) as u16 + a[1].abs_diff(b[1]) as u16 + a[2].abs_diff(b[2]) as u16) as f32) / 765.0
}

fn diff_at(a: &RgbaImage, ax: usize, ay: usize, b: &RgbaImage, bx: usize, by: usize) -> f32 {
    match (a.get_pixel_checked(ax as u32, ay as u32), b.get_pixel_checked(bx as u32, by as u32)) {
        (Some(a), Some(b)) => rgb_diff(a, b),
        _ => 1.0,
    }
}

fn sample_rows(start: usize, end: usize, count: usize) -> Vec<usize> {
    if start > end || count == 0 {
        return Vec::new();
    }
    let len = end - start;
    let n = count.min(len + 1);
    (0..n).map(|i| start + i * len / n.saturating_sub(1).max(1)).collect()
}

fn active_columns(prev: &RgbaImage, cur: &RgbaImage, columns: &[usize]) -> Vec<usize> {
    let h = prev.height() as usize;
    let rows = sample_rows(h / 10, h * 9 / 10, 24);
    let mut active: Vec<_> = columns
        .iter()
        .copied()
        .filter(|&x| {
            let mut diffs: Vec<_> = rows.iter().map(|&y| diff_at(prev, x, y, cur, x, y)).collect();
            diffs.sort_by(f32::total_cmp);
            let skip = diffs.len().saturating_sub(diffs.len().div_ceil(3));
            let mean = diffs.get(skip..).unwrap_or(&[]).iter().sum::<f32>() / diffs.len().saturating_sub(skip).max(1) as f32;
            mean > 0.018
        })
        .collect();
    if active.len() < 3 {
        active = columns.to_vec();
    }
    active
}

fn stable_edge(prev: &RgbaImage, cur: &RgbaImage, columns: &[usize], top: bool) -> usize {
    let h = prev.height() as usize;
    let limit = h / 3;
    let mut stable = 0;
    let mut unstable = 0;
    for i in 0..limit {
        let y = if top { i } else { h - 1 - i };
        let mut diffs: Vec<_> = columns.iter().map(|&x| diff_at(prev, x, y, cur, x, y)).collect();
        diffs.sort_by(f32::total_cmp);
        let keep = (diffs.len() * 3).div_ceil(4);
        let mean = diffs.get(..keep.max(1)).unwrap_or(&[]).iter().sum::<f32>() / keep.max(1) as f32;
        if mean <= 0.025 {
            stable = i + 1;
            unstable = 0;
        } else {
            unstable += 1;
            if unstable >= 4 {
                break;
            }
        }
    }
    stable
}

fn patch_texture(img: &RgbaImage, x: usize, y: usize, w: usize) -> f32 {
    let mut sum = 0.0;
    let mut n = 0;
    for py in (y..y + 6).step_by(2) {
        for px in (x..x + w).step_by(2) {
            if px + 1 < x + w {
                sum += diff_at(img, px, py, img, px + 1, py);
                n += 1;
            }
            if py + 1 < y + 6 {
                sum += diff_at(img, px, py, img, px, py + 1);
                n += 1;
            }
        }
    }
    sum / n.max(1) as f32
}

fn find_offset(
    prev: &RgbaImage,
    cur: &RgbaImage,
    header: usize,
    known_footer: Option<usize>,
    columns: &[usize],
    diagnostic: &mut MatchDiagnostic,
) -> Option<(usize, usize, usize)> {
    let h = prev.height() as usize;
    let w = prev.width() as usize;
    let initial_footer = stable_edge(prev, cur, columns, false);
    let footer = known_footer.unwrap_or(initial_footer);
    let patch_w = 24.min(w);
    if h < 6 || patch_w == 0 {
        diagnostic.reason = "frame too small for features";
        return None;
    }
    let x_end = w.saturating_sub(w / 20).saturating_sub(patch_w);
    let xs = sample_rows((w / 20).min(x_end), x_end, 16);
    let ys = sample_rows(header, h.saturating_sub(footer + 6), 20);
    let mut votes: Vec<(usize, usize)> = Vec::new();
    for y in ys {
        let mut choices = Vec::new();
        for &x in &xs {
            let texture = patch_texture(prev, x, y, patch_w);
            let activity = (0..6)
                .step_by(2)
                .flat_map(|py| (0..patch_w).step_by(2).map(move |px| (px, py)))
                .map(|(px, py)| diff_at(prev, x + px, y + py, cur, x + px, y + py))
                .sum::<f32>()
                / ((patch_w / 2).max(1) * 3) as f32;
            if texture > 0.025 && activity > 0.025 {
                choices.push((texture * activity, x));
            }
        }
        choices.sort_by(|a, b| b.0.total_cmp(&a.0));
        for &(_, x) in choices.iter().take(2) {
            let max_dy = y.saturating_sub(header);
            let mut scores = Vec::new();
            for dy in 1..=max_dy {
                let mut sum = 0.0;
                let mut n = 0;
                for py in (0..6).step_by(2) {
                    for px in (0..patch_w).step_by(2) {
                        sum += diff_at(prev, x + px, y + py, cur, x + px, y - dy + py);
                        n += 1;
                    }
                }
                scores.push((dy, sum / n.max(1) as f32));
            }
            scores.sort_by(|a, b| a.1.total_cmp(&b.1));
            if let Some(&(dy, score)) = scores.first() {
                let second = scores.iter().find(|(other, _)| other.abs_diff(dy) > 2).map_or(f32::INFINITY, |(_, score)| *score);
                if score < 0.065 && second - score >= 0.008 {
                    votes.push((dy, y));
                }
            }
        }
    }
    let mut candidates = Vec::new();
    for (dy, _) in votes.iter().copied() {
        if !candidates.contains(&dy) {
            candidates.push(dy);
        }
    }
    if candidates.is_empty() {
        diagnostic.reason = "no reliable feature displacement";
        return None;
    }
    let mut best: Option<(usize, usize, f32, usize, usize)> = None;
    for dy in candidates {
        let vote_count = votes.iter().filter(|(v, _)| v.abs_diff(dy) <= 2).count();
        if vote_count < 2 && h.saturating_sub(dy + footer) >= 12 {
            continue;
        }
        let detected_footer = known_footer.unwrap_or_else(|| infer_footer(prev, cur, dy, initial_footer, columns));
        if h.saturating_sub(header + detected_footer + dy) < 6 {
            continue;
        }
        let end = h.saturating_sub(detected_footer + dy);
        let ys = sample_rows(header, end.saturating_sub(1), 64);
        let mut row_diffs = Vec::new();
        for y in ys {
            let mut diffs = Vec::new();
            for &x in columns {
                if textured(prev, x, y + dy) || textured(cur, x, y) {
                    diffs.push(diff_at(prev, x, y + dy, cur, x, y));
                }
            }
            if !diffs.is_empty() {
                row_diffs.push(diffs.iter().sum::<f32>() / diffs.len() as f32);
            }
        }
        if row_diffs.len() < MIN_COMPARED_ROWS {
            continue;
        }
        row_diffs.sort_by(f32::total_cmp);
        let keep = (row_diffs.len() * 92).div_ceil(100).max(MIN_COMPARED_ROWS);
        let error = row_diffs.iter().take(keep).sum::<f32>() / keep as f32;
        if best.is_none_or(|(_, _, old, _, _)| error < old) {
            best = Some((dy, detected_footer, error, row_diffs.len(), row_diffs.iter().filter(|&&d| d <= 0.10).count()));
        }
    }
    let Some((dy, detected_footer, error, compared, matched)) = best else {
        diagnostic.reason = "insufficient textured overlap";
        return None;
    };
    diagnostic.best_dy = Some(dy);
    diagnostic.compared = compared;
    diagnostic.matched = matched;
    diagnostic.score = 1.0 - error;
    diagnostic.footer = detected_footer;
    if error > 1.0 - MIN_SCORE {
        diagnostic.reason = "score below threshold (0.90)";
        return None;
    }
    diagnostic.reason = "matched";
    let refresh_from = votes.iter().filter_map(|(vote, y)| (*vote == dy).then(|| y.checked_sub(dy)).flatten()).min()?;
    Some((dy, detected_footer, refresh_from))
}

fn textured(img: &RgbaImage, x: usize, y: usize) -> bool {
    [(x.saturating_sub(1), y), (x + 1, y), (x, y.saturating_sub(1)), (x, y + 1)]
        .into_iter()
        .filter(|&(nx, ny)| nx < img.width() as usize && ny < img.height() as usize)
        .any(|(nx, ny)| diff_at(img, x, y, img, nx, ny) >= 0.035)
}

fn infer_footer(prev: &RgbaImage, cur: &RgbaImage, dy: usize, fallback: usize, columns: &[usize]) -> usize {
    let h = prev.height() as usize;
    let mut run = 0;
    for y in (h.saturating_sub(h / 3)..h).rev() {
        if y < dy {
            break;
        }
        let mut diffs: Vec<_> = columns.iter().map(|&x| diff_at(prev, x, y, cur, x, y - dy)).collect();
        diffs.sort_by(f32::total_cmp);
        let keep = (diffs.len() * 3).div_ceil(4);
        let mean = diffs.iter().take(keep.max(1)).sum::<f32>() / keep.max(1) as f32;
        if mean <= 0.025 {
            run += 1;
            if run >= 6 {
                return (h - (y + 6)).max(fallback);
            }
        } else {
            run = 0;
        }
    }
    fallback
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tall page whose every row is distinct noise.
    fn page(w: u32, h: u32) -> RgbaImage {
        let mut state = 0x1234_5678u32;
        RgbaImage::from_fn(w, h, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let b = state.to_le_bytes();
            image::Rgba([b[0], b[1], b[2], 255])
        })
    }

    fn view(page: &RgbaImage, top: u32, h: u32) -> RgbaImage {
        image::imageops::crop_imm(page, 0, top, page.width(), h).to_image()
    }

    #[test]
    fn stitches_plain_scroll_exactly() {
        let p = page(16, 300);
        let mut s = Stitcher::new(10_000);
        for top in [0, 37, 80, 150, 200] {
            assert!(matches!(s.push(view(&p, top, 100)), Push::Appended(_)));
        }
        assert_eq!(s.push(view(&p, 200, 100)), Push::Unchanged);
        assert_eq!(s.finish().unwrap(), p);
    }

    #[test]
    fn keeps_sticky_header_and_footer_once() {
        let p = page(12, 400);
        let header = page(12, 10);
        let footer = view(&page(12, 50), 30, 15);
        let frame = |top: u32| {
            let mut f = view(&p, top, 120);
            image::imageops::replace(&mut f, &header, 0, 0);
            image::imageops::replace(&mut f, &footer, 0, 105);
            f
        };
        let mut s = Stitcher::new(10_000);
        for top in [0, 40, 95, 160] {
            s.push(frame(top));
        }
        let out = s.finish().unwrap();
        // header(10) + page rows 10..(160+105) + footer(15)
        assert_eq!(out.height(), 10 + (160 + 105 - 10) + 15);
        assert_eq!(view(&out, 0, 10), header);
        assert_eq!(view(&out, 10, 255), view(&p, 10, 255));
        assert_eq!(view(&out, out.height() - 15, 15), footer);
    }

    #[test]
    fn keeps_fixed_chrome_when_scroll_is_small() {
        let p = page(160, 300);
        let header = |v| image::RgbaImage::from_pixel(160, 12, image::Rgba([v, v, v, 255]));
        let footer = image::RgbaImage::from_pixel(160, 16, image::Rgba([255, 255, 255, 255]));
        let frame = |top, head| {
            let mut f = view(&p, top, 120);
            image::imageops::replace(&mut f, &header(head), 0, 0);
            image::imageops::replace(&mut f, &footer, 0, 104);
            f
        };
        let mut s = Stitcher::new(10_000);
        assert_eq!(s.push(frame(0, 60)), Push::Appended(120));
        assert_eq!(s.push(frame(4, 100)), Push::Appended(4));

        let mut expected = view(&p, 0, 124);
        image::imageops::replace(&mut expected, &header(60), 0, 0);
        image::imageops::replace(&mut expected, &footer, 0, 108);
        assert_eq!(s.finish().unwrap(), expected);
    }

    #[test]
    fn stitches_with_fixed_sidebar_and_small_body_color_changes() {
        let p = page(160, 400);
        let sidebar = page(40, 120);
        let header = page(160, 12);
        let footer = page(160, 16);
        let make_frame = |top: u32, change_body: bool| {
            let mut frame = view(&p, top, 120);
            image::imageops::replace(&mut frame, &sidebar, 0, 0);
            image::imageops::replace(&mut frame, &header, 0, 0);
            image::imageops::replace(&mut frame, &footer, 0, 104);
            if change_body {
                for y in 12..104 {
                    for x in 40..160 {
                        let pixel = frame.get_pixel_mut(x, y);
                        for channel in &mut pixel.0[..3] {
                            *channel = channel.saturating_add(1);
                        }
                    }
                }
            }
            frame
        };
        let mut first = make_frame(0, false);
        for x in 40..160 {
            first.put_pixel(x, 103, image::Rgba([100, 100, 100, 255]));
        }
        let second = make_frame(37, true);
        let third = make_frame(80, true);
        let expected_tail = view(&third, 61, 59);

        let mut s = Stitcher::new(10_000);
        assert_eq!(s.push(first), Push::Appended(120));
        assert_eq!(s.push(second), Push::Appended(37));
        assert_eq!(s.push(third), Push::Appended(43));
        let out = s.finish().unwrap();
        assert_eq!((out.width(), out.height()), (160, 200));
        assert_eq!(view(&out, 0, 12), header);
        assert_eq!(view(&out, 184, 16), footer);
        assert_eq!(view(&out, 141, 59), expected_tail);
        for y in 12..184 {
            for x in 40..160 {
                let actual = out.get_pixel(x, y);
                let expected = p.get_pixel(x, y);
                for channel in 0..3 {
                    assert!(actual[channel].abs_diff(expected[channel]) <= 1, "pixel ({x}, {y}) channel {channel}");
                }
            }
        }
    }

    #[test]
    fn ignores_unrelated_frame() {
        let mut s = Stitcher::new(10_000);
        s.push(page(8, 60));
        let other = RgbaImage::from_fn(8, 60, |x, y| image::Rgba([(x * 31) as u8, (y * 7) as u8, 99, 255]));
        assert_eq!(s.push(other), Push::NoMatch);
    }

    #[test]
    fn stops_at_max_height() {
        let p = page(8, 400);
        let mut s = Stitcher::new(150);
        s.push(view(&p, 0, 100));
        s.push(view(&p, 90, 100));
        assert!(s.is_full());
        assert_eq!(s.finish().unwrap().height(), 150);
    }

    #[test]
    fn empty_frames_are_skipped() {
        let p = page(8, 200);
        let mut s = Stitcher::new(10_000);
        assert_eq!(s.push(RgbaImage::new(0, 100)), Push::NoMatch, "nothing to start from");
        s.push(view(&p, 0, 100));
        assert_eq!(s.push(RgbaImage::new(0, 100)), Push::NoMatch);
        assert!(matches!(s.push(view(&p, 40, 100)), Push::Appended(40)));
    }
}
