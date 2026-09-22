use std::{
    collections::HashSet,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
};

use forge_runtime_domain::{
    InventorySnapshot, SnapshotOwner, SnapshotRow, canonicalize_inventory_snapshot,
    inventory_snapshot_digest,
};
use serde::{Deserialize, Serialize, de, de::DeserializeSeed};

use crate::args::{DeviceCommand, DeviceInventoryCommand};

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
const MAX_CASES: usize = 128;
const SCHEMA_VERSION: &str = "forge.device-inventory-snapshot-canonical/v1";
const EVALUATION_MODE: &str = "pure_owner_scoped_snapshot_only";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotFixture {
    schema_version: String,
    evaluation_mode: String,
    owner_declaration_unverified: bool,
    inventory_declarations_unverified: bool,
    authority: Authority,
    cases: Vec<SnapshotCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotCase {
    name: String,
    input: SnapshotInput,
    expected: SnapshotExpected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotInput {
    snapshot_id: String,
    observed_at_ms: u64,
    owner: SnapshotOwnerInput,
    rows: Vec<SnapshotRowInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotOwnerInput {
    issuer: String,
    subject: String,
    tenant_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotRowInput {
    device_id: String,
    instance_id: String,
    owner: SnapshotOwnerInput,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotExpected {
    #[serde(default)]
    ordered_keys: Option<Vec<String>>,
    #[serde(default)]
    canonical_sha256: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
struct Authority {
    identity_verified: bool,
    heartbeat_persisted: bool,
    inventory_authoritative: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct SnapshotCanonicalOutput {
    v: u16,
    #[serde(rename = "type")]
    output_type: &'static str,
    schema_version: &'static str,
    evaluation_mode: &'static str,
    owner_declaration_unverified: bool,
    inventory_declarations_unverified: bool,
    cases: Vec<SnapshotCaseOutput>,
    authority: Authority,
}

#[derive(Debug, Serialize)]
struct SnapshotCaseOutput {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ordered_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

pub(crate) fn execute(command: &DeviceCommand) -> Result<SnapshotCanonicalOutput, Box<dyn Error>> {
    let DeviceCommand::Inventory(DeviceInventoryCommand::SnapshotCanonical { input }) = command
    else {
        return Err("device inventory snapshot-canonical command is required".into());
    };
    let bytes = read_bounded_input(input)?;
    reject_duplicate_keys(&bytes)
        .map_err(|error| format!("device inventory snapshot input is invalid JSON: {error}"))?;
    let fixture: SnapshotFixture = serde_json::from_slice(&bytes)
        .map_err(|error| format!("device inventory snapshot input is invalid JSON: {error}"))?;
    evaluate(fixture)
}

pub(crate) fn run(command: &DeviceCommand, json: bool) -> ExitCode {
    let output = match execute(command) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("Device inventory snapshot command failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = write_output(&output, json, &mut io::stdout().lock()) {
        eprintln!("failed to write device inventory snapshot output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

pub(crate) fn write_output(
    output: &SnapshotCanonicalOutput,
    json: bool,
    writer: &mut impl Write,
) -> io::Result<()> {
    if json {
        serde_json::to_writer_pretty(&mut *writer, output)?;
        return writeln!(writer);
    }
    writeln!(
        writer,
        "offline device inventory snapshot canonical [{}]",
        output.schema_version
    )?;
    writeln!(
        writer,
        "declarations: owner_unverified={} inventory_unverified={}",
        output.owner_declaration_unverified, output.inventory_declarations_unverified
    )?;
    for case in &output.cases {
        if let Some(error) = &case.error {
            writeln!(writer, "{}: error={error}", case.name)?;
        } else {
            writeln!(
                writer,
                "{}: ordered_keys={} digest={}",
                case.name,
                case.ordered_keys.as_ref().map_or(0, Vec::len),
                case.canonical_sha256.as_deref().unwrap_or("missing")
            )?;
        }
    }
    writeln!(
        writer,
        "authority: identity_verified=false heartbeat_persisted=false inventory_authoritative=false reservation_created=false execution_authorized=false dispatch_performed=false"
    )
}

fn evaluate(fixture: SnapshotFixture) -> Result<SnapshotCanonicalOutput, Box<dyn Error>> {
    validate_fixture_shape(&fixture)?;
    let mut names = HashSet::with_capacity(fixture.cases.len());
    let mut cases = Vec::with_capacity(fixture.cases.len());
    for case in fixture.cases {
        if case.name.trim().is_empty() || case.name.len() > 128 {
            return Err("device inventory snapshot case name is empty or too long".into());
        }
        if !names.insert(case.name.clone()) {
            return Err(format!("duplicate device inventory snapshot case {:?}", case.name).into());
        }
        cases.push(evaluate_case(case)?);
    }
    Ok(SnapshotCanonicalOutput {
        v: 1,
        output_type: "device_inventory_snapshot_canonical",
        schema_version: SCHEMA_VERSION,
        evaluation_mode: EVALUATION_MODE,
        owner_declaration_unverified: true,
        inventory_declarations_unverified: true,
        cases,
        authority: Authority::default(),
    })
}

fn validate_fixture_shape(fixture: &SnapshotFixture) -> Result<(), Box<dyn Error>> {
    if fixture.schema_version != SCHEMA_VERSION
        || fixture.evaluation_mode != EVALUATION_MODE
        || !fixture.owner_declaration_unverified
        || !fixture.inventory_declarations_unverified
        || fixture.cases.is_empty()
        || fixture.cases.len() > MAX_CASES
        || fixture.authority != Authority::default()
    {
        return Err(
            "device inventory snapshot input is not a bounded pure read-only contract".into(),
        );
    }
    Ok(())
}

fn evaluate_case(case: SnapshotCase) -> Result<SnapshotCaseOutput, Box<dyn Error>> {
    let snapshot = to_domain_snapshot(case.input);
    let actual = canonicalize_inventory_snapshot(&snapshot);
    match (case.expected.error, actual) {
        (Some(expected_error), Err(error)) => {
            if expected_error != error.to_string()
                || case.expected.ordered_keys.is_some()
                || case.expected.canonical_sha256.is_some()
            {
                return Err(format!("inventory snapshot case {:?} expectation mismatch", case.name).into());
            }
            Ok(SnapshotCaseOutput {
                name: case.name,
                ordered_keys: None,
                canonical_sha256: None,
                error: Some(expected_error),
            })
        }
        (None, Ok(canonical)) => {
            let expected_keys = case.expected.ordered_keys.ok_or("missing expected ordered_keys")?;
            let expected_digest = case.expected.canonical_sha256.ok_or("missing expected canonical_sha256")?;
            let ordered_keys = row_keys(&canonical.rows);
            let digest = inventory_snapshot_digest(&snapshot)?;
            if expected_keys != ordered_keys || expected_digest != digest {
                return Err(format!("inventory snapshot case {:?} expectation mismatch", case.name).into());
            }
            Ok(SnapshotCaseOutput {
                name: case.name,
                ordered_keys: Some(ordered_keys),
                canonical_sha256: Some(digest),
                error: None,
            })
        }
        (Some(expected_error), Ok(_)) => Err(format!(
            "inventory snapshot case {:?} expected error {expected_error:?} but canonicalization succeeded",
            case.name
        )
        .into()),
        (None, Err(error)) => Err(format!(
            "inventory snapshot case {:?} rejected unexpectedly: {error}",
            case.name
        )
        .into()),
    }
}

fn to_domain_snapshot(input: SnapshotInput) -> InventorySnapshot {
    InventorySnapshot {
        snapshot_id: input.snapshot_id,
        observed_at_ms: input.observed_at_ms,
        owner: to_domain_owner(input.owner),
        rows: input
            .rows
            .into_iter()
            .map(|row| SnapshotRow {
                device_id: row.device_id,
                instance_id: row.instance_id,
                owner: to_domain_owner(row.owner),
            })
            .collect(),
    }
}

fn to_domain_owner(owner: SnapshotOwnerInput) -> SnapshotOwner {
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

fn read_bounded_input(input: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(MAX_INPUT_BYTES.min(64 * 1024));
    if input == "-" {
        io::stdin()
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    } else {
        File::open(Path::new(input))?
            .take((MAX_INPUT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(
            format!("device inventory snapshot input exceeds {MAX_INPUT_BYTES} bytes").into(),
        );
    }
    Ok(bytes)
}

/// Reject duplicate object keys at every depth before serde materializes the
/// fixture. `serde_json` otherwise keeps the last duplicate value, which
/// would make an offline snapshot declaration depend on parser behavior.
fn reject_duplicate_keys(bytes: &[u8]) -> Result<(), serde_json::Error> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    UniqueJson.deserialize(&mut decoder)?;
    decoder.end()
}

struct UniqueJson;

impl<'de> DeserializeSeed<'de> for UniqueJson {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<(), D::Error> {
        decoder.deserialize_any(self)
    }
}

impl<'de> de::Visitor<'de> for UniqueJson {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence.next_element_seed(UniqueJson)?.is_some() {}
        Ok(())
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut object: A) -> Result<(), A::Error> {
        let mut keys = HashSet::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            object.next_value_seed(UniqueJson)?;
        }
        Ok(())
    }
}
