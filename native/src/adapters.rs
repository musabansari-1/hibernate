//! Capability registry. Discovery never grants close or semantic-restore authority.
use serde_json::{json, Value};

pub const GENERIC: &str = "windows-generic";
pub const NATIVE_CLIENT: &str = "windows-discovery";

pub struct Descriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub reviewed_close: bool,
    pub semantic_restore: bool,
    pub verification: &'static str,
}

pub const REGISTRY: &[Descriptor] = &[
    Descriptor {
        id: GENERIC,
        label: "Windows app context",
        reviewed_close: false,
        semantic_restore: false,
        verification: "application-presence-only",
    },
    Descriptor {
        id: "chrome",
        label: "Chrome",
        reviewed_close: true,
        semantic_restore: true,
        verification: "tab-organization",
    },
    Descriptor {
        id: "vscode",
        label: "VS Code",
        reviewed_close: true,
        semantic_restore: true,
        verification: "saved-local-editor-context",
    },
    Descriptor {
        id: "context",
        label: "Folders and terminal context",
        reviewed_close: false,
        semantic_restore: true,
        verification: "folder-or-instrumented-cwd",
    },
];

pub fn descriptor(id: &str) -> Option<&'static Descriptor> {
    REGISTRY.iter().find(|d| d.id == id)
}

pub fn catalog() -> Value {
    json!(REGISTRY
        .iter()
        .map(|d| json!({
            "id":d.id,"label":d.label,"version":1,
            "discovery":true,"contextCapture":true,"reviewedClose":d.reviewed_close,
            "relaunch":"requires-live-policy-check","semanticRestore":d.semantic_restore,
            "verification":d.verification,"capabilitiesAreAuthority":false,
            "sleepRequires":["exact-live-instance","working-state-reconstruction-contract","conditional-close-contract"],
            "reconstructionAssessmentVersion":1
        }))
        .collect::<Vec<_>>())
}

/// Names suggest an enhancement only; they never bind an integration to an HWND.
pub fn enhancement(executable: &str) -> Option<&'static str> {
    match executable
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "chrome.exe" => Some("chrome"),
        "code.exe" => Some("vscode"),
        "explorer.exe" | "windowsterminal.exe" | "wt.exe" => Some("context"),
        _ => None,
    }
}

pub fn validate_snapshot(adapter: &str, body: &Value) -> Result<(), String> {
    if descriptor(adapter).is_none() || body["adapter"] != adapter || body["version"] != 1 {
        return Err("Unsupported or mismatched adapter schema".into());
    }
    if adapter == GENERIC {
        crate::app_context::validate(body)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_names_never_acquire_semantic_authority() {
        assert!(descriptor("future-app").is_none());
        assert!(!descriptor(GENERIC).unwrap().reviewed_close);
        assert!(!descriptor(GENERIC).unwrap().semantic_restore);
        assert_eq!(enhancement("C:\\Apps\\Code.exe"), Some("vscode"));
        assert_eq!(enhancement("C:\\Apps\\CodeHelper.exe"), None);
    }
}
