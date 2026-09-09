use super::{codegen, lex, scan, serde_policy};
use sha2::{Digest, Sha256};
use std::path::Path;

// The final implementation closure is populated only after review. Each entry
// grants source access, never runtime permission, and is checked before use.
const REVIEWED: &[(&str, &str)] = &[
    (
        "crates/infrastructure/src/lib.rs",
        "6027f506ab4231eefe4f0b3a1a059ca4344b6319db15de87d51db7163c41af72",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/budget.rs",
        "e9c0e1db91b1af15b1f7f1f6db81fa022d6154ca86fb155b4ab48d3603ec1060",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/codec.rs",
        "acc6d2954be34c5a532f20aa60f07d1815203ef5cb9fd0cd642f365ff762f2ff",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/codec_model.rs",
        "7359f21dffafb53899d6c585c3d1d69efbdbf049cd82a869ba484ae67212a090",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/codec_tests.rs",
        "8495bbb1008d066726e0f7d98405a2f6ca63dbbee5860feac3d1c516a43fc349",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/corruption_tests.rs",
        "6877975287c75c98f2e682a2643a8eb4f623051268454ea0cc902d7be87b904a",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/crash_fixture.rs",
        "5d58d523e3bc1ffb2cdb288313393d8badf0cefb7553b1c558e37dc7ed7c9629",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/crash_tests.rs",
        "d27d5d51d5aa4bc5d2d460b9c5867902da113656f00b84a2d14eabdb7ce4caa9",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/event.rs",
        "78e9b45c01b2d95b8c5e30d45e4a71be4b49d3e0446e882ef196aeb70f0c0cc9",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/mod.rs",
        "4e08d9ffda9ed96151c6f5099cf635087807e81e8eb779987725846564a78bc2",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/read.rs",
        "9321d1a4d3e8d160fbe24ceb16a47d1dce18581b034583369a355f4ab7e0e701",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/schema.rs",
        "919dc9c73beb821912aa7441448b6e4fb66e037a3a538700fcc7fbba058da22c",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/schema_sql.rs",
        "3212f30b43c8d93aeeb94d09ef7f941e9d2b1a757ac8e103444e5e800c3ee3d6",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/types.rs",
        "4d5ee68cc2375255d42046a9ad772a87c6bc24f6e07a4a621bc4d914331bc50c",
    ),
    (
        "crates/infrastructure/src/sqlite_execution/write.rs",
        "d0b29b41c03d9bf50f6f8578836bc6e9a3fe72f87f918bb4783a7cba088e279d",
    ),
    (
        "crates/infrastructure/tests/attempt_admission.rs",
        "f2bfd0dcf881da89b9bdd77c73c29452baa097d828b68cbfe5ec0c3ac32fcec3",
    ),
    (
        "crates/infrastructure/tests/attempt_admission_support/bindings.rs",
        "d5543d0396bd44633bd305715ed8bee7aef92d8eb1d33a8a845170d090a1a638",
    ),
    (
        "crates/infrastructure/tests/attempt_admission_support/capacity.rs",
        "6acc4859907dcb7302c5d27b5307980d902f6f1cff9069b34ce356d1a63a9bd5",
    ),
    (
        "crates/infrastructure/tests/attempt_admission_support/connections.rs",
        "5442914bfb438cb41af956330f29d28c9bdfa5ab888bc51ea3e82034d2926406",
    ),
    (
        "crates/infrastructure/tests/attempt_admission_support/mod.rs",
        "78f6badc0db76a0855e246c22ce5c01ef452ddfee40cca4cdc92634ae047986d",
    ),
    (
        "crates/infrastructure/tests/attempt_admission_support/writers.rs",
        "76c86f744bae24871a8ee1facc7bc0bb8bdceac1c837573ca5c3e26fcc758465",
    ),
];
const PRODUCTION: &str = "crates/infrastructure/src/sqlite_execution/";
const REGISTRATION: &str = "crates/infrastructure/src/lib.rs";
const SYMBOLS: &[&str] = &[
    "sqlite_execution",
    "SqliteAttemptJournal",
    "AttemptJournalError",
    "AttemptAdmission",
    "AdmissionDisposition",
    "AdmissionResult",
    "PendingPage",
    "PendingEvent",
];

pub(super) fn verify_inventory(workspace: &Path) {
    assert!(
        !REVIEWED.is_empty(),
        "admission review inventory is missing"
    );
    let mut total = 0;
    for (relative, _) in REVIEWED {
        let source = scan::read_bounded_source(&workspace.join(relative), &mut total);
        check_source(&source, Some(relative)).expect("exact admission source inventory");
    }
}

pub(super) fn check_source(source: &str, relative: Option<&str>) -> Result<bool, String> {
    serde_policy::check(source, relative)?;
    let tokens = lex::tokenize(source)?;
    codegen::check(&tokens, true, source, relative)?;
    if let Some((path, expected)) = REVIEWED.iter().find(|(path, _)| Some(*path) == relative) {
        if format!("{:x}", Sha256::digest(source.as_bytes())) != *expected {
            return Err(format!("admission reviewed source drift: {path}"));
        }
        return Ok(*path != REGISTRATION);
    }
    if relative.is_some_and(|path| path.starts_with(PRODUCTION)) {
        return Err("unreviewed admission implementation source".into());
    }
    if tokens.iter().any(|token| SYMBOLS.contains(&token.as_str())) {
        return Err("unreviewed admission consumer".into());
    }
    Ok(false)
}

pub(super) fn reviewed_path(path: &Path, workspace: &Path) -> bool {
    REVIEWED
        .iter()
        .any(|(relative, _)| path == workspace.join(relative))
}

pub fn reviewed_sources() -> &'static [(&'static str, &'static str)] {
    REVIEWED
}
