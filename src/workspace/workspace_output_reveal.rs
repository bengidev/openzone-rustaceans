//! Output Dock reveal seam — passive badge versus user-invoked open.
//!
//! Passive output marks the status-bar Output control without changing
//! dock visibility or workspace focus. User-invoked output opens the
//! Output Dock through the same visibility and composition-root factory
//! path as [`crate::workspace::workspace_command::Command::OpenDock`],
//! focusing the dock only when its active panel is interactive.

/// How output should surface in the Output Dock chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputRevealKind {
    /// Mark the Output Dock control without opening it or stealing focus.
    Passive,
    /// Open the Output Dock so command results or diagnostics are visible.
    UserInvoked,
}

/// Status-bar label for a dock control, including visibility affordance
/// and an optional passive-output badge.
pub fn output_dock_control_label(
    label: &str,
    visibility: crate::workspace::workspace_dock::DockVisibility,
    badged: bool,
) -> String {
    let base = match visibility {
        crate::workspace::workspace_dock::DockVisibility::Open => format!("▾ {label}"),
        crate::workspace::workspace_dock::DockVisibility::Collapsed => format!("▸ {label}"),
        crate::workspace::workspace_dock::DockVisibility::Hidden => label.to_string(),
    };
    if badged {
        format!("● {base}")
    } else {
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_dock::DockVisibility;

    #[test]
    fn passive_badge_prefixes_label_in_every_visibility_state() {
        for visibility in [
            DockVisibility::Hidden,
            DockVisibility::Collapsed,
            DockVisibility::Open,
        ] {
            let label = output_dock_control_label("Output", visibility, true);
            assert!(label.starts_with("● "));
        }
    }

    #[test]
    fn unbadged_label_matches_visibility_affordance() {
        assert_eq!(
            output_dock_control_label("Output", DockVisibility::Hidden, false),
            "Output"
        );
        assert_eq!(
            output_dock_control_label("Output", DockVisibility::Collapsed, false),
            "▸ Output"
        );
        assert_eq!(
            output_dock_control_label("Output", DockVisibility::Open, false),
            "▾ Output"
        );
    }
}
