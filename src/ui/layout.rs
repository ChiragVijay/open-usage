//! Home keeps every current account visible; crowded windows use compact rows.

pub const HEADER_HEIGHT: f32 = 48.;

#[derive(Debug, Clone, Copy)]
pub struct OverviewLayout {
    pub width: f32,
    pub gap: f32,
    pub dense: bool,
    pub cards: bool,
    pub columns: usize,
    pub item_height: f32,
    pub limits: usize,
    pub accounts_height: f32,
    pub scroll_accounts: bool,
    pub summary_height: f32,
    pub chart_height: f32,
}

pub fn sidebar_width(width: f32) -> f32 {
    if width < 1000. { 56. } else { 188. }
}

pub fn page_padding(width: f32, height: f32) -> f32 {
    if width < 1000. || height < 700. {
        12.
    } else {
        24.
    }
}

impl OverviewLayout {
    pub fn new(width: f32, height: f32, accounts: usize, limits: usize) -> Self {
        let padding = page_padding(width, height);
        let width = width - sidebar_width(width) - padding * 2.;
        let height = height - HEADER_HEIGHT - padding * 2.;
        let dense = height < 660. || width < 650.;
        let gap = if dense { 8. } else { 12. };
        let summary_height = if dense { 56. } else { 76. };
        let chrome = 28. + summary_height + 20. + 20. + gap * 5.;
        let available = height - chrome;
        let card_columns = ((width + gap) / (260. + gap)).floor().max(1.) as usize;
        let card_columns = card_columns.min(accounts.max(1));
        let card_rows = accounts.max(1).div_ceil(card_columns);
        let preferred_chart = (height * 0.24).clamp(84., 220.);
        let card_budget = (available - preferred_chart - card_rows.saturating_sub(1) as f32 * gap)
            / card_rows as f32;
        let slots = ((card_budget - 124.) / 48.).floor().max(0.) as usize;
        let cards = width >= 650. && slots >= limits.clamp(1, 2);
        let columns = if cards {
            card_columns
        } else {
            (((width + gap) / (375. + gap)).floor().max(1.) as usize).min(accounts.max(1))
        };
        let limits = if cards { slots.min(limits.max(1)) } else { 2 };
        let item_height = if cards {
            124. + limits as f32 * 48.
        } else {
            80.
        };
        let rows = accounts.max(1).div_ceil(columns);
        let grid_height = rows as f32 * item_height + rows.saturating_sub(1) as f32 * gap;
        // A bounded account region handles a future provider count larger than the screen.
        // Six providers fit together at normal desktop sizes.
        let accounts_height = grid_height.min(available - 64.);
        Self {
            width,
            gap,
            dense,
            cards,
            columns,
            item_height,
            limits,
            accounts_height,
            scroll_accounts: grid_height > accounts_height,
            summary_height,
            chart_height: available - accounts_height,
        }
    }

    pub fn item_width(self) -> f32 {
        (self.width - self.columns.saturating_sub(1) as f32 * self.gap) / self.columns as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_current_accounts_fit_without_paging_or_scrolling() {
        for width in [420., 640., 850., 1000., 1280., 1920.] {
            for height in [560., 640., 768., 860., 1080.] {
                for limits in [0, 1, 2, 4, 12] {
                    let layout = OverviewLayout::new(width, height, 3, limits);
                    assert!(!layout.scroll_accounts, "{width}x{height}: {layout:?}");
                    assert!(layout.chart_height >= 64.);
                    assert!(layout.item_width() >= 260.);
                }
            }
        }
    }

    #[test]
    fn future_accounts_preserve_the_viewport_budget() {
        for accounts in [0, 1, 3, 8, 20, 100] {
            for (width, height) in [(420., 560.), (1280., 860.), (1920., 1080.)] {
                let layout = OverviewLayout::new(width, height, accounts, 4);
                let occupied = HEADER_HEIGHT
                    + page_padding(width, height) * 2.
                    + 28.
                    + layout.summary_height
                    + 20.
                    + 20.
                    + layout.gap * 5.
                    + layout.accounts_height
                    + layout.chart_height;
                assert!((occupied - height).abs() < 0.01);
                assert!(layout.accounts_height >= 80. && layout.chart_height >= 64.);
            }
        }
    }
    #[test]
    fn six_accounts_fit_at_desktop_sizes() {
        for (width, height) in [(1000., 768.), (1280., 768.), (1280., 860.), (1920., 1080.)] {
            let layout = OverviewLayout::new(width, height, 6, 4);
            assert!(!layout.scroll_accounts, "{width}x{height}: {layout:?}");
        }
    }
}
