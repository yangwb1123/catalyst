use super::*;
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../../docs/contracts/fixtures/forge-device-inventory-snapshot-canonical-v1.json"
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: String,
    evaluation_mode: String,
    owner_declaration_unverified: bool,
    inventory_declarations_unverified: bool,
    authority: Authority,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    input: Input,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    snapshot_id: String,
    observed_at_ms: u64,
    owner: Owner,
    rows: Vec<Row>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Owner {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    device_id: String,
    instance_id: String,
    owner: Owner,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    #[serde(default)]
    ordered_keys: Option<Vec<String>>,
    #[serde(default)]
    canonical_sha256: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[test]
fn inventory_snapshot_canonical_contract_fixture() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture.schema_version,
        "forge.device-inventory-snapshot-canonical/v1"
    );
    assert_eq!(fixture.evaluation_mode, "pure_owner_scoped_snapshot_only");
    assert!(fixture.owner_declaration_unverified);
    assert!(fixture.inventory_declarations_unverified);
    assert!(!fixture.authority.identity_verified);
    assert!(!fixture.authority.heartbeat_persisted);
    assert!(!fixture.authority.inventory_authoritative);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert_eq!(fixture.cases.len(), 6);
    for case in fixture.cases {
        let snapshot = to_snapshot(case.input);
        let before = snapshot.rows.clone();
        let result = canonicalize_inventory_snapshot(&snapshot);
        if let Some(error) = case.expected.error.as_deref() {
            assert_eq!(
                result.expect_err(&case.name).to_string(),
                error,
                "{}",
                case.name
            );
            continue;
        }
        let canonical = result.unwrap_or_else(|error| panic!("{} rejected: {error}", case.name));
        assert_eq!(snapshot.rows, before, "{} mutated input", case.name);
        let expected_keys = case.expected.ordered_keys.as_deref().unwrap();
        assert_eq!(row_keys(&canonical.rows), expected_keys, "{}", case.name);
        assert_eq!(
            inventory_snapshot_digest(&snapshot).unwrap(),
            case.expected.canonical_sha256.as_deref().unwrap(),
            "{}",
            case.name
        );
    }
}

fn to_snapshot(input: Input) -> InventorySnapshot {
    InventorySnapshot {
        snapshot_id: input.snapshot_id,
        observed_at_ms: input.observed_at_ms,
        owner: to_owner(input.owner),
        rows: input
            .rows
            .into_iter()
            .map(|row| SnapshotRow {
                device_id: row.device_id,
                instance_id: row.instance_id,
                owner: to_owner(row.owner),
            })
            .collect(),
    }
}

fn to_owner(owner: Owner) -> SnapshotOwner {
    SnapshotOwner {
        issuer: owner.issuer,
        subject: owner.subject,
        tenant_id: owner.tenant_id,
    }
}

fn row_keys(rows: &[SnapshotRow]) -> Vec<String> {
    rows.iter()
        .map(|row| format!("{}/{}", row.device_id, row.instance_id))
        .collect()
}
