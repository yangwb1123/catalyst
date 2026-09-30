use super::*;

#[test]
fn remote_credential_candidate_preview_requires_one_bounded_input() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "credential-candidate",
            "preview",
            "--input",
            "candidate.json",
        ])
        .unwrap(),
        RemoteCommand::CredentialCandidatePreview {
            input: "candidate.json".into(),
        }
    );
    assert!(parse_remote_command(&["remote", "credential-candidate"]).is_err());
    assert!(parse_remote_command(&["remote", "credential-candidate", "preview",]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "credential-candidate",
            "preview",
            "--input",
            "candidate.json",
            "--input",
            "other.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "credential-candidate",
            "preview",
            "--input",
            "candidate.json",
            "--unknown",
        ])
        .is_err()
    );
}

#[test]
fn remote_execution_consent_preview_parses_as_an_exact_read_only_command() {
    assert_eq!(
        parse_remote_command(&["remote", "execution-consent", "preview", "conversation-17",])
            .unwrap(),
        RemoteCommand::ExecutionConsentPreview {
            conversation_id: "conversation-17".into(),
            instance_id: None,
            instance_view: None,
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "execution-consent",
            "preview",
            "conversation-17",
            "--instance",
            "client-cli-001",
            "--instance-view",
            "view.json",
        ])
        .unwrap(),
        RemoteCommand::ExecutionConsentPreview {
            conversation_id: "conversation-17".into(),
            instance_id: Some("client-cli-001".into()),
            instance_view: Some("view.json".into()),
        }
    );
    assert_execution_consent_rejects_invalid_options();
}

fn assert_execution_consent_rejects_invalid_options() {
    assert!(parse_remote_command(&["remote", "execution-consent"]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "execution-consent",
            "preview",
            "conversation-17",
            "extra",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&["remote", "execution-consent", "grant", "conversation-17",])
            .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "execution-consent",
            "preview",
            "conversation-17",
            "--instance-view",
            "view.json",
        ])
        .is_err()
    );
}

#[test]
fn remote_execution_reconciliation_preview_requires_one_bounded_input() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "execution-reconciliation",
            "preview",
            "--input",
            "restart.json",
        ])
        .unwrap(),
        RemoteCommand::ExecutionReconciliationPreview {
            input: "restart.json".into(),
        }
    );
    assert!(parse_remote_command(&["remote", "execution-reconciliation"]).is_err());
    assert!(parse_remote_command(&["remote", "execution-reconciliation", "preview",]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "execution-reconciliation",
            "preview",
            "--input",
            "restart.json",
            "--input",
            "other.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "execution-reconciliation",
            "preview",
            "--input",
            "",
        ])
        .is_err()
    );
}
