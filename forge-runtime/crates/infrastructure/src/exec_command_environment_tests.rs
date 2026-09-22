use std::ffi::OsString;

use forge_runtime_domain::execution::fabric::{
    ENVIRONMENT_DIGEST_ALGORITHM, EnvironmentDigest, MAX_ENVIRONMENT_DIGEST_ENTRIES,
};

use super::{
    SafeEnvironment, capture_from, is_sensitive_environment_name, is_valid_environment_digest,
};

#[test]
fn sorted_safe_entries_match_the_frozen_digest_fixture() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/local_environment_digest.v1.json"
    ))
    .expect("environment digest fixture parses");
    let environment = capture_from([
        (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
        (OsString::from("LANG"), OsString::from("C.UTF-8")),
        (OsString::from("HOME"), OsString::from("/fixture/home")),
    ]);
    assert_eq!(
        serde_json::to_value(environment.digest()).expect("digest serializes"),
        fixture["digest"]
    );
    assert!(is_valid_environment_digest(&environment.digest));
}

#[test]
fn sensitive_or_unallowlisted_names_never_enter_the_snapshot_or_digest() {
    let environment = capture_from([
        (OsString::from("PATH"), OsString::from("/usr/bin")),
        (OsString::from("HOME"), OsString::from("/fixture/home")),
        (
            OsString::from("OPENAI_API_KEY"),
            OsString::from("secret-value"),
        ),
        (
            OsString::from("DB_PASSWORD"),
            OsString::from("secret-value"),
        ),
        (
            OsString::from("HTTP_AUTHORIZATION"),
            OsString::from("secret-value"),
        ),
    ]);
    assert!(is_sensitive_environment_name("OPENAI_API_KEY"));
    assert_eq!(environment.entries.len(), 2);
    let serialized = serde_json::to_string(&environment.digest()).expect("digest serializes");
    assert!(!serialized.contains("secret-value"));
    assert!(matches!(
        environment.digest,
        EnvironmentDigest::Captured {
            algorithm,
            entry_count: 2,
            ..
        } if algorithm == ENVIRONMENT_DIGEST_ALGORITHM
    ));
}

#[test]
fn malformed_or_unbounded_safe_values_fail_closed_without_leaking_input() {
    let oversized = "x".repeat(16 * 1024 + 1);
    let environment = capture_from([
        (OsString::from("PATH"), OsString::from("/usr/bin")),
        (OsString::from("HOME"), OsString::from(oversized.clone())),
    ]);
    let EnvironmentDigest::NotCaptured { reason } = environment.digest else {
        panic!("oversized value must not produce a captured digest");
    };
    assert!(reason.contains("exceeds capture bound"));
    assert!(!reason.contains(&oversized));
    // The digest fails closed, while the command path retains the valid
    // allowlisted entry for compatibility with the existing environment
    // filtering behavior.
    assert_eq!(environment.entries.len(), 1);

    let duplicate = capture_from([
        (OsString::from("PATH"), OsString::from("/usr/bin")),
        (OsString::from("path"), OsString::from("/bin")),
    ]);
    assert!(matches!(
        duplicate.digest,
        EnvironmentDigest::NotCaptured { reason } if reason.contains("duplicate")
    ));
}

#[test]
fn missing_path_fails_closed_without_a_partial_captured_digest() {
    let environment = capture_from([(OsString::from("HOME"), OsString::from("/fixture/home"))]);
    assert!(matches!(
        environment.digest,
        EnvironmentDigest::NotCaptured { reason } if reason.contains("PATH")
    ));
}

#[cfg(unix)]
#[test]
fn invalid_utf8_safe_values_fail_closed() {
    use std::os::unix::ffi::OsStringExt;

    let environment = capture_from([
        (OsString::from("PATH"), OsString::from("/usr/bin")),
        (OsString::from("HOME"), OsString::from_vec(vec![0xff, 0xfe])),
    ]);
    assert!(matches!(
        environment.digest,
        EnvironmentDigest::NotCaptured { reason } if reason.contains("UTF-8")
    ));
    assert_eq!(environment.entries.len(), 1);
}

#[cfg(unix)]
#[test]
fn invalid_utf8_environment_names_fail_closed() {
    use std::os::unix::ffi::OsStringExt;

    let environment = capture_from([
        (OsString::from("PATH"), OsString::from("/usr/bin")),
        (
            OsString::from_vec(vec![b'H', 0xff]),
            OsString::from("value"),
        ),
    ]);
    assert!(matches!(
        environment.digest,
        EnvironmentDigest::NotCaptured { reason } if reason.contains("name") && reason.contains("UTF-8")
    ));
    assert_eq!(environment.entries.len(), 1);
}

#[test]
fn environment_digest_validation_rejects_malformed_captured_values() {
    assert!(!is_valid_environment_digest(&EnvironmentDigest::Captured {
        algorithm: "sha512".into(),
        sha256: "a".repeat(64),
        entry_count: 1,
    }));
    assert!(!is_valid_environment_digest(&EnvironmentDigest::Captured {
        algorithm: ENVIRONMENT_DIGEST_ALGORITHM.into(),
        sha256: "A".repeat(64),
        entry_count: 1,
    }));
    assert!(!is_valid_environment_digest(&EnvironmentDigest::Captured {
        algorithm: ENVIRONMENT_DIGEST_ALGORITHM.into(),
        sha256: "a".repeat(64),
        entry_count: MAX_ENVIRONMENT_DIGEST_ENTRIES + 1,
    }));
    assert!(!is_valid_environment_digest(
        &EnvironmentDigest::NotCaptured {
            reason: String::new(),
        }
    ));
}

#[test]
fn actual_capture_is_bounded_and_never_claims_more_than_the_local_profile() {
    let environment = SafeEnvironment::capture();
    assert!(environment.entries.len() <= usize::from(MAX_ENVIRONMENT_DIGEST_ENTRIES));
    assert!(is_valid_environment_digest(&environment.digest));
}
