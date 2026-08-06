//! wave-admit — wave-parallel batch materialization CLI adapter.
//!
//! Plans a wave with the Go core (graph-scheduled-ready-nodes), materializes
//! one successor candidate per ready node (graph-scheduled-node-contract
//! --target-node), and admits each candidate into the hub. Effect-free
//! beyond admission (the same authority as a manual successor admit); the
//! per-node dispatch chain (prepare/authorize/execute) stays on the existing
//! provider-request commands.

use std::error::Error;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::{
    args::Args,
    state_path::{idempotency_key, unix_time_millis},
};

use super::{
    scheduled_contract_command::{
        self, successor_service, WaveAdmitInput, WaveSuccessorService,
    },
    scheduled_contract_output::GroupAgentScheduledNodeContractCliOutput,
};

/// One admitted wave node.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct WaveAdmitNodeOutput {
    pub node_id: String,
    pub disposition: String,
    pub contract_id: String,
}

/// Wave-admit aggregate output (JSON-serializable).
#[derive(serde::Serialize, serde::Deserialize)]
pub struct WaveAdmitOutput {
    pub wave: Vec<WaveAdmitNodeOutput>,
    pub rejected: Vec<WaveAdmitNodeOutput>,
}

pub fn execute_wave_admit(
    args: &Args,
    graph_run_id: &str,
    receipt_sources: &[String],
    schedule_sha256: &str,
    go_core: Option<&str>,
) -> Result<GroupAgentScheduledNodeContractCliOutput, Box<dyn Error>> {
    let control = scheduled_contract_command::export_control(args, graph_run_id)?;
    if receipt_sources.is_empty() {
        return Err("wave-admit requires at least one --predecessor-receipt".into());
    }
    let ready = run_ready_nodes(&control, receipt_sources, schedule_sha256, go_core)?;
    if ready.is_empty() {
        return Ok(GroupAgentScheduledNodeContractCliOutput::wave(
            WaveAdmitOutput {
                wave: Vec::new(),
                rejected: Vec::new(),
            },
        ));
    }
    let mut wave = Vec::new();
    let mut rejected = Vec::new();
    for node_id in ready {
        match materialize_node(args, graph_run_id, &control, &node_id,
                                receipt_sources, schedule_sha256, go_core) {
            Ok(node) => wave.push(node),
            Err(err) => rejected.push(WaveAdmitNodeOutput {
                node_id,
                disposition: format!("rejected: {err}"),
                contract_id: String::new(),
            }),
        }
    }
    Ok(GroupAgentScheduledNodeContractCliOutput::wave(
        WaveAdmitOutput { wave, rejected },
    ))
}

/// `run_ready_nodes` asks the Go core for the topologically-ready successor
/// node IDs of the consumed receipt set.
fn run_ready_nodes(
    control: &[u8],
    receipt_sources: &[String],
    schedule_sha256: &str,
    go_core: Option<&str>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut go = go_command(go_core);
    go.args([
        "graph-scheduled-ready-nodes",
        "--control",
        "-",
        "--schedule-sha256",
        schedule_sha256,
    ]);
    for source in receipt_sources {
        go.args(["--predecessor-receipt", source]);
    }
    let output = run_go_with_stdin(&mut go, control)?;
    let parsed: Vec<String> = serde_json::from_slice(&output)?;
    Ok(parsed)
}

/// `materialize_node` builds one successor candidate for a target node via the
/// Go core and admits it into the hub.
fn materialize_node(
    args: &Args,
    graph_run_id: &str,
    control: &[u8],
    node_id: &str,
    receipt_sources: &[String],
    schedule_sha256: &str,
    go_core: Option<&str>,
) -> Result<WaveAdmitNodeOutput, Box<dyn Error>> {
    let candidate = build_target_candidate(go_core, control, node_id,
                                           receipt_sources, schedule_sha256)?;
    let input = WaveAdmitInput {
        graph_run_id: graph_run_id.into(),
        contract_json: String::from_utf8(candidate)?,
        idempotency_key: idempotency_key("scheduled-wave-admit"),
        admitted_at_ms: unix_time_millis(),
        predecessor_content: None,
    };
    WaveSuccessorService::preflight_admit(&input)?;
    let result = successor_service(args)?.admit(&input)?;
    Ok(WaveAdmitNodeOutput {
        node_id: node_id.into(),
        disposition: format!("{:?}", result.disposition),
        contract_id: result.inspection.record.contract_id,
    })
}

/// `build_target_candidate` runs `graph-scheduled-node-contract` with
/// --target-node; control bytes are piped on stdin.
fn build_target_candidate(
    go_core: Option<&str>,
    control: &[u8],
    node_id: &str,
    receipt_sources: &[String],
    schedule_sha256: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut go = go_command(go_core);
    go.args([
        "graph-scheduled-node-contract",
        "--control",
        "-",
        "--schedule-sha256",
        schedule_sha256,
        "--endpoint",
        "https://api.openai.com/v1/responses",
        "--model",
        "gpt-5.2",
        "--max-output-tokens",
        "4096",
        "--max-model-output-bytes",
        "65536",
        "--max-model-events",
        "4096",
        "--timeout-ms",
        "300000",
        "--max-cost-usd-micros",
        "1000000",
        "--pricing-snapshot-sha256",
        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "--max-result-bytes",
        "262144",
        "--target-node",
        node_id,
    ]);
    for source in receipt_sources {
        go.args(["--predecessor-receipt", source]);
    }
    run_go_with_stdin(&mut go, control)
}

/// `run_go_with_stdin` pipes the control bytes into the Go core command's
/// stdin and returns its stdout; a non-zero exit fails closed.
fn run_go_with_stdin(go: &mut Command, control: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut child = go
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    std::io::Write::write_all(&mut stdin, control)?;
    drop(stdin);
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "go core failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(output.stdout)
}

/// `go_command` resolves the Go core invocation: an explicit binary path when
/// `--go-core` is given, else `go run ./cmd/forge` from `FORGE_CORE_DIR`
/// (the test convention).
fn go_command(go_core: Option<&str>) -> Command {
    if let Some(path) = go_core {
        return Command::new(path);
    }
    let core_dir = std::env::var("FORGE_CORE_DIR").unwrap_or_else(|_| {
        // Prefer the workspace layout (crates/interfaces -> forge-runtime ->
        // repository root) so the CLI works from any cwd; fall back to cwd.
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut candidate: Option<PathBuf> = None;
        // crates/interfaces -> crates -> forge-runtime -> repository root.
        if let Some(root) = manifest
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            let joined = root.join("forge-core");
            if joined.exists() {
                candidate = Some(joined);
            }
        }
        let fallback = std::env::current_dir().unwrap_or_default().join("forge-core");
        candidate.unwrap_or(fallback).display().to_string()
    });
    let mut cmd = Command::new("go");
    cmd.current_dir(core_dir).args(["run", "./cmd/forge"]);
    cmd
}


///  renders the wave-admit summary (human output path).
pub(crate) fn write_wave(
    writer: &mut impl std::io::Write,
    wave: &[WaveAdmitNodeOutput],
    rejected: &[WaveAdmitNodeOutput],
) -> std::io::Result<()> {
    for node in wave {
        writeln!(writer, "wave-admit {} — {}", node.node_id, node.disposition)?;
        if !node.contract_id.is_empty() {
            writeln!(writer, "  contract_id: {}", node.contract_id)?;
        }
    }
    for node in rejected {
        writeln!(writer, "wave-admit {} — {}", node.node_id, node.disposition)?;
    }
    writeln!(writer, "wave: {} admitted, {} rejected", wave.len(), rejected.len())
}
