use std::{fmt::Write as _, path::Path};

use forge_runtime_domain::GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};

pub fn snapshot_hash(bytes: &[u8]) -> String {
    let hash = snapshot_hash_bytes(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in hash {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

pub fn tamper_rows(database: &Path, other_group_id: &str) {
    let connection = Connection::open(database).expect("raw SQLite");
    connection
        .execute_batch("PRAGMA ignore_check_constraints = ON")
        .expect("allow corruption fixture");
    connection
        .execute(
            "UPDATE group_runs SET snapshot_sha256=zeroblob(32) WHERE id='run-0'",
            [],
        )
        .expect("tamper outer digest");
    connection
        .execute(
            "UPDATE group_runs SET context_slice_sha256=zeroblob(32) WHERE id='run-1'",
            [],
        )
        .expect("tamper inner digest");
    connection
        .execute(
            "UPDATE group_runs SET status='executed' WHERE id='run-2'",
            [],
        )
        .expect("tamper status");
    connection
        .execute(
            "UPDATE group_runs SET group_id=?1 WHERE id='run-3'",
            [other_group_id],
        )
        .expect("tamper Group binding");
    connection
        .execute(
            "UPDATE group_runs SET idempotency_key=' ' WHERE id='run-5'",
            [],
        )
        .expect("tamper idempotency key emptiness");
    connection
        .execute(
            "UPDATE group_runs SET idempotency_key=?1 WHERE id='run-6'",
            ["k".repeat(257)],
        )
        .expect("tamper idempotency key bounds");
    install_noncanonical_json(&connection);
}

fn install_noncanonical_json(connection: &Connection) {
    let bytes: Vec<u8> = connection
        .query_row(
            "SELECT context_blob FROM group_runs WHERE id='run-4'",
            [],
            |row| row.get(0),
        )
        .expect("snapshot bytes");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("snapshot value");
    let pretty = serde_json::to_vec_pretty(&value).expect("pretty snapshot");
    let digest = snapshot_hash_bytes(&pretty);
    connection
        .execute(
            "UPDATE group_runs
             SET context_blob=?1,snapshot_sha256=?2
             WHERE id='run-4'",
            params![pretty, digest.as_slice()],
        )
        .expect("install noncanonical snapshot");
}

fn snapshot_hash_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(GROUP_RUN_SNAPSHOT_DIGEST_DOMAIN);
    digest.update(bytes);
    digest.finalize().into()
}
