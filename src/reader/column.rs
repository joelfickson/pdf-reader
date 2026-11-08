use std::ops::Range;

pub(super) const PAGE_GAP: f32 = 16.0;
pub(super) const COLUMN_TOP: f32 = 28.0;
const POINTS_TO_PIXELS: f32 = 96.0 / 72.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Column {
    pub key: (u32, Option<u32>),
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
    pub fn new(sizes: &[(f32, f32)], available: u32, zoom: Option<u32>) -> Self {
        let available = available.max(1) as f32;
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
            tops.push(cursor);
            widths.push(shown);
            let shown_height = shown * height / width;
            heights.push(shown_height);
            cursor += shown_height + PAGE_GAP;
        }
        Self {
            key: (available as u32, zoom),
            width: widths.iter().copied().fold(0.0, f32::max),
            height: (cursor - PAGE_GAP).max(0.0),
            widths,
            heights,
            tops,
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
        let first = self.first_at(top);
        let first = if self.tops[first] + self.heights[first] < top {
            (first + 1).min(self.len() - 1)
        } else {
            first
        };
        let last = self.first_at(top + height.max(1.0));
        first..last.max(first) + 1
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

    fn letters(count: usize) -> Vec<(f32, f32)> {
        vec![(612.0, 792.0); count]
    }

    #[test]
    fn fit_lays_out_pages_with_gaps_and_caps_wide_pages() {
        let mut sizes = letters(3);
        sizes[1] = (792.0, 612.0);
        let column = Column::new(&sizes, 600, None);
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
        let column = Column::new(&sizes, 600, Some(100));
        assert_eq!(column.widths, [816.0, 1056.0]);
        assert_eq!(column.width, 1056.0);
        assert_eq!(column.view_width(0), 0);
    }

    #[test]
    fn tracks_visible_and_dominant_pages() {
        let column = Column::new(&letters(10), 612, None);
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
        let column = Column::new(&[(0.0, 0.0), (f32::NAN, 10.0)], 612, None);
        assert_eq!(column.heights, [792.0, 792.0]);
        assert!(Column::new(&[], 612, None).visible(0.0, 100.0).is_empty());
    }
}
