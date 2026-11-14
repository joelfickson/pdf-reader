use std::ops::Range;

pub(super) const PAGE_GAP: f32 = 16.0;
pub(super) const COLUMN_TOP: f32 = 28.0;
pub(super) const SPREAD_GAP: f32 = 4.0;
const POINTS_TO_PIXELS: f32 = 96.0 / 72.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Spread {
    #[default]
    Single,
    Pairs,
    CoverFirst,
}

impl Spread {
    fn row(self, page: usize, count: usize) -> Range<usize> {
        let start = match self {
            Self::Single => return page..page + 1,
            Self::Pairs => page - page % 2,
            Self::CoverFirst if page == 0 => return 0..1,
            Self::CoverFirst => page - (page - 1) % 2,
        };
        start..(start + 2).min(count)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Column {
    pub key: (u32, Option<u32>, Spread),
    pub lefts: Vec<f32>,
    pub widths: Vec<f32>,
    pub heights: Vec<f32>,
    pub tops: Vec<f32>,
    pub width: f32,
    pub height: f32,
}

fn median_width(sizes: &[(f32, f32)]) -> f32 {
    let mut widths: Vec<f32> = sizes
        .iter()
        .map(|(width, _)| *width)
        .filter(|width| width.is_finite() && *width > 0.0)
        .collect();
    if widths.is_empty() {
        return 612.0;
    }
    widths.sort_by(f32::total_cmp);
    widths[widths.len() / 2]
}

impl Column {
    pub fn new(sizes: &[(f32, f32)], available: u32, zoom: Option<u32>, spread: Spread) -> Self {
        let whole = available.max(1) as f32;
        let available = match spread {
            Spread::Single => whole,
            Spread::Pairs | Spread::CoverFirst => ((whole - SPREAD_GAP) / 2.0).max(1.0),
        };
        let fit = available / median_width(sizes);
        let mut widths = Vec::with_capacity(sizes.len());
        let mut heights = Vec::with_capacity(sizes.len());
        let mut tops = Vec::with_capacity(sizes.len());
        let mut cursor = 0.0;
        for &(width, height) in sizes {
            let (width, height) =
                if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
                    (width, height)
                } else {
                    (612.0, 792.0)
                };
            let shown = match zoom {
                Some(zoom) => width * POINTS_TO_PIXELS * zoom as f32 / 100.0,
                None => (width * fit).min(available).round().max(1.0),
            };
            widths.push(shown);
            heights.push(shown * height / width);
        }
        let count = widths.len();
        let mut rows = Vec::new();
        let mut page = 0;
        while page < count {
            let row = spread.row(page, count);
            page = row.end;
            rows.push(row);
        }
        let across = |row: &Range<usize>| {
            widths[row.clone()].iter().sum::<f32>() + SPREAD_GAP * (row.len() as f32 - 1.0)
        };
        let width = rows.iter().map(across).fold(0.0, f32::max);
        let mut lefts = vec![0.0; count];
        for row in &rows {
            let mut left = (width - across(row)) / 2.0;
            let tallest = heights[row.clone()].iter().copied().fold(0.0, f32::max);
            for page in row.clone() {
                tops.push(cursor);
                lefts[page] = left;
                left += widths[page] + SPREAD_GAP;
            }
            cursor += tallest + PAGE_GAP;
        }
        Self {
            key: (whole as u32, zoom, spread),
            width,
            height: (cursor - PAGE_GAP).max(0.0),
            widths,
            heights,
            tops,
            lefts,
        }
    }

    pub fn len(&self) -> usize {
        self.tops.len()
    }

    fn first_at(&self, y: f32) -> usize {
        self.tops
            .partition_point(|top| *top <= y)
            .saturating_sub(1)
            .min(self.len().saturating_sub(1))
    }

    pub fn visible(&self, top: f32, height: f32) -> Range<usize> {
        if self.len() == 0 {
            return 0..0;
        }
        let mut first = self.first_at(top);
        let mut end = first;
        while first > 0 && self.tops[first - 1] == self.tops[first] {
            first -= 1;
        }
        while end + 1 < self.len() && self.tops[end + 1] == self.tops[first] {
            end += 1;
        }
        let bottom = (first..=end)
            .map(|page| self.tops[page] + self.heights[page])
            .fold(0.0, f32::max);
        let first = if bottom < top {
            (end + 1).min(self.len() - 1)
        } else {
            first
        };
        let mut last = self.first_at(top + height.max(1.0)).max(first);
        while last + 1 < self.len() && self.tops[last + 1] == self.tops[last] {
            last += 1;
        }
        first..last + 1
    }

    pub fn dominant(&self, top: f32, height: f32) -> usize {
        let bottom = top + height.max(1.0);
        self.visible(top, height)
            .max_by(|a, b| {
                let seen = |page: &usize| {
                    let start = self.tops[*page].max(top);
                    let end = (self.tops[*page] + self.heights[*page]).min(bottom);
                    (end - start).max(0.0)
                };
                seen(a).total_cmp(&seen(b)).then(b.cmp(a))
            })
            .unwrap_or(0)
    }

    pub fn scroll_to(&self, page: usize) -> f32 {
        match self.tops.get(page) {
            Some(_) if page == 0 => 0.0,
            Some(top) => COLUMN_TOP + top - PAGE_GAP,
            None => 0.0,
        }
    }

    pub fn view_width(&self, page: usize) -> u32 {
        match self.key.1 {
            Some(_) => 0,
            None => self.widths.get(page).copied().unwrap_or(1.0) as u32,
        }
    }

    pub fn around(&self, top: f32, height: f32) -> Range<usize> {
        let visible = self.visible(top, height);
        visible.start.saturating_sub(1)..(visible.end + 1).min(self.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_page_layouts_pair_pages_side_by_side_and_keep_rows_whole() {
        let sizes = vec![(612.0, 792.0); 5];
        let pairs = Column::new(&sizes, 604, None, Spread::Pairs);
        assert_eq!(pairs.widths, [300.0; 5]);
        assert_eq!(pairs.width, 604.0);
        assert_eq!(pairs.tops[0], pairs.tops[1]);
        assert_eq!(pairs.tops[2], pairs.tops[3]);
        assert!(pairs.tops[2] > pairs.tops[1] && pairs.tops[4] > pairs.tops[3]);
        assert_eq!((pairs.lefts[0], pairs.lefts[1]), (0.0, 304.0));
        assert_eq!(pairs.lefts[4], 152.0);
        let row = 300.0 * 792.0 / 612.0;
        assert_eq!(pairs.visible(0.0, 10.0), 0..2);
        assert_eq!(pairs.visible(row + PAGE_GAP + 5.0, 10.0), 2..4);
        assert_eq!(pairs.visible(row + 1.0, 10.0), 2..4);
        assert_eq!(pairs.visible(row - 1.0, PAGE_GAP + 10.0), 0..4);
        assert_eq!(pairs.around(0.0, 10.0), 0..3);
        assert_eq!(pairs.view_width(3), 300);

        let cover = Column::new(&sizes, 604, None, Spread::CoverFirst);
        assert_eq!(cover.lefts[0], 152.0);
        assert_eq!(cover.tops[1], cover.tops[2]);
        assert_eq!(cover.tops[3], cover.tops[4]);
        assert!(cover.tops[1] > cover.tops[0]);
        assert_eq!(cover.visible(0.0, 10.0), 0..1);
        assert_ne!(pairs.key, cover.key);

        let single = Column::new(&sizes, 604, None, Spread::Single);
        assert_eq!(single.lefts, [0.0; 5]);
        assert_eq!(single.widths, [604.0; 5]);
    }

    fn letters(count: usize) -> Vec<(f32, f32)> {
        vec![(612.0, 792.0); count]
    }

    #[test]
    fn fit_lays_out_pages_with_gaps_and_caps_wide_pages() {
        let mut sizes = letters(3);
        sizes[1] = (792.0, 612.0);
        let column = Column::new(&sizes, 600, None, Spread::Single);
        assert_eq!(column.widths, [600.0, 600.0, 600.0]);
        assert_eq!(column.heights[0], 600.0 * 792.0 / 612.0);
        assert_eq!(column.heights[1], 600.0 * 612.0 / 792.0);
        assert_eq!(column.tops[1], column.heights[0] + PAGE_GAP);
        assert_eq!(
            column.height,
            column.heights.iter().sum::<f32>() + 2.0 * PAGE_GAP
        );
        assert_eq!(column.view_width(1), 600);
    }

    #[test]
    fn zoom_keeps_each_page_at_its_own_width() {
        let mut sizes = letters(2);
        sizes[1] = (792.0, 612.0);
        let column = Column::new(&sizes, 600, Some(100), Spread::Single);
        assert_eq!(column.widths, [816.0, 1056.0]);
        assert_eq!(column.width, 1056.0);
        assert_eq!(column.view_width(0), 0);
    }

    #[test]
    fn tracks_visible_and_dominant_pages() {
        let column = Column::new(&letters(10), 612, None, Spread::Single);
        let page = 792.0 + PAGE_GAP;
        assert_eq!(column.visible(0.0, 700.0), 0..1);
        assert_eq!(column.visible(700.0, 300.0), 0..2);
        assert_eq!(column.visible(page * 3.0 + 10.0, 700.0), 3..4);
        assert_eq!(column.visible(792.0 + 4.0, 10.0), 1..2);
        assert_eq!(column.dominant(700.0, 300.0), 1);
        assert_eq!(column.dominant(500.0, 400.0), 0);
        assert_eq!(column.dominant(page * 9.5, 2000.0), 9);
        assert_eq!(column.around(page * 3.0 + 10.0, 700.0), 2..5);
        assert_eq!(column.around(0.0, 700.0), 0..2);
        assert_eq!(column.scroll_to(0), 0.0);
        assert_eq!(column.scroll_to(2), COLUMN_TOP + page * 2.0 - PAGE_GAP);
    }

    #[test]
    fn invalid_sizes_fall_back_to_letter() {
        let column = Column::new(&[(0.0, 0.0), (f32::NAN, 10.0)], 612, None, Spread::Single);
        assert_eq!(column.heights, [792.0, 792.0]);
        assert!(
            Column::new(&[], 612, None, Spread::Single)
                .visible(0.0, 100.0)
                .is_empty()
        );
    }
}
