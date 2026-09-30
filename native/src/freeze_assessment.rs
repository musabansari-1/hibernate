//! Evidence model for freeze policy. Descriptive only: never grants native authority.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SleepStrategy {
    VerifiedReconstruction,
    GenericDeepFreeze,
    RestoreOnly,
    Blocked,
}

#[derive(Clone, Debug, Default)]
pub struct Evidence {
    pub process_identity_proven: bool,
    pub process_group_proven: bool,
    pub elevated: Option<bool>,
    pub system_critical: Option<bool>,
    pub same_interactive_user: Option<bool>,
    pub protected_process: Option<bool>,
    pub packaged: Option<bool>,
    pub in_job: Option<bool>,
    // An exact, independently validated single-process app contract, never
    // inferred from job membership or granted by the frontend.
    pub job_independent_contract: bool,
    pub shared_with_unselected_windows: bool,
    pub active_external_work_risk: Option<bool>,
    pub suspend_available: bool,
    pub memory_trim_available: bool,
    pub excluded_role: bool,
    // Positive app-role/ownership evidence is mandatory; lack of a denylist match
    // is not evidence. No public API accepts these fields as execution authority.
    pub validated_non_security_app_contract: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FreezeAssessment {
    pub version: u32,
    pub process_identity_proven: bool,
    pub process_group_proven: bool,
    pub elevated: Option<bool>,
    pub system_critical: Option<bool>,
    pub in_job: Option<bool>,
    pub shared_with_unselected_windows: bool,
    pub active_external_work_risk: Option<bool>,
    pub suspend_available: bool,
    pub memory_trim_available: bool,
    pub safe_to_freeze: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}
pub fn assess(e: &Evidence) -> FreezeAssessment {
    let mut blockers = Vec::new();
    for (ok, reason) in [
        (
            e.process_identity_proven,
            "Exact process identity is unavailable or changed",
        ),
        (
            e.process_group_proven,
            "Exclusive application process-group ownership is unproven",
        ),
        (
            e.elevated == Some(false),
            "Elevated or unknown process authority",
        ),
        (
            e.system_critical == Some(false),
            "Critical-process status is unsafe or unknown",
        ),
        (
            e.same_interactive_user == Some(true),
            "Same interactive user and session are unproven",
        ),
        (
            e.protected_process == Some(false),
            "Protected-process status is unsafe or unknown",
        ),
        (
            e.packaged == Some(false),
            "Packaged or unknown application identity",
        ),
        (
            e.in_job == Some(false) || (e.in_job == Some(true) && e.job_independent_contract),
            "Unknown job membership or missing independent-process contract",
        ),
        (
            !e.shared_with_unselected_windows,
            "Unselected or hidden windows share this process",
        ),
        (
            e.active_external_work_risk != Some(true),
            "Active external work must finish before freeze",
        ),
        (
            e.suspend_available,
            "Owned process-state mechanism or required rights unavailable",
        ),
        (
            e.memory_trim_available,
            "Working-set trimming rights unavailable",
        ),
        (
            !e.excluded_role,
            "Self, shell, system, service, runtime or security role excluded",
        ),
        (
            e.validated_non_security_app_contract,
            "Positive non-security app and ownership contract is not validated",
        ),
    ] {
        if !ok {
            blockers.push(reason.to_string());
        }
    }
    FreezeAssessment {
        version: 1, process_identity_proven:e.process_identity_proven, process_group_proven:e.process_group_proven,
        elevated:e.elevated, system_critical:e.system_critical, in_job:e.in_job, shared_with_unselected_windows:e.shared_with_unselected_windows,
        active_external_work_risk:e.active_external_work_risk, suspend_available:e.suspend_available,
        memory_trim_available:e.memory_trim_available, safe_to_freeze:blockers.is_empty(), blockers,
        warnings:vec!["Local process preservation cannot preserve remote leases, network/audio/device sessions or guarantee external work. Windows remain visible and unresponsive until Wake. Committed memory remains allocated.".into()],
    }
}
pub fn choose(
    verified_adapter_available: bool,
    assessment: &FreezeAssessment,
    restore_available: bool,
    hard_excluded: bool,
) -> SleepStrategy {
    if hard_excluded {
        return SleepStrategy::Blocked;
    }
    if verified_adapter_available {
        return SleepStrategy::VerifiedReconstruction;
    }
    if assessment.safe_to_freeze {
        return SleepStrategy::GenericDeepFreeze;
    }
    if restore_available {
        SleepStrategy::RestoreOnly
    } else {
        SleepStrategy::Blocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Evidence {
        Evidence {
            process_identity_proven: true,
            process_group_proven: true,
            elevated: Some(false),
            system_critical: Some(false),
            same_interactive_user: Some(true),
            protected_process: Some(false),
            packaged: Some(false),
            in_job: Some(false),
            job_independent_contract: false,
            shared_with_unselected_windows: false,
            active_external_work_risk: Some(false),
            suspend_available: true,
            memory_trim_available: true,
            excluded_role: false,
            validated_non_security_app_contract: true,
        }
    }
    #[test]
    fn unknown_evidence_is_not_permission() {
        assert!(!assess(&Evidence::default()).safe_to_freeze);
        let mut e = fixture();
        e.validated_non_security_app_contract = false;
        assert!(
            !assess(&e).safe_to_freeze,
            "Absence from a denylist is insufficient"
        );
        assert_eq!(
            choose(false, &assess(&e), true, false),
            SleepStrategy::RestoreOnly
        );
    }
    #[test]
    fn independent_identity_security_topology_and_activity_blockers() {
        let base = fixture();
        assert!(assess(&base).safe_to_freeze);
        let variants: Vec<fn(&mut Evidence)> = vec![
            |e| e.process_identity_proven = false,
            |e| e.process_group_proven = false,
            |e| e.elevated = Some(true),
            |e| e.elevated = None,
            |e| e.system_critical = Some(true),
            |e| e.system_critical = None,
            |e| e.same_interactive_user = Some(false),
            |e| e.protected_process = None,
            |e| e.packaged = Some(true),
            |e| e.in_job = Some(true),
            |e| e.shared_with_unselected_windows = true,
            |e| e.active_external_work_risk = Some(true),
            |e| e.suspend_available = false,
            |e| e.memory_trim_available = false,
            |e| e.excluded_role = true,
        ];
        for change in variants {
            let mut e = base.clone();
            change(&mut e);
            assert!(!assess(&e).safe_to_freeze);
        }
    }
    #[test]
    fn semantic_adapter_priority_and_hard_exclusion() {
        let a = assess(&fixture());
        assert_eq!(
            choose(true, &a, true, false),
            SleepStrategy::VerifiedReconstruction
        );
        assert_eq!(
            choose(false, &a, true, false),
            SleepStrategy::GenericDeepFreeze
        );
        assert_eq!(choose(true, &a, true, true), SleepStrategy::Blocked);
    }
    #[test]
    fn job_membership_requires_explicit_independent_contract() {
        let mut e = fixture();
        e.in_job = Some(true);
        assert!(!assess(&e).safe_to_freeze);
        e.job_independent_contract = true;
        assert!(assess(&e).safe_to_freeze);
        e.in_job = None;
        assert!(!assess(&e).safe_to_freeze);
        e.in_job = Some(true);
        e.process_group_proven = false;
        assert!(!assess(&e).safe_to_freeze);
    }
}
