#![allow(dead_code)]

//! Tab drag-and-drop subsystem.
//!
//! Isolates bounds capture and hit testing for custom tab DnD on top of
//! the multi-window shell. Native pane_grid drag for whole-pane
//! rearrangement is untouched; this module handles tab-strip drag,
//! drop-into-strip, edge-split, dock drop, and cross-window tab move.
//!
//! ## Drop-target resolution
//!
//! Each cursor-move during a drag recomputes a [`DropTarget`] from the
//! captured pane bounds:
//!
//! ```text
//! TabStrip(loc, index) | SplitPane(pane, Direction) | Dock(side) | None
//! ```
//!
//! Edges use the outer quarter of a pane's bounds (central hit testing,
//! not widget-native boundaries). A center hit drops a tab into that
//! pane's tab strip. Dock rails and open dock bodies are checked after
//! pane edges.

use iced::widget::pane_grid;
use iced::{Point, Rectangle, Size};

use crate::workspace::dock::Docks;
use crate::workspace::location::{DockSide, PanelLocation};

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
}

/// Rails and body rectangles for all three dock sides.
pub type DockRegions = ([(DockSide, Rectangle); 3], [(DockSide, Rectangle); 3]);

const TAB_STRIP_HEIGHT: f32 = 34.0;
const DOCK_RAIL_THICKNESS: f32 = 28.0;
const SIDE_DOCK_WIDTH: f32 = 280.0;
const BOTTOM_DOCK_HEIGHT: f32 = 200.0;
const TITLE_BAR_HEIGHT: f32 = 36.0;
const STATUS_BAR_HEIGHT: f32 = 24.0;

/// The area of the window devoted to the center pane grid + docks.
fn workspace_area(window_size: Size) -> Rectangle {
    Rectangle {
        x: 0.0,
        y: TITLE_BAR_HEIGHT,
        width: window_size.width,
        height: (window_size.height - TITLE_BAR_HEIGHT - STATUS_BAR_HEIGHT).max(0.0),
    }
}

/// Compute per-pane pixel bounds by walking the pane grid's split tree.
pub fn compute_pane_bounds(
    panes: &pane_grid::State<crate::workspace::pane_state::PaneState>,
    grid_bounds: Rectangle,
) -> Vec<PaneBounds> {
    let mut out = Vec::new();
    let root = panes.layout();

    fn walk(node: &pane_grid::Node, region: Rectangle, out: &mut Vec<PaneBounds>) {
        match node {
            pane_grid::Node::Split {
                axis, ratio, a, b, ..
            } => {
                let (region_a, region_b) = match axis {
                    pane_grid::Axis::Horizontal => {
                        let split_at = region.y + region.height * ratio;
                        (
                            Rectangle {
                                y: region.y,
                                height: split_at - region.y,
                                ..region
                            },
                            Rectangle {
                                y: split_at,
                                height: region.height - (split_at - region.y),
                                ..region
                            },
                        )
                    }
                    pane_grid::Axis::Vertical => {
                        let split_at = region.x + region.width * ratio;
                        (
                            Rectangle {
                                x: region.x,
                                width: split_at - region.x,
                                ..region
                            },
                            Rectangle {
                                x: split_at,
                                width: region.width - (split_at - region.x),
                                ..region
                            },
                        )
                    }
                };
                walk(a, region_a, out);
                walk(b, region_b, out);
            }
            pane_grid::Node::Pane(pane) => {
                let tab_strip = Rectangle {
                    x: region.x,
                    y: region.y,
                    width: region.width,
                    height: TAB_STRIP_HEIGHT.min(region.height),
                };
                out.push(PaneBounds {
                    pane: *pane,
                    bounds: region,
                    tab_strip,
                });
            }
        }
    }

    walk(root, grid_bounds, &mut out);
    out
}

