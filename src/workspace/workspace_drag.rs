//! Tab drag-and-drop subsystem.
//!
//! Isolates bounds capture and hit testing for custom tab DnD on top of
//! the multi-window shell. Native pane_grid drag for whole-pane
//! rearrangement is untouched; this module handles tab-strip drag,
//! drop-into-strip, edge-split, dock drop, and cross-window tab move.

use iced::widget::pane_grid;
use iced::window;
use iced::{Point, Rectangle, Size};

use crate::workspace::workspace_dock::Docks;
use crate::workspace::workspace_layout_metrics::{
    self, BOTTOM_DOCK_HEIGHT, DOCK_RAIL_THICKNESS, MAIN_AXIS_SPACING, PANE_GRID_SPACING,
    SIDE_DOCK_WIDTH, dock_horizontal_extent, estimated_tab_width, tab_chip_spacing,
    tab_strip_height, tab_strip_padding,
};
use crate::workspace::workspace_location::{DockSide, PanelLocation};
use crate::workspace::workspace_pane_state::PaneState;

/// Direction for edge-split drops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// Resolved drop target from hit-testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropTarget {
    TabStrip(TabStripTarget),
    SplitPane(SplitPaneTarget),
    Dock(DockSide),
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabStripTarget {
    pub location: PanelLocation,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitPaneTarget {
    pub pane: pane_grid::Pane,
    pub direction: Direction,
}

/// The current tab drag operation state.
#[derive(Debug, Clone)]
pub struct DragState {
    pub source_location: PanelLocation,
    pub source_tab: usize,
    pub target: DropTarget,
    /// Window that should receive the drop when dragging across OS windows.
    pub target_window: Option<window::Id>,
    /// OS window that last reported [`Self::cursor`], in that window's client coords.
    pub cursor_window: Option<window::Id>,
    /// Latest cursor position in window coordinates.
    pub cursor: Point,
    /// Set once the cursor moves during an active drag. A drop with
    /// [`DropTarget::None`] and no movement is treated as a tab click.
    pub pointer_moved: bool,
}

impl DragState {
    pub fn new(source_location: PanelLocation, source_tab: usize) -> Self {
        Self {
            source_location,
            source_tab,
            target: DropTarget::None,
            target_window: None,
            cursor_window: None,
            cursor: Point::ORIGIN,
            pointer_moved: false,
        }
    }
}

/// Pixel bounds of a center pane plus its tab strip.
#[derive(Debug, Clone, Copy)]
pub struct PaneBounds {
    pub pane: pane_grid::Pane,
    pub bounds: Rectangle,
    pub tab_strip: Rectangle,
    pub tab_count: usize,
}

/// Rails and body rectangles for all three dock sides.
pub type DockRegions = ([(DockSide, Rectangle); 3], [(DockSide, Rectangle); 3]);

const SPLIT_EDGE_FRACTION: f32 = 0.25;
const TAB_INSERT_MARKER_WIDTH: f32 = 2.0;

/// Compute per-pane pixel bounds by walking the pane grid's split tree.
pub fn compute_pane_bounds(
    panes: &pane_grid::State<PaneState>,
    grid_bounds: Rectangle,
) -> Vec<PaneBounds> {
    let strip_h = tab_strip_height();
    let mut out = Vec::new();
    let root = panes.layout();

    fn walk(
        node: &pane_grid::Node,
        region: Rectangle,
        spacing: f32,
        strip_h: f32,
        panes: &pane_grid::State<PaneState>,
        out: &mut Vec<PaneBounds>,
    ) {
        match node {
            pane_grid::Node::Split {
                axis, ratio, a, b, ..
            } => {
                let (region_a, region_b) = match axis {
                    pane_grid::Axis::Horizontal => {
                        let content_h = (region.height - spacing).max(0.0);
                        let split_at = region.y + content_h * ratio;
                        (
                            Rectangle {
                                y: region.y,
                                height: split_at - region.y,
                                ..region
                            },
                            Rectangle {
                                y: split_at + spacing,
                                height: region.y + region.height - (split_at + spacing),
                                ..region
                            },
                        )
                    }
                    pane_grid::Axis::Vertical => {
                        let content_w = (region.width - spacing).max(0.0);
                        let split_at = region.x + content_w * ratio;
                        (
                            Rectangle {
                                x: region.x,
                                width: split_at - region.x,
                                ..region
                            },
                            Rectangle {
                                x: split_at + spacing,
                                width: region.x + region.width - (split_at + spacing),
                                ..region
                            },
                        )
                    }
                };
                walk(a, region_a, spacing, strip_h, panes, out);
                walk(b, region_b, spacing, strip_h, panes, out);
            }
            pane_grid::Node::Pane(pane) => {
                let tab_count = panes.get(*pane).map(PaneState::len).unwrap_or(0);
                let tab_strip = Rectangle {
                    x: region.x,
                    y: region.y,
                    width: region.width,
                    height: strip_h.min(region.height),
                };
                out.push(PaneBounds {
                    pane: *pane,
                    bounds: region,
                    tab_strip,
                    tab_count,
                });
            }
        }
    }

    walk(
        root,
        grid_bounds,
        PANE_GRID_SPACING,
        strip_h,
        panes,
        &mut out,
    );
    out
}

/// Compute dock positions (rails and open bodies) from window size and dock state.
pub fn compute_dock_regions(docks: &Docks, window_size: Size) -> DockRegions {
    let inner = workspace_layout_metrics::framed_inner(window_size);
    let (main_row_height, bottom_occ) =
        workspace_layout_metrics::main_row_layout(docks, window_size);
    let main_row_y = inner.y;
    let bottom_y = inner.y
        + main_row_height
        + if bottom_occ > 0.0 {
            MAIN_AXIS_SPACING
        } else {
            0.0
        };

    let left_extent = dock_horizontal_extent(&docks.left);
    let right_extent = dock_horizontal_extent(&docks.right);

    let center_x = inner.x
        + left_extent
        + if left_extent > 0.0 {
            MAIN_AXIS_SPACING
        } else {
            0.0
        };
    let center_width = (inner.width
        - left_extent
        - right_extent
        - if left_extent > 0.0 {
            MAIN_AXIS_SPACING
        } else {
            0.0
        }
        - if right_extent > 0.0 {
            MAIN_AXIS_SPACING
        } else {
            0.0
        })
    .max(0.0);

    let left_rail = if !docks.left.open && !docks.left.tabs.is_empty() {
        Rectangle {
            x: inner.x,
            y: main_row_y,
            width: DOCK_RAIL_THICKNESS,
            height: main_row_height,
        }
    } else {
        Rectangle::default()
    };

    let right_rail = if !docks.right.open && !docks.right.tabs.is_empty() {
        Rectangle {
            x: inner.x + inner.width - DOCK_RAIL_THICKNESS,
            y: main_row_y,
            width: DOCK_RAIL_THICKNESS,
            height: main_row_height,
        }
    } else {
        Rectangle::default()
    };

    let bottom_rail = if !docks.bottom.open && !docks.bottom.tabs.is_empty() {
        Rectangle {
            x: center_x,
            y: bottom_y,
            width: center_width,
            height: DOCK_RAIL_THICKNESS,
        }
    } else {
        Rectangle::default()
    };

    let left_body = if docks.left.open {
        Rectangle {
            x: inner.x,
            y: main_row_y,
            width: SIDE_DOCK_WIDTH,
            height: main_row_height,
        }
    } else {
        Rectangle::default()
    };

    let right_body = if docks.right.open {
        Rectangle {
            x: inner.x + inner.width - SIDE_DOCK_WIDTH,
            y: main_row_y,
            width: SIDE_DOCK_WIDTH,
            height: main_row_height,
        }
    } else {
        Rectangle::default()
    };

    let bottom_body = if docks.bottom.open {
        Rectangle {
            x: center_x,
            y: bottom_y,
            width: center_width,
            height: BOTTOM_DOCK_HEIGHT,
        }
    } else {
        Rectangle::default()
    };

    let rails = [
        (DockSide::Left, left_rail),
        (DockSide::Right, right_rail),
        (DockSide::Bottom, bottom_rail),
    ];
    let bodies = [
        (DockSide::Left, left_body),
        (DockSide::Right, right_body),
        (DockSide::Bottom, bottom_body),
    ];

    (rails, bodies)
}

fn omit_tab_at(location: PanelLocation, drag: Option<&DragState>) -> Option<usize> {
    drag.and_then(|d| (d.source_location == location).then_some(d.source_tab))
}

/// Hit-test cursor position against all drop zones and resolve a [`DropTarget`].
pub fn compute_drop_target(
    cursor: Point,
    grid_bounds: Rectangle,
    pane_bounds: &[PaneBounds],
    dock_rails: &[(DockSide, Rectangle)],
    dock_bodies: &[(DockSide, Rectangle)],
    docks: &Docks,
    drag: Option<&DragState>,
) -> DropTarget {
    let strip_h = tab_strip_height();

    // 1. Tab strips of center panes.
    for pb in pane_bounds {
        if pb.tab_strip.contains(cursor) {
            let location = PanelLocation::Center(pb.pane);
            return DropTarget::TabStrip(TabStripTarget {
                location,
                index: tab_insert_index(
                    cursor.x,
                    pb.tab_strip,
                    pb.tab_count,
                    omit_tab_at(location, drag),
                ),
            });
        }
    }

    // 2. Dock tab strips (open docks only).
    for &(side, body) in dock_bodies {
        if body.width > 0.0 && body.height > 0.0 {
            let dock_strip = Rectangle {
                x: body.x,
                y: body.y,
                width: body.width,
                height: strip_h.min(body.height),
            };
            if dock_strip.contains(cursor) {
                let location = PanelLocation::Dock(side);
                let tab_count = docks.get(side).tabs.len();
                return DropTarget::TabStrip(TabStripTarget {
                    location,
                    index: tab_insert_index(
                        cursor.x,
                        dock_strip,
                        tab_count,
                        omit_tab_at(location, drag),
                    ),
                });
            }
        }
    }

    // 3. Open dock bodies (content area).
    for &(side, body) in dock_bodies {
        if body.width > 0.0 && body.height > 0.0 {
            let content = Rectangle {
                x: body.x,
                y: body.y + strip_h.min(body.height),
                width: body.width,
                height: (body.height - strip_h.min(body.height)).max(0.0),
            };
            if content.contains(cursor) {
                return DropTarget::Dock(side);
            }
        }
    }

    // 4. Pane edges for split targets (below tab strip only).
    for pb in pane_bounds {
        if !pb.bounds.contains(cursor) {
            continue;
        }

        let quarter_w = pb.bounds.width * SPLIT_EDGE_FRACTION;
        let body_h = (pb.bounds.height - strip_h).max(0.0);
        let quarter_h = body_h * SPLIT_EDGE_FRACTION;
        let rel_x = cursor.x - pb.bounds.x;
        let rel_y = cursor.y - pb.bounds.y;
        let below_strip = rel_y > strip_h;

        if below_strip {
            if rel_x < quarter_w {
                return DropTarget::SplitPane(SplitPaneTarget {
                    pane: pb.pane,
                    direction: Direction::Left,
                });
            }
            if rel_x > pb.bounds.width - quarter_w {
                return DropTarget::SplitPane(SplitPaneTarget {
                    pane: pb.pane,
                    direction: Direction::Right,
                });
            }
            if rel_y < strip_h + quarter_h {
                return DropTarget::SplitPane(SplitPaneTarget {
                    pane: pb.pane,
                    direction: Direction::Up,
                });
            }
            if rel_y > pb.bounds.height - quarter_h {
                return DropTarget::SplitPane(SplitPaneTarget {
                    pane: pb.pane,
                    direction: Direction::Down,
                });
            }

            return DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(pb.pane),
                index: pb.tab_count,
            });
        }

        let location = PanelLocation::Center(pb.pane);
        return DropTarget::TabStrip(TabStripTarget {
            location,
            index: tab_insert_index(
                cursor.x,
                pb.tab_strip,
                pb.tab_count,
                omit_tab_at(location, drag),
            ),
        });
    }

    // 5. Dock rails — only outside the center grid.
    for &(side, rail) in dock_rails {
        if rail.width > 0.0
            && rail.height > 0.0
            && rail.contains(cursor)
            && !grid_bounds.contains(cursor)
        {
            return DropTarget::Dock(side);
        }
    }

    DropTarget::None
}

