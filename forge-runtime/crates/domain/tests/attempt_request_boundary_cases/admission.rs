use crate::attempt_request_boundary_support::{
    check_admission_consumer_fixture, check_lifecycle_consumer_fixture, check_path_fixture,
    reviewed_admission_sources,
};
use std::{fs, path::Path};

#[test]
fn admission_access_requires_exact_reviewed_path_and_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let sources = reviewed_admission_sources();
    assert!(!sources.is_empty());
    for (relative, _) in sources {
        let source = fs::read_to_string(root.join(relative)).unwrap();
        assert!(check_admission_consumer_fixture(&source, Some(relative)).is_ok());
        assert!(check_admission_consumer_fixture(&format!("{source}\n"), Some(relative)).is_err());
        let with_consumer =
            format!("{source}\nuse forge_runtime_infrastructure::sqlite_execution;");
        assert!(check_admission_consumer_fixture(&with_consumer, None).is_err());
    }
}

#[test]
fn admission_exceptions_do_not_admit_lifecycle_or_serde_codegen() {
    for (relative, _) in reviewed_admission_sources() {
        assert!(check_lifecycle_consumer_fixture(
            "fn bypass() { let _ = execution::attempt_lifecycle::AttemptLifecycle::requested(); }",
            Some(relative),
        ).is_err());
        assert!(
            check_admission_consumer_fixture(
                r#"#[serde(remote = "Hidden")] enum Mirror {}"#,
                Some(relative),
            )
            .is_err()
        );
    }
}

#[test]
fn admission_consumer_names_and_unreviewed_closure_members_fail_closed() {
    for source in [
        "use forge_runtime_infrastructure::sqlite_execution as hidden;",
        "use forge_runtime_infrastructure::{sqlite_execution::*};",
        "fn consumer(store: SqliteAttemptJournal) {}",
        "pub use forge_runtime_infrastructure::r#sqlite_execution;",
    ] {
        assert!(check_admission_consumer_fixture(source, None).is_err());
    }
    assert!(
        check_admission_consumer_fixture(
            "fn extra() {}",
            Some("crates/infrastructure/src/sqlite_execution/extra.rs"),
        )
        .is_err()
    );
    assert!(
        !check_admission_consumer_fixture(
            "// SqliteAttemptJournal\nconst LABEL: &str = \"sqlite_execution\";",
            None,
        )
        .unwrap()
    );
}

#[test]
fn admission_reviewed_sources_cannot_be_reused_by_path_attributes() {
    for (relative, _) in reviewed_admission_sources() {
        let target = relative.strip_prefix("crates/").unwrap();
        for attribute in ["path", "r#path"] {
            let source = format!("#[{attribute} = \"../../{target}\"] mod reused;");
            assert!(
                check_path_fixture(&source).is_err(),
                "path reuse passed: {source}"
            );
        }
    }
}
