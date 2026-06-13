//! Shell layout metrics shared by the workspace view and drag geometry.
//!
//! Every constant here is derived from the same design tokens the view
//! uses so hit-testing and preview overlays line up with painted widgets.

use iced::{Rectangle, Size};

use crate::shared::design::{SpacingToken, TypeRole};
use crate::workspace::workspace_dock::{Dock, Docks};

pub const SIDE_DOCK_WIDTH: f32 = 280.0;
pub const BOTTOM_DOCK_HEIGHT: f32 = 200.0;
pub const DOCK_RAIL_THICKNESS: f32 = 28.0;

/// Matches [`SpacingToken::Hairline`].
pub const FRAMED_PADDING: f32 = 2.0;
/// Matches [`SpacingToken::Hairline`].
pub const MAIN_AXIS_SPACING: f32 = 2.0;
/// Matches [`SpacingToken::S2`].
pub const PANE_GRID_SPACING: f32 = 8.0;

const BAR_BORDER: f32 = 1.0;

fn type_line_height(role: TypeRole) -> f32 {
    role.size() * role.line_height()
}

/// Height of the top application chrome row.
pub fn title_bar_height() -> f32 {
    let vertical_pad = SpacingToken::S3.value() * 2.0;
    let control_height = SpacingToken::S1.value() * 2.0 + type_line_height(TypeRole::LabelMd);
    vertical_pad + control_height + BAR_BORDER
}

/// Height of the bottom status bar.
pub fn status_bar_height() -> f32 {
    SpacingToken::S2.value() * 2.0 + type_line_height(TypeRole::MonoSm) + BAR_BORDER
}

/// Height of a pane or dock tab strip (container + chip + label).
pub fn tab_strip_height() -> f32 {
    SpacingToken::S1.value() * 4.0 + type_line_height(TypeRole::LabelMd) + BAR_BORDER
}

/// Estimated width of a single tab chip for ghost rendering.
pub fn estimated_tab_width() -> f32 {
    SpacingToken::S3.value() * 2.0 + 72.0
}

/// Inner padding of the tab-strip container (matches the view).
pub fn tab_strip_padding() -> f32 {
    SpacingToken::S1.value()
}

/// Horizontal gap between tab chips in a strip (matches the view).
pub fn tab_chip_spacing() -> f32 {
    SpacingToken::S1.value()
}

/// The framed workspace body between the title and status bars.
pub fn workspace_area(window_size: Size) -> Rectangle {
    let top = title_bar_height();
    let bottom = status_bar_height();
    Rectangle {
        x: 0.0,
        y: top,
        width: window_size.width,
        height: (window_size.height - top - bottom).max(0.0),
    }
}

/// Inner content rectangle inside the framed body padding.
pub fn framed_inner(window_size: Size) -> Rectangle {
    let area = workspace_area(window_size);
    Rectangle {
        x: area.x + FRAMED_PADDING,
        y: area.y + FRAMED_PADDING,
        width: (area.width - FRAMED_PADDING * 2.0).max(0.0),
        height: (area.height - FRAMED_PADDING * 2.0).max(0.0),
    }
}

pub fn dock_horizontal_extent(dock: &Dock) -> f32 {
    if dock.is_empty() {
        0.0
    } else if dock.open {
        SIDE_DOCK_WIDTH
    } else {
        DOCK_RAIL_THICKNESS
    }
}

fn dock_bottom_extent(dock: &Dock) -> f32 {
    if dock.is_empty() {
        0.0
    } else if dock.open {
        BOTTOM_DOCK_HEIGHT
    } else {
        DOCK_RAIL_THICKNESS
    }
}

/// Main-row height and bottom-band height inside the framed body.
pub fn main_row_layout(docks: &Docks, window_size: Size) -> (f32, f32) {
    let inner = framed_inner(window_size);
    let bottom_occ = dock_bottom_extent(&docks.bottom);
    let column_spacing = if bottom_occ > 0.0 {
        MAIN_AXIS_SPACING
    } else {
        0.0
    };
    let main_row_height = (inner.height - bottom_occ - column_spacing).max(0.0);
    (main_row_height, bottom_occ)
}

/// Pixel bounds of the center `PaneGrid` region.
pub fn compute_grid_bounds(docks: &Docks, window_size: Size) -> Rectangle {
    let inner = framed_inner(window_size);
    let (main_row_height, _) = main_row_layout(docks, window_size);

    let left = dock_horizontal_extent(&docks.left);
    let right = dock_horizontal_extent(&docks.right);

    let mut x = inner.x;
    let mut width = inner.width;

    if left > 0.0 {
        x += left + MAIN_AXIS_SPACING;
        width -= left + MAIN_AXIS_SPACING;
    }
    if right > 0.0 {
        width -= right + MAIN_AXIS_SPACING;
    }

    Rectangle {
        x,
        y: inner.y,
        width: width.max(0.0),
        height: main_row_height,
    }
}