fn visible_tabs_before(logical_index: usize, tab_count: usize, omit: Option<usize>) -> usize {
    (0..logical_index.min(tab_count))
        .filter(|index| omit != Some(*index))
        .count()
}

/// Left-packed x coordinate for a logical insert index (0..=tab_count).
fn tab_insert_marker_x(
    strip: Rectangle,
    tab_count: usize,
    index: usize,
    omit: Option<usize>,
) -> f32 {
    let visible_before = visible_tabs_before(index.min(tab_count), tab_count, omit);
    strip.x
        + tab_strip_padding()
        + visible_before as f32 * (estimated_tab_width() + tab_chip_spacing())
}

fn tab_insert_index(
    cursor_x: f32,
    strip: Rectangle,
    tab_count: usize,
    omit: Option<usize>,
) -> usize {
    if tab_count == 0 {
        return 0;
    }

    for index in 0..tab_count {
        let left = tab_insert_marker_x(strip, tab_count, index, omit);
        let right = tab_insert_marker_x(strip, tab_count, index + 1, omit);
        if cursor_x < (left + right) / 2.0 {
            return index;
        }
    }

    tab_count
}

/// Pixel rectangle to highlight for a resolved [`DropTarget`].
pub fn preview_bounds(
    target: DropTarget,
    pane_bounds: &[PaneBounds],
    dock_rails: &[(DockSide, Rectangle)],
    dock_bodies: &[(DockSide, Rectangle)],
    docks: &Docks,
    drag: Option<&DragState>,
) -> Option<Rectangle> {
    match target {
        DropTarget::TabStrip(strip) => {
            tab_insert_marker(strip, pane_bounds, dock_bodies, docks, drag)
        }
        DropTarget::SplitPane(split) => pane_bounds
            .iter()
            .find(|pb| pb.pane == split.pane)
            .map(|pb| split_preview_rect(pb, split.direction)),
        DropTarget::Dock(side) => dock_preview_bounds(side, dock_rails, dock_bodies),
        DropTarget::None => None,
    }
}