/// Compute dock positions (rails and open bodies) from window size and dock state.
pub fn compute_dock_regions(docks: &Docks, window_size: Size) -> DockRegions {
    let area = workspace_area(window_size);

    let left_width = if docks.left.open {
        SIDE_DOCK_WIDTH
    } else {
        0.0
    };
    let right_width = if docks.right.open {
        SIDE_DOCK_WIDTH
    } else {
        0.0
    };
    let bottom_height = if docks.bottom.open {
        BOTTOM_DOCK_HEIGHT
    } else {
        0.0
    };

    let center_height = (area.height - bottom_height).max(0.0);
    let center_width = (area.width - left_width - right_width).max(0.0);
    let center_x = area.x + left_width;

    let left_rail = if !docks.left.open && !docks.left.tabs.is_empty() {
        Rectangle {
            x: area.x,
            y: area.y,
            width: DOCK_RAIL_THICKNESS,
            height: center_height,
        }
    } else {
        Rectangle::default()
    };

    let right_rail = if !docks.right.open && !docks.right.tabs.is_empty() {
        Rectangle {
            x: area.x + area.width - DOCK_RAIL_THICKNESS,
            y: area.y,
            width: DOCK_RAIL_THICKNESS,
            height: center_height,
        }
    } else {
        Rectangle::default()
    };

    let bottom_rail = if !docks.bottom.open && !docks.bottom.tabs.is_empty() {
        Rectangle {
            x: center_x,
            y: area.y + center_height,
            width: center_width,
            height: DOCK_RAIL_THICKNESS,
        }
    } else {
        Rectangle::default()
    };

    let left_body = if docks.left.open {
        Rectangle {
            x: area.x,
            y: area.y,
            width: SIDE_DOCK_WIDTH,
            height: center_height,
        }
    } else {
        Rectangle::default()
    };

    let right_body = if docks.right.open {
        Rectangle {
            x: area.x + area.width - SIDE_DOCK_WIDTH,
            y: area.y,
            width: SIDE_DOCK_WIDTH,
            height: center_height,
        }
    } else {
        Rectangle::default()
    };

    let bottom_body = if docks.bottom.open {
        Rectangle {
            x: center_x,
            y: area.y + center_height,
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

/// Hit-test cursor position against all drop zones and resolve a [`DropTarget`].
///
/// Priority order:
/// 1. Tab strips
/// 2. Open dock tab strips
/// 3. Open dock bodies
/// 4. Dock rails (narrow explicit affordances)
/// 5. Pane edges for split targets (outer quarter, below tab strip)
/// 6. Center of a pane → drop as new tab
/// 7. None (tear-off candidate)
pub fn compute_drop_target(
    cursor: Point,
    pane_bounds: &[PaneBounds],
    dock_rails: &[(DockSide, Rectangle)],
    dock_bodies: &[(DockSide, Rectangle)],
) -> DropTarget {
    // 1. Tab strips of center panes
    for pb in pane_bounds {
        if pb.tab_strip.contains(cursor) {
            return DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(pb.pane),
                index: 0,
            });
        }
    }

    // 2. Dock tab strips
    for &(side, body) in dock_bodies {
        if body.width > 0.0 && body.height > 0.0 {
            let dock_strip = Rectangle {
                x: body.x,
                y: body.y,
                width: body.width,
                height: TAB_STRIP_HEIGHT.min(body.height),
            };
            if dock_strip.contains(cursor) {
                return DropTarget::TabStrip(TabStripTarget {
                    location: PanelLocation::Dock(side),
                    index: 0,
                });
            }
        }
    }

    // 3. Open dock bodies (content area)
    for &(side, body) in dock_bodies {
        if body.width > 0.0 && body.height > 0.0 {
            let content = Rectangle {
                x: body.x,
                y: body.y + TAB_STRIP_HEIGHT.min(body.height),
                width: body.width,
                height: (body.height - TAB_STRIP_HEIGHT.min(body.height)).max(0.0),
            };
            if content.contains(cursor) {
                return DropTarget::Dock(side);
            }
        }
    }

    // 4. Dock rails — checked before pane edges
    for &(side, rail) in dock_rails {
        if rail.width > 0.0 && rail.height > 0.0 && rail.contains(cursor) {
            return DropTarget::Dock(side);
        }
    }

    // 5. Pane edges for split targets, then center
    for pb in pane_bounds {
        if !pb.bounds.contains(cursor) {
            continue;
        }

        let quarter_w = pb.bounds.width / 4.0;
        let quarter_h = pb.bounds.height / 4.0;
        let rel_x = cursor.x - pb.bounds.x;
        let rel_y = cursor.y - pb.bounds.y;

        let below_strip = rel_y > TAB_STRIP_HEIGHT;

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
            if rel_y < quarter_h + TAB_STRIP_HEIGHT {
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
        }

        return DropTarget::TabStrip(TabStripTarget {
            location: PanelLocation::Center(pb.pane),
            index: 0,
        });
    }

    DropTarget::None
}

/// Pixel rectangle to highlight for a resolved [`DropTarget`].
pub fn preview_bounds(
    target: DropTarget,
    pane_bounds: &[PaneBounds],
    dock_rails: &[(DockSide, Rectangle)],
    dock_bodies: &[(DockSide, Rectangle)],
) -> Option<Rectangle> {
    match target {
        DropTarget::TabStrip(strip) => tab_strip_bounds(strip, pane_bounds, dock_bodies),
        DropTarget::SplitPane(split) => pane_bounds
            .iter()
            .find(|pb| pb.pane == split.pane)
            .map(|pb| split_zone_rect(pb, split.direction)),
        DropTarget::Dock(side) => dock_preview_bounds(side, dock_rails, dock_bodies),
        DropTarget::None => None,
    }
}

fn tab_strip_bounds(
    strip: TabStripTarget,
    pane_bounds: &[PaneBounds],
    dock_bodies: &[(DockSide, Rectangle)],
) -> Option<Rectangle> {
    match strip.location {
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
                    height: TAB_STRIP_HEIGHT.min(body.height),
                })
            } else {
                None
            }
        }),
    }
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

