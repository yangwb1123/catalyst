//! wave-admit — wave-parallel batch materialization CLI adapter.
//!
//! Plans a wave with the Go core (graph-scheduled-ready-nodes), materializes
//! one successor candidate per ready node (graph-scheduled-node-contract
//! --target-node), and admits each candidate into the hub. Effect-free
//! beyond admission (the same authority as a manual successor admit); the
//! per-node dispatch chain (prepare/authorize/execute) stays on the existing
//! provider-request commands.

use std::collections::VecDeque;
use std::error::Error;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::args::{next_value, WaveAdmitExecutionOptions};

use crate::{
    args::Args,
    state_path::{idempotency_key as generated_idempotency_key, unix_time_millis},
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
    idempotency_key: Option<&str>,
    execution: &crate::args::WaveAdmitExecutionOptions,
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
        let ctx = MaterializeContext {
            args,
            control: &control,
            receipt_sources,
            schedule_sha256,
            go_core,
            idempotency_key,
            execution,
        };
        match materialize_node(graph_run_id, &node_id, &ctx) {
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

/// Shared materialization context (keeps `materialize_node` within the
/// argument budget).
struct MaterializeContext<'a> {
    args: &'a Args,
    control: &'a [u8],
    receipt_sources: &'a [String],
    schedule_sha256: &'a str,
    go_core: Option<&'a str>,
    idempotency_key: Option<&'a str>,
    execution: &'a crate::args::WaveAdmitExecutionOptions,
}

/// `materialize_node` builds one successor candidate for a target node via the
/// Go core and admits it into the hub.
fn materialize_node(
    graph_run_id: &str,
    node_id: &str,
    ctx: &MaterializeContext<'_>,
) -> Result<WaveAdmitNodeOutput, Box<dyn Error>> {
    let candidate = build_target_candidate(ctx.go_core, ctx.control, node_id,
                                           ctx.receipt_sources, ctx.schedule_sha256, ctx.execution)?;
    let input = WaveAdmitInput {
        graph_run_id: graph_run_id.into(),
        contract_json: String::from_utf8(candidate)?,
        // Deterministic per-node key derived from the user key (or a fresh
        // base): a re-run with the same --idempotency-key replays admitted
        // nodes instead of rejecting them (Finding 3).
        idempotency_key: {
            let base = ctx
                .idempotency_key
                .map_or_else(|| generated_idempotency_key("scheduled-wave-admit"), str::to_owned);
            format!("{base}-{node_id}")
        },
        admitted_at_ms: unix_time_millis(),
        predecessor_content: None,
    };
    WaveSuccessorService::preflight_admit(&input)?;
    let result = successor_service(ctx.args)?.admit(&input)?;
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
    execution: &crate::args::WaveAdmitExecutionOptions,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut go = go_command(go_core);
    go.args([
        "graph-scheduled-node-contract",
        "--control",
        "-",
        "--schedule-sha256",
        schedule_sha256,
        "--endpoint",
        &execution.endpoint,
        "--model",
        &execution.model,
        "--max-output-tokens",
        &execution.max_output_tokens,
        "--max-model-output-bytes",
        &execution.max_model_output_bytes,
        "--max-model-events",
        &execution.max_model_events,
        "--timeout-ms",
        &execution.timeout_ms,
        "--max-cost-usd-micros",
        &execution.max_cost_usd_micros,
        "--pricing-snapshot-sha256",
        &execution.pricing_snapshot_sha256,
        "--max-result-bytes",
        &execution.max_result_bytes,
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

/// Binds one wave-admit execution-option flag; returns false for unknown
/// flags so the caller can reject them (review Finding 2: no literals).
pub(crate) fn bind_wave_execution_flag(
    flag: &str,
    tokens: &mut VecDeque<String>,
    execution: &mut WaveAdmitExecutionOptions,
) -> Result<bool, String> {
    let value = next_value(tokens, flag)?;
    match flag {
        "--endpoint" => execution.endpoint = value,
        "--model" => execution.model = value,
        "--max-output-tokens" => execution.max_output_tokens = value,
        "--max-model-output-bytes" => execution.max_model_output_bytes = value,
        "--max-model-events" => execution.max_model_events = value,
        "--timeout-ms" => execution.timeout_ms = value,
        "--max-cost-usd-micros" => execution.max_cost_usd_micros = value,
        "--pricing-snapshot-sha256" => execution.pricing_snapshot_sha256 = value,
        "--max-result-bytes" => execution.max_result_bytes = value,
        _ => return Ok(false),
    }
    Ok(true)
}


pub(crate) fn wave_execution_complete(execution: &WaveAdmitExecutionOptions) -> bool {
    !execution.endpoint.is_empty()
        && !execution.model.is_empty()
        && !execution.max_output_tokens.is_empty()
        && !execution.max_model_output_bytes.is_empty()
        && !execution.max_model_events.is_empty()
        && !execution.timeout_ms.is_empty()
        && !execution.max_cost_usd_micros.is_empty()
        && !execution.pricing_snapshot_sha256.is_empty()
        && !execution.max_result_bytes.is_empty()
}