fn tab_insert_marker(
    strip: TabStripTarget,
    pane_bounds: &[PaneBounds],
    dock_bodies: &[(DockSide, Rectangle)],
    docks: &Docks,
    drag: Option<&DragState>,
) -> Option<Rectangle> {
    let strip_h = tab_strip_height();
    let strip_rect = match strip.location {
        PanelLocation::Center(pane) => pane_bounds
            .iter()
            .find(|pb| pb.pane == pane)
            .map(|pb| pb.tab_strip),
        PanelLocation::Dock(side) => dock_bodies.iter().find_map(|&(dock_side, body)| {
            if dock_side == side && body.width > 0.0 && body.height > 0.0 {
                Some(Rectangle {
                    x: body.x,
                    y: body.y,
                    width: body.width,
                    height: strip_h.min(body.height),
                })
            } else {
                None
            }
        }),
    }?;

    if let Some(drag) = drag
        && strip.location == drag.source_location
    {
        return None;
    }

    let tab_count = match strip.location {
        PanelLocation::Center(pane) => pane_bounds
            .iter()
            .find(|pb| pb.pane == pane)
            .map(|pb| pb.tab_count)
            .unwrap_or(0),
        PanelLocation::Dock(side) => docks.get(side).tabs.len(),
    };

    let x = tab_insert_marker_x(
        strip_rect,
        tab_count,
        strip.index,
        omit_tab_at(strip.location, drag),
    );

    Some(Rectangle {
        x: x - TAB_INSERT_MARKER_WIDTH / 2.0,
        y: strip_rect.y,
        width: TAB_INSERT_MARKER_WIDTH,
        height: strip_rect.height,
    })
}