fn split_zone_rect(pb: &PaneBounds, direction: Direction) -> Rectangle {
    let quarter_w = pb.bounds.width / 4.0;
    let quarter_h = (pb.bounds.height - TAB_STRIP_HEIGHT).max(0.0) / 4.0;
    let body = Rectangle {
        x: pb.bounds.x,
        y: pb.bounds.y + TAB_STRIP_HEIGHT,
        width: pb.bounds.width,
        height: (pb.bounds.height - TAB_STRIP_HEIGHT).max(0.0),
    };

    match direction {
        Direction::Left => Rectangle {
            width: quarter_w,
            ..body
        },
        Direction::Right => Rectangle {
            x: body.x + body.width - quarter_w,
            width: quarter_w,
            y: body.y,
            height: body.height,
        },
        Direction::Up => Rectangle {
            height: quarter_h,
            ..body
        },
        Direction::Down => Rectangle {
            x: body.x,
            y: body.y + body.height - quarter_h,
            width: body.width,
            height: quarter_h,
        },
    }
}

/// Compute the grid area from window size and dock state.
pub fn compute_grid_bounds(docks: &Docks, window_size: Size) -> Rectangle {
    let area = workspace_area(window_size);
    let left_width = if docks.left.open {
        SIDE_DOCK_WIDTH
    } else {
        0.0
    };
    let right_width = if docks.right.open {
        SIDE_DOCK_WIDTH
    } else {
        0.0
    };
    let bottom_height = if docks.bottom.open {
        BOTTOM_DOCK_HEIGHT
    } else {
        0.0
    };

    Rectangle {
        x: area.x + left_width,
        y: area.y,
        width: (area.width - left_width - right_width).max(0.0),
        height: (area.height - bottom_height).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::pane_state::PaneState;

    fn single_pane_grid() -> (pane_grid::State<PaneState>, pane_grid::Pane) {
        pane_grid::State::new(PaneState::empty())
    }

    fn grid_bounds_800x600() -> Rectangle {
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
        let bounds = compute_pane_bounds(&panes, grid_bounds_800x600());
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0].bounds.width, 800.0);
        assert_eq!(bounds[0].bounds.height, 500.0);
        assert_eq!(bounds[0].tab_strip.height, TAB_STRIP_HEIGHT);
    }

    #[test]
    fn two_panes_vertical_split_halves_width() {
        let (mut panes, first) = pane_grid::State::new(PaneState::empty());
        panes.split(pane_grid::Axis::Vertical, first, PaneState::empty());
        let bounds = compute_pane_bounds(&panes, grid_bounds_800x600());
        assert_eq!(bounds.len(), 2);
        let widths: Vec<f32> = bounds.iter().map(|b| b.bounds.width).collect();
        assert!((widths[0] - 400.0).abs() < 1.0);
        assert!((widths[1] - 400.0).abs() < 1.0);
        assert_eq!(bounds[0].bounds.height, 500.0);
    }

    #[test]
    fn two_panes_horizontal_split_halves_height() {
        let (mut panes, first) = pane_grid::State::new(PaneState::empty());
        panes.split(pane_grid::Axis::Horizontal, first, PaneState::empty());
        let bounds = compute_pane_bounds(&panes, grid_bounds_800x600());
        assert_eq!(bounds.len(), 2);
        let heights: Vec<f32> = bounds.iter().map(|b| b.bounds.height).collect();
        assert!((heights[0] - 250.0).abs() < 1.0);
        assert!((heights[1] - 250.0).abs() < 1.0);
        assert_eq!(bounds[0].bounds.width, 800.0);
    }

    #[test]
    fn drop_target_none_outside_all_regions() {
        let (panes, _first) = single_pane_grid();
        let grid = compute_grid_bounds(&Docks::empty(), Size::new(1024.0, 768.0));
        let pane_bounds = compute_pane_bounds(&panes, grid);
        let docks = Docks::empty();
        let (rails, bodies) = compute_dock_regions(&docks, Size::new(1024.0, 768.0));

        let target = compute_drop_target(Point::new(400.0, 10.0), &pane_bounds, &rails, &bodies);
        assert_eq!(target, DropTarget::None);
    }

    #[test]
    fn drop_target_tab_strip_center_of_pane() {
        let (panes, first) = single_pane_grid();
        let pane_bounds = compute_pane_bounds(&panes, grid_bounds_800x600());
        let (rails, bodies) = compute_dock_regions(&Docks::empty(), Size::new(1024.0, 768.0));

        let target = compute_drop_target(Point::new(400.0, 200.0), &pane_bounds, &rails, &bodies);
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
        let pane_bounds = compute_pane_bounds(&panes, grid_bounds_800x600());
        let (rails, bodies) = compute_dock_regions(&Docks::empty(), Size::new(1024.0, 768.0));

        let target = compute_drop_target(Point::new(50.0, 200.0), &pane_bounds, &rails, &bodies);
        assert_eq!(
            target,
            DropTarget::SplitPane(SplitPaneTarget {
                pane: first,
                direction: Direction::Left,
            })
        );
    }

    #[test]
    fn drop_target_split_right_edge() {
        let (panes, first) = single_pane_grid();
        let pane_bounds = compute_pane_bounds(&panes, grid_bounds_800x600());
        let (rails, bodies) = compute_dock_regions(&Docks::empty(), Size::new(1024.0, 768.0));

        let target = compute_drop_target(Point::new(780.0, 200.0), &pane_bounds, &rails, &bodies);
        assert_eq!(
            target,
            DropTarget::SplitPane(SplitPaneTarget {
                pane: first,
                direction: Direction::Right,
            })
        );
    }

    #[test]
    fn drop_target_dock_body() {
        let (panes, _first) = single_pane_grid();
        let mut docks = Docks::empty();
        docks.right.open = true;
        let grid = compute_grid_bounds(&docks, Size::new(1024.0, 768.0));
        let pane_bounds = compute_pane_bounds(&panes, grid);
        let (rails, bodies) = compute_dock_regions(&docks, Size::new(1024.0, 768.0));

        let target = compute_drop_target(Point::new(900.0, 200.0), &pane_bounds, &rails, &bodies);
        assert_eq!(target, DropTarget::Dock(DockSide::Right));
    }

    #[test]
    fn synthetic_bounds_resolve_all_targets() {
        let (mut panes, first) = pane_grid::State::new(PaneState::empty());
        let second = panes
            .split(pane_grid::Axis::Vertical, first, PaneState::empty())
            .unwrap()
            .0;
        let pane_bounds = compute_pane_bounds(
            &panes,
            Rectangle {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 500.0,
            },
        );

        let t = compute_drop_target(Point::new(200.0, 200.0), &pane_bounds, &[], &[]);
        assert_eq!(
            t,
            DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(first),
                index: 0,
            })
        );

        let t = compute_drop_target(Point::new(350.0, 200.0), &pane_bounds, &[], &[]);
        assert_eq!(
            t,
            DropTarget::SplitPane(SplitPaneTarget {
                pane: first,
                direction: Direction::Right,
            })
        );

        let t = compute_drop_target(Point::new(600.0, 200.0), &pane_bounds, &[], &[]);
        assert_eq!(
            t,
            DropTarget::TabStrip(TabStripTarget {
                location: PanelLocation::Center(second),
                index: 0,
            })
        );

        let t = compute_drop_target(Point::new(420.0, 200.0), &pane_bounds, &[], &[]);
        assert_eq!(
            t,
            DropTarget::SplitPane(SplitPaneTarget {
                pane: second,
                direction: Direction::Left,
            })
        );
    }
}
