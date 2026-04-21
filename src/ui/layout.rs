const STATUS_BAR_HEIGHT: u16 = 1;
const LEFT_RATIO: f64 = 0.50;
/// Height of a collapsed (inactive) panel: top border + bottom border + 1 line of content.
pub const COLLAPSED_PANEL_H: u16 = 3;
/// Below this width, the layout auto-switches to the vertical (focus+tabs) mode.
pub const NARROW_BREAKPOINT: u16 = 100;

/// Layout mode: horizontal = classic desktop split, vertical = one full-screen panel at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Default)]
pub struct Layout {
    pub width: u16,
    pub height: u16,
    pub left_width: u16,
    pub right_width: u16,
    pub content_h: u16,
    pub status_bar_h: u16,
    pub mode: LayoutMode,
}

/// Calculates panel sizes from terminal dimensions.
///
/// `override_mode` forces a specific layout mode; when `None`, the mode is
/// auto-selected based on `width` (< `NARROW_BREAKPOINT` → Vertical).
pub fn compute_layout(width: u16, height: u16, override_mode: Option<LayoutMode>) -> Layout {
    let content_h = height.saturating_sub(STATUS_BAR_HEIGHT);

    let mut left_w = (width as f64 * LEFT_RATIO) as u16;
    if left_w < 20 {
        left_w = 20.min(width);
    }
    let right_w = width.saturating_sub(left_w);

    let mode = override_mode.unwrap_or(if width < NARROW_BREAKPOINT {
        LayoutMode::Vertical
    } else {
        LayoutMode::Horizontal
    });

    Layout {
        width,
        height,
        left_width: left_w,
        right_width: right_w,
        content_h,
        status_bar_h: STATUS_BAR_HEIGHT,
        mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_terminal() {
        let l = compute_layout(120, 40, None);
        assert_eq!(l.width, 120);
        assert_eq!(l.height, 40);
        assert_eq!(l.content_h, 39);
        assert_eq!(l.left_width, 60);
        assert_eq!(l.right_width, 60);
        assert_eq!(l.left_width + l.right_width, l.width);
    }

    #[test]
    fn narrow_terminal() {
        let l = compute_layout(40, 20, None);
        assert_eq!(l.left_width, 20);
        assert_eq!(l.right_width, 20);
    }

    #[test]
    fn content_height_calculation() {
        let tests = vec![
            ("standard", 50u16, 49u16),
            ("small", 10, 9),
            ("minimal", 3, 2),
        ];

        for (name, height, want_ch) in tests {
            let l = compute_layout(100, height, None);
            assert_eq!(l.content_h, want_ch, "case={name}");
        }
    }

    #[test]
    fn auto_vertical_when_narrow() {
        let l = compute_layout(80, 40, None);
        assert_eq!(l.mode, LayoutMode::Vertical);
    }

    #[test]
    fn auto_horizontal_when_wide() {
        let l = compute_layout(120, 40, None);
        assert_eq!(l.mode, LayoutMode::Horizontal);
    }

    #[test]
    fn breakpoint_is_exclusive() {
        // Exactly at NARROW_BREAKPOINT → Horizontal (< not <=)
        let l = compute_layout(NARROW_BREAKPOINT, 40, None);
        assert_eq!(l.mode, LayoutMode::Horizontal);
    }

    #[test]
    fn override_forces_horizontal() {
        let l = compute_layout(60, 40, Some(LayoutMode::Horizontal));
        assert_eq!(l.mode, LayoutMode::Horizontal);
    }

    #[test]
    fn override_forces_vertical() {
        let l = compute_layout(200, 40, Some(LayoutMode::Vertical));
        assert_eq!(l.mode, LayoutMode::Vertical);
    }
}