fn dock_preview_bounds(
    side: DockSide,
    dock_rails: &[(DockSide, Rectangle)],
    dock_bodies: &[(DockSide, Rectangle)],
) -> Option<Rectangle> {
    dock_bodies
        .iter()
        .find_map(|&(dock_side, body)| {
            if dock_side == side && body.width > 0.0 && body.height > 0.0 {
                Some(body)
            } else {
                None
            }
        })
        .or_else(|| {
            dock_rails.iter().find_map(|&(dock_side, rail)| {
                if dock_side == side && rail.width > 0.0 && rail.height > 0.0 {
                    Some(rail)
                } else {
                    None
                }
            })
        })
}

fn split_preview_rect(pb: &PaneBounds, direction: Direction) -> Rectangle {
    let strip_h = tab_strip_height();
    let body = Rectangle {
        x: pb.bounds.x,
        y: pb.bounds.y + strip_h,
        width: pb.bounds.width,
        height: (pb.bounds.height - strip_h).max(0.0),
    };

    match direction {
        Direction::Left => Rectangle {
            width: body.width / 2.0,
            ..body
        },
        Direction::Right => Rectangle {
            x: body.x + body.width / 2.0,
            width: body.width / 2.0,
            y: body.y,
            height: body.height,
        },
        Direction::Up => Rectangle {
            height: body.height / 2.0,
            ..body
        },
        Direction::Down => Rectangle {
            x: body.x,
            y: body.y + body.height / 2.0,
            width: body.width,
            height: body.height / 2.0,
        },
    }
}

