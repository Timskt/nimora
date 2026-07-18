use nimora_diagnostics_bundle::{
    ApplicationSummary, DataProtectionSummary, DiagnosticReport, DiagnosticSourcesSummary,
    PrivacySummary, RuntimeSummary, SystemSummary,
};

pub(crate) const DIAGNOSTIC_REPORT_SPEC: &str = "nimora.diagnostic-report/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticStartupMode {
    Normal,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticSafetyMode {
    Normal,
    Safe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticReportFacts {
    pub generated_at_ms: u64,
    pub application_version: String,
    pub operating_system: String,
    pub architecture: String,
    pub startup_mode: DiagnosticStartupMode,
    pub startup_reason: Option<String>,
    pub safety_mode: DiagnosticSafetyMode,
    pub outbox_pending: u64,
    pub outbox_dead_letter: u64,
    pub database_schema: u32,
    pub backup_count: u64,
    pub latest_backup_at_ms: Option<u64>,
    pub pending_restore: bool,
    pub last_backup_error: bool,
    pub event_count: u64,
    pub event_retention_days: u64,
}

pub(crate) fn build_diagnostic_report(facts: DiagnosticReportFacts) -> DiagnosticReport {
    DiagnosticReport {
        spec: DIAGNOSTIC_REPORT_SPEC.to_owned(),
        generated_at_ms: facts.generated_at_ms,
        application: ApplicationSummary {
            name: "Nimora".to_owned(),
            version: facts.application_version,
        },
        system: SystemSummary {
            os: facts.operating_system,
            architecture: facts.architecture,
        },
        runtime: RuntimeSummary {
            startup_mode: match facts.startup_mode {
                DiagnosticStartupMode::Normal => "normal",
                DiagnosticStartupMode::Recovery => "recovery",
            }
            .to_owned(),
            startup_reason: facts.startup_reason,
            safety_mode: match facts.safety_mode {
                DiagnosticSafetyMode::Normal => "normal",
                DiagnosticSafetyMode::Safe => "safe",
            }
            .to_owned(),
            outbox_pending: facts.outbox_pending,
            outbox_dead_letter: facts.outbox_dead_letter,
        },
        data_protection: DataProtectionSummary {
            database_schema: facts.database_schema,
            backup_count: facts.backup_count,
            latest_backup_at_ms: facts.latest_backup_at_ms,
            pending_restore: facts.pending_restore,
            last_backup_error: facts.last_backup_error,
        },
        sources: DiagnosticSourcesSummary {
            event_count: facts.event_count,
            event_retention_days: facts.event_retention_days,
        },
        privacy: PrivacySummary {
            includes_logs: false,
            includes_user_content: false,
            includes_secrets: false,
            includes_file_paths: false,
            automatically_uploaded: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> DiagnosticReportFacts {
        DiagnosticReportFacts {
            generated_at_ms: 1_784_294_125_392,
            application_version: "0.1.0".to_owned(),
            operating_system: "macos".to_owned(),
            architecture: "aarch64".to_owned(),
            startup_mode: DiagnosticStartupMode::Recovery,
            startup_reason: Some("database-unavailable".to_owned()),
            safety_mode: DiagnosticSafetyMode::Safe,
            outbox_pending: 3,
            outbox_dead_letter: 1,
            database_schema: 7,
            backup_count: 2,
            latest_backup_at_ms: Some(1_784_294_000_000),
            pending_restore: true,
            last_backup_error: true,
            event_count: 4,
            event_retention_days: 14,
        }
    }

    #[test]
    fn builds_versioned_report_from_normalized_facts() {
        let report = build_diagnostic_report(facts());
        assert_eq!(report.spec, DIAGNOSTIC_REPORT_SPEC);
        assert_eq!(report.application.name, "Nimora");
        assert_eq!(report.runtime.startup_mode, "recovery");
        assert_eq!(report.runtime.safety_mode, "safe");
        assert_eq!(report.runtime.outbox_pending, 3);
        assert_eq!(report.data_protection.database_schema, 7);
        assert_eq!(report.sources.event_retention_days, 14);
    }

    #[test]
    fn privacy_contract_is_fail_closed_and_never_claims_upload() {
        let report = build_diagnostic_report(facts());
        assert!(!report.privacy.includes_logs);
        assert!(!report.privacy.includes_user_content);
        assert!(!report.privacy.includes_secrets);
        assert!(!report.privacy.includes_file_paths);
        assert!(!report.privacy.automatically_uploaded);
    }

    #[test]
    fn maps_normal_runtime_modes_without_host_types() {
        let mut input = facts();
        input.startup_mode = DiagnosticStartupMode::Normal;
        input.startup_reason = None;
        input.safety_mode = DiagnosticSafetyMode::Normal;
        let report = build_diagnostic_report(input);
        assert_eq!(report.runtime.startup_mode, "normal");
        assert_eq!(report.runtime.safety_mode, "normal");
        assert!(report.runtime.startup_reason.is_none());
    }
}
