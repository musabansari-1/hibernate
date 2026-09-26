//! Evidence describes reconstruction, never authorizes a side effect. In particular,
//! serialized capability flags are not consumed by the close/launch coordinators.
use crate::app_windows::Window;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PackageIdentity {
    Unpackaged,
    Packaged {
        name: String,
    },
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RestartRegistration {
    Absent,
    Registered {
        characters: u32,
        flags: u32,
    },
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeEvidence {
    pub package: PackageIdentity,
    pub restart: RestartRegistration,
    pub owner_hwnd: Option<String>,
    pub parent_hwnd: Option<String>,
    pub show_state: Option<u32>,
    pub normal_rect: Option<[i32; 4]>,
    #[serde(default)]
    pub geometry_space: Option<GeometrySpace>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeometrySpace {
    pub dpi_context: i32,
    pub window_dpi: u32,
    pub primary_size: [i32; 2],
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", content = "reason", rename_all = "kebab-case")]
pub enum Recipe {
    Unchecked,
    Available,
    Blocked(String),
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub version: u32,
    pub disposition: &'static str,
    pub summary: String,
    pub discovered: bool,
    pub process_identity_proven: bool,
    pub identity_proven: bool,
    pub relaunch_recipe_available: bool,
    pub window_placement_restorable: bool,
    pub window_placement_request_available: bool,
    pub document_context_restorable: bool,
    pub unsaved_state_restorable: bool,
    pub safe_close_allowed: bool,
    pub restore_verifiable: bool,
    pub verification_scope: &'static str,
    pub reconstruction_level: &'static str,
    pub command_line_restorable: bool,
    pub working_directory_restorable: bool,
    pub document_evidence: &'static str,
    pub semantic_adapter_required: bool,
    pub recipe: Recipe,
    pub blockers: Vec<&'static str>,
}

/// Minimum generic preservation level is application presence only. It cannot
/// satisfy Sleep's required working-state coverage, even for an apparently empty app.
pub fn assess(w: &Window, recipe: Recipe) -> Assessment {
    let process = w.owner.as_ref().is_some_and(|p| {
        p.pid != 0 && p.pid == w.owner_pid && p.process_started.parse::<u64>().is_ok_and(|v| v > 0)
    });
    let available = process
        && crate::app_context::launch_policy(w).is_ok()
        && matches!(recipe, Recipe::Available);
    let mut blockers = vec![
        "window-lifetime-unproven",
        "working-state-unknown",
        "conditional-close-contract-missing",
    ];
    if !process {
        blockers.push("process-identity-unavailable");
    }
    if w.elevated != Some(false) {
        blockers.push("security-context-unavailable-or-elevated");
    }
    if w.same_owner_windows != 1 {
        blockers.push("shared-or-unknown-window-ownership");
    }
    if !matches!(w.evidence.package, PackageIdentity::Unpackaged) {
        blockers.push("package-activation-contract-required");
    }
    if matches!(w.evidence.restart, RestartRegistration::Registered { .. }) {
        blockers.push("restart-registration-is-not-state-coverage");
    }
    let (disposition, summary) = match &recipe {
        Recipe::Available if available => ("restore-only", "Restore only — application launch is available; documents and current work cannot be proven. Keep open during Sleep.".into()),
        Recipe::Blocked(reason) => ("keep-open", format!("Keep open — {reason}")),
        _ => ("keep-open", "Keep open — reconstruction evidence has not been validated. Save context to check launch availability.".into()),
    };
    Assessment {
        version: 1,
        disposition,
        summary,
        discovered: true,
        process_identity_proven: process,
        identity_proven: false,
        relaunch_recipe_available: available,
        window_placement_restorable: false,
        window_placement_request_available: available && placement_request(w).is_some(),
        document_context_restorable: false,
        unsaved_state_restorable: false,
        safe_close_allowed: false,
        restore_verifiable: available,
        verification_scope: "new-process-window-presence-only",
        reconstruction_level: if available {
            "application-launch"
        } else {
            "observation-only"
        },
        command_line_restorable: false,
        working_directory_restorable: false,
        document_evidence: "unknown-no-validated-document-contract",
        semantic_adapter_required: true,
        recipe,
        blockers,
    }
}

/// Only screen coordinates of a normal, unowned, sole top-level window. Never
/// feed WINDOWPLACEMENT workspace coordinates to STARTUPINFO screen coordinates.
pub fn placement_request(w: &Window) -> Option<[i32; 4]> {
    let geometry = w.evidence.geometry_space.as_ref()?;
    let [left, top, right, bottom] = w.placement;
    (matches!(geometry.dpi_context, -5..=-1)
        && geometry.window_dpi > 0
        && right <= geometry.primary_size[0]
        && bottom <= geometry.primary_size[1]
        && w.same_owner_windows == 1
        && w.evidence.show_state == Some(1)
        && w.evidence.owner_hwnd.as_deref() == Some("0")
        && w.evidence.parent_hwnd.as_deref() == Some("0")
        && left >= 0
        && top >= 0
        && right > left
        && bottom > top
        && right <= 32767
        && bottom <= 32767)
        .then_some(w.placement)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_fifteen_fixture_classes_need_a_state_and_close_contract() {
        for case in [
            "stateless",
            "arguments",
            "cwd",
            "multi-window",
            "shared-process",
            "separate-processes",
            "saved-file",
            "unsaved-document",
            "packaged",
            "process-restart",
            "stale-hwnd",
            "pid-reuse",
            "inaccessible",
            "changed-image",
            "ambiguous",
        ] {
            let mut w = crate::app_context::tests::window();
            w.same_owner_windows = 1;
            if case == "inaccessible" {
                w.owner = None;
                w.elevated = None;
            }
            if case == "packaged" {
                w.evidence.package = PackageIdentity::Packaged {
                    name: "fixture".into(),
                };
            }
            let recipe = if matches!(case, "inaccessible" | "changed-image" | "packaged") {
                Recipe::Blocked(case.into())
            } else {
                Recipe::Available
            };
            let decision = assess(&w, recipe);
            assert!(!decision.safe_close_allowed, "{case}");
            assert!(!decision.identity_proven, "{case}");
            assert!(!decision.document_context_restorable, "{case}");
            assert!(!decision.unsaved_state_restorable, "{case}");
            assert!(decision.semantic_adapter_required, "{case}");
        }
    }
    #[test]
    fn geometry_is_a_request_not_a_claim_and_requires_single_normal_window() {
        let mut w = crate::app_context::tests::window();
        w.same_owner_windows = 1;
        w.evidence.show_state = Some(1);
        w.evidence.owner_hwnd = Some("0".into());
        w.evidence.parent_hwnd = Some("0".into());
        w.evidence.geometry_space = Some(GeometrySpace {
            dpi_context: -1,
            window_dpi: 96,
            primary_size: [1920, 1080],
        });
        assert!(assess(&w, Recipe::Available).window_placement_request_available);
        assert!(!assess(&w, Recipe::Available).window_placement_restorable);
        for state in [None, Some(2), Some(3)] {
            w.evidence.show_state = state;
            assert!(placement_request(&w).is_none());
        }
        w.evidence.show_state = Some(1);
        w.same_owner_windows = 2;
        assert!(placement_request(&w).is_none());
    }
}