/// Compute the grid area from window size and dock state.
pub fn compute_grid_bounds(docks: &Docks, window_size: Size) -> Rectangle {
    workspace_layout_metrics::compute_grid_bounds(docks, window_size)
}

/// Per-window geometry bundle for cross-window drop hit-testing.
pub struct WindowDropGeometry {
    pub window_size: Size,
    pub grid_bounds: Rectangle,
    pub pane_bounds: Vec<PaneBounds>,
    pub dock_rails: Vec<(DockSide, Rectangle)>,
    pub dock_bodies: Vec<(DockSide, Rectangle)>,
}

/// Resolve a drop target within a single window's precomputed geometry.
pub fn resolve_drop_target_in_geometry(
    cursor: Point,
    geometry: &WindowDropGeometry,
    docks: &Docks,
    drag: Option<&DragState>,
) -> DropTarget {
    compute_drop_target(
        cursor,
        geometry.grid_bounds,
        &geometry.pane_bounds,
        &geometry.dock_rails,
        &geometry.dock_bodies,
        docks,
        drag,
    )
}

/// Pick the window under `cursor` and resolve its drop target.
///
/// `cursor` is in the matching window's client coordinates. Each entry's
/// geometry is tested against `(0, 0, window_size.width, window_size.height)`.
/// When no window contains the cursor, returns `(None, DropTarget::None)`.
///
/// Production routing resolves against the event window only (see
/// [`crate::workspace::workspace_state::Workspace::resolve_drop_at`]); this helper exists
/// for pure geometry tests with synthetic window layouts.
pub fn pick_cross_window_drop_target(
    cursor: Point,
    hit_windows: &[(window::Id, WindowDropGeometry, Docks)],
    drag: Option<&DragState>,
) -> (Option<window::Id>, DropTarget) {
    for &(window_id, ref geometry, ref docks) in hit_windows {
        let window_rect = Rectangle::new(Point::ORIGIN, geometry.window_size);
        if window_rect.contains(cursor) {
            let target = resolve_drop_target_in_geometry(cursor, geometry, docks, drag);
            return (Some(window_id), target);
        }
    }

    (None, DropTarget::None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_pane_state::PaneState;

    fn single_pane_grid() -> (pane_grid::State<PaneState>, pane_grid::Pane) {
        pane_grid::State::new(PaneState::empty())
    }

    fn grid_bounds_800x500() -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 500.0,
        }
    }

    #[test]
    fn single_pane_computes_one_bounds_entry() {
        let (panes, _first) = single_pane_grid();
        let bounds = compute_pane_bounds(&panes, grid_bounds_800x500());
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0].bounds.width, 800.0);
        assert_eq!(bounds[0].bounds.height, 500.0);
        assert!((bounds[0].tab_strip.height - tab_strip_height()).abs() < 0.5);
    }

    #[test]
    fn two_panes_vertical_split_accounts_for_spacing() {
        let (mut panes, first) = pane_grid::State::new(PaneState::empty());
        panes.split(pane_grid::Axis::Vertical, first, PaneState::empty());
        let bounds = compute_pane_bounds(&panes, grid_bounds_800x500());
        assert_eq!(bounds.len(), 2);
        let expected = (800.0 - PANE_GRID_SPACING) / 2.0;
        let widths: Vec<f32> = bounds.iter().map(|b| b.bounds.width).collect();
        assert!((widths[0] - expected).abs() < 1.0);
        assert!((widths[1] - expected).abs() < 1.0);
    }

    #[test]
    fn drop_target_none_outside_all_regions() {
        let (panes, _first) = single_pane_grid();
        let window = Size::new(1024.0, 768.0);
        let grid = compute_grid_bounds(&Docks::empty(), window);
        let pane_bounds = compute_pane_bounds(&panes, grid);
        let docks = Docks::empty();
        let (rails, bodies) = compute_dock_regions(&docks, window);

        let target = compute_drop_target(
            Point::new(400.0, 10.0),
            grid,
            &pane_bounds,
            &rails,
            &bodies,
            &docks,
            None,
        );
        assert_eq!(target, DropTarget::None);
    }

    #[test]
    fn drop_target_tab_strip_center_of_pane() {
        let (panes, first) = single_pane_grid();
        let window = Size::new(1024.0, 768.0);
        let grid = compute_grid_bounds(&Docks::empty(), window);
        let pane_bounds = compute_pane_bounds(&panes, grid);
        let docks = Docks::empty();
        let (rails, bodies) = compute_dock_regions(&docks, window);

        let strip_h = tab_strip_height();
        let body_mid_y = grid.y + strip_h + (grid.height - strip_h) / 2.0;
        let target = compute_drop_target(
            Point::new(grid.x + grid.width / 2.0, body_mid_y),
            grid,
            &pane_bounds,
            &rails,
            &bodies,
            &docks,
            None,
        );
        assert_eq!(
            target,
            DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(first),
                index: 0,
            })
        );
    }

    #[test]
    fn drop_target_split_left_edge() {
        let (panes, first) = single_pane_grid();
        let window = Size::new(1024.0, 768.0);
        let grid = compute_grid_bounds(&Docks::empty(), window);
        let pane_bounds = compute_pane_bounds(&panes, grid);
        let docks = Docks::empty();
        let (rails, bodies) = compute_dock_regions(&docks, window);
        let pb = &pane_bounds[0];

        let target = compute_drop_target(
            Point::new(pb.bounds.x + 10.0, pb.bounds.y + tab_strip_height() + 40.0),
            grid,
            &pane_bounds,
            &rails,
            &bodies,
            &docks,
            None,
        );
        assert_eq!(
            target,
            DropTarget::SplitPane(SplitPaneTarget {
                pane: first,
                direction: Direction::Left,
            })
        );
    }

    #[test]
    fn tab_strip_wins_over_split_with_collapsed_right_dock() {
        use crate::features::dummies::text::TextPanel;

        let (mut panes, first) = pane_grid::State::new(PaneState::empty());
        let second = panes
            .split(pane_grid::Axis::Vertical, first, PaneState::empty())
            .unwrap()
            .0;
        let docks = Docks::new(
            PaneState::empty(),
            PaneState::new(vec![Box::new(TextPanel::new())]),
            PaneState::empty(),
        );

        let window = Size::new(1100.0, 760.0);
        let grid = compute_grid_bounds(&docks, window);
        let pane_bounds = compute_pane_bounds(&panes, grid);
        let (rails, bodies) = compute_dock_regions(&docks, window);

        let pb = pane_bounds
            .iter()
            .find(|pb| pb.pane == second)
            .expect("second pane bounds");
        let cursor = Point::new(
            pb.tab_strip.x + pb.tab_strip.width - 12.0,
            pb.tab_strip.y + pb.tab_strip.height / 2.0,
        );

        let target = compute_drop_target(cursor, grid, &pane_bounds, &rails, &bodies, &docks, None);
        assert!(
            matches!(target, DropTarget::TabStrip(_)),
            "expected tab strip, got {target:?}"
        );
    }

    #[test]
    fn split_preview_covers_half_pane() {
        let (panes, first) = single_pane_grid();
        let pane_bounds = compute_pane_bounds(&panes, grid_bounds_800x500());
        let pb = &pane_bounds[0];
        let preview = split_preview_rect(pb, Direction::Right);
        let body_h = pb.bounds.height - tab_strip_height();
        assert!((preview.width - pb.bounds.width / 2.0).abs() < 0.5);
        assert!((preview.height - body_h).abs() < 0.5);
        assert!((preview.x - (pb.bounds.x + pb.bounds.width / 2.0)).abs() < 0.5);
        let _ = first;
    }

    #[test]
    fn tab_insert_marker_is_narrow() {
        let (_, pane) = single_pane_grid();
        let strip = Rectangle::new(Point::new(100.0, 40.0), Size::new(300.0, 30.0));
        let pb = PaneBounds {
            pane,
            bounds: strip,
            tab_strip: strip,
            tab_count: 3,
        };
        let docks = Docks::empty();
        let marker = tab_insert_marker(
            TabStripTarget {
                location: PanelLocation::Center(pane),
                index: 1,
            },
            &[pb],
            &[],
            &docks,
            None,
        )
        .unwrap();
        assert!((marker.width - TAB_INSERT_MARKER_WIDTH).abs() < 0.1);
        assert!((marker.height - strip.height).abs() < 0.1);
        let expected_x = tab_insert_marker_x(strip, 3, 1, None) - TAB_INSERT_MARKER_WIDTH / 2.0;
        assert!((marker.x - expected_x).abs() < 0.1);
    }

    #[test]
    fn tab_insert_index_uses_left_packed_tab_widths() {
        let strip = Rectangle::new(Point::new(50.0, 10.0), Size::new(600.0, 28.0));
        let before_second = tab_insert_marker_x(strip, 2, 1, None) - 2.0;
        let after_second = tab_insert_marker_x(strip, 2, 2, None) - 2.0;
        assert_eq!(tab_insert_index(before_second, strip, 2, None), 1);
        assert_eq!(tab_insert_index(after_second, strip, 2, None), 2);
        assert_eq!(
            tab_insert_index(strip.x + tab_strip_padding() + 1.0, strip, 2, None),
            0
        );
    }

    fn compact_window_geometry() -> (pane_grid::Pane, WindowDropGeometry) {
        let (panes, pane) = single_pane_grid();
        let window_size = Size::new(400.0, 300.0);
        let docks = Docks::empty();
        let grid = compute_grid_bounds(&docks, window_size);
        let pane_bounds = compute_pane_bounds(&panes, grid);
        let (rails, bodies) = compute_dock_regions(&docks, window_size);
        (
            pane,
            WindowDropGeometry {
                window_size,
                grid_bounds: grid,
                pane_bounds,
                dock_rails: rails.to_vec(),
                dock_bodies: bodies.to_vec(),
            },
        )
    }

    fn docked_window_geometry() -> (pane_grid::Pane, WindowDropGeometry) {
        use crate::features::dummies::text::TextPanel;

        let (panes, pane) = single_pane_grid();
        let mut docks = Docks::new(
            PaneState::new(vec![Box::new(TextPanel::new())]),
            PaneState::empty(),
            PaneState::empty(),
        );
        docks.left.open = true;
        let window_size = Size::new(800.0, 600.0);
        let grid = compute_grid_bounds(&docks, window_size);
        let pane_bounds = compute_pane_bounds(&panes, grid);
        let (rails, bodies) = compute_dock_regions(&docks, window_size);
        (
            pane,
            WindowDropGeometry {
                window_size,
                grid_bounds: grid,
                pane_bounds,
                dock_rails: rails.to_vec(),
                dock_bodies: bodies.to_vec(),
            },
        )
    }

    #[test]
    fn pick_cross_window_resolves_second_window_tab_strip() {
        let id_a = window::Id::unique();
        let id_b = window::Id::unique();
        let (pane_a, geom_a) = compact_window_geometry();
        let (pane_b, geom_b) = docked_window_geometry();
        let pb = geom_b
            .pane_bounds
            .iter()
            .find(|pb| pb.pane == pane_b)
            .expect("pane bounds");
        // Outside the compact 400px-wide window, inside the docked window tab strip.
        let cursor = Point::new(
            pb.tab_strip.x + pb.tab_strip.width / 2.0,
            pb.tab_strip.y + pb.tab_strip.height / 2.0,
        );
        let docks_a = Docks::empty();
        let mut docks_b = Docks::new(
            PaneState::new(vec![Box::new(
                crate::features::dummies::text::TextPanel::new(),
            )]),
            PaneState::empty(),
            PaneState::empty(),
        );
        docks_b.left.open = true;
        let hit_windows = [(id_a, geom_a, docks_a), (id_b, geom_b, docks_b)];
        let (window_id, target) = pick_cross_window_drop_target(cursor, &hit_windows, None);

        assert_eq!(window_id, Some(id_b));
        assert_eq!(
            target,
            DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(pane_b),
                index: 0,
            })
        );
        let _ = pane_a;
    }

    #[test]
    fn pick_cross_window_resolves_docked_tab_strip() {
        let id_b = window::Id::unique();
        let (_pane_b, geom_b) = docked_window_geometry();
        let (_, left_body) = geom_b
            .dock_bodies
            .iter()
            .find(|(side, _)| *side == DockSide::Left)
            .expect("left dock body");
        let cursor = Point::new(left_body.x + 12.0, left_body.y + tab_strip_height() / 2.0);

        let mut docks_b = Docks::new(
            PaneState::new(vec![Box::new(
                crate::features::dummies::text::TextPanel::new(),
            )]),
            PaneState::empty(),
            PaneState::empty(),
        );
        docks_b.left.open = true;
        let (window_id, target) =
            pick_cross_window_drop_target(cursor, &[(id_b, geom_b, docks_b)], None);

        assert_eq!(window_id, Some(id_b));
        assert!(matches!(
            target,
            DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Dock(DockSide::Left),
                ..
            })
        ));
    }

    #[test]
    fn pick_cross_window_outside_all_windows_returns_none() {
        let id_a = window::Id::unique();
        let id_b = window::Id::unique();
        let (_, geom_a) = compact_window_geometry();
        let (_, geom_b) = docked_window_geometry();
        let hit_windows = [
            (id_a, geom_a, Docks::empty()),
            (id_b, geom_b, Docks::empty()),
        ];

        let (window_id, target) =
            pick_cross_window_drop_target(Point::new(2000.0, 2000.0), &hit_windows, None);

        assert_eq!(window_id, None);
        assert_eq!(target, DropTarget::None);
    }

    #[test]
    fn tab_insert_marker_omits_dragged_tab_slot() {
        let strip = Rectangle::new(Point::new(50.0, 10.0), Size::new(600.0, 28.0));
        // Dragging tab 0: insert before tab 1 lines up with the lone visible chip.
        let dragging_first = tab_insert_marker_x(strip, 2, 1, Some(0));
        let before_first = tab_insert_marker_x(strip, 2, 0, None);
        assert_eq!(dragging_first, before_first);
        // Dragging tab 1: end insert lines up after the lone visible chip.
        let dragging_second = tab_insert_marker_x(strip, 2, 2, Some(1));
        let after_first = tab_insert_marker_x(strip, 2, 1, None);
        assert_eq!(dragging_second, after_first);
    }
}
