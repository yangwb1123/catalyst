#![cfg(unix)]

use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::{TempDir, tempdir};

#[allow(clippy::duplicate_mod, dead_code)]
mod group_agent_graph_run_support;
#[allow(clippy::duplicate_mod, dead_code)]
mod group_agent_graph_support;
#[allow(clippy::duplicate_mod, dead_code)]
mod scheduled_graph_controller_cli_support;
#[allow(clippy::duplicate_mod, dead_code)]
mod scheduled_graph_reconcile_cli_support;

use group_agent_graph_support::{successful_json, text};
use scheduled_graph_controller_cli_support::{ControllerFixture, MODEL, assert_event_chain};
use scheduled_graph_reconcile_cli_support::{PinnedGoCore, shared_core};

const DUMMY_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn recovery_precedes_pricing_and_concurrent_processes_cas_without_provider_effect() {
    let real_core = shared_core();
    let core = BlockingMaterializationCore::new(real_core);
    let fixture = ControllerFixture::new(real_core);
    assert_recovery_precedes_pricing(&fixture, &core);
    let outputs = race_passive_advances(&fixture, &core);
    assert_cas_outcome(&fixture, &core, &outputs);
}

fn assert_recovery_precedes_pricing(
    fixture: &ControllerFixture,
    core: &BlockingMaterializationCore,
) {
    core.fail_materialization();
    let start = fixture.start(&core.path, &core.sha256);
    assert_materialization_failure(&start);
    let planned = successful_json(&fixture.show());
    assert_planned_recovery(&planned);
    let planned_hub = fixture.source.hub_state();

    let pricing_poison = fixture.source.cwd.path().join("pricing-must-not-be-read");
    let mut poisoned = fixture.step_command(
        &core.path,
        &core.sha256,
        DUMMY_DIGEST,
        "provider-must-not-be-consumed",
        DUMMY_DIGEST,
        &pricing_poison,
    );
    let poisoned = poisoned.output().expect("run recovery step");
    assert_materialization_failure(&poisoned);
    assert!(!String::from_utf8_lossy(&poisoned.stderr).contains("pricing"));
    assert_eq!(
        fixture.source.hub_state(),
        planned_hub,
        "failed recovery changed Hub"
    );
}

fn race_passive_advances(
    fixture: &ControllerFixture,
    core: &BlockingMaterializationCore,
) -> [Output; 2] {
    core.allow_materialization();
    core.block_materialization();
    let first = spawn_advance(fixture, core);
    assert!(
        core.wait_for_arrivals(1),
        "first process did not reach materialization"
    );
    thread::sleep(Duration::from_millis(10));
    let second = spawn_advance(fixture, core);
    assert!(
        core.wait_for_arrivals(2),
        "second process did not reach materialization"
    );
    core.unblock_materialization();
    [wait(first), wait(second)]
}

fn assert_cas_outcome(
    fixture: &ControllerFixture,
    core: &BlockingMaterializationCore,
    outputs: &[Output; 2],
) {
    assert_eq!(
        outputs
            .iter()
            .filter(|output| output.status.success())
            .count(),
        1
    );
    let loser = outputs
        .iter()
        .find(|output| !output.status.success())
        .unwrap();
    assert!(loser.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&loser.stderr).contains("journal changed concurrently"),
        "{}",
        String::from_utf8_lossy(&loser.stderr)
    );
    for output in outputs {
        fixture.assert_private(output, &["provider-must-not-be-consumed"]);
    }
    let shown = successful_json(&fixture.show());
    assert_awaiting_after_cas(&shown, fixture);
    assert_event_chain(&shown);
    assert_eq!(core.arrival_count(), 2);
    fixture.assert_workspace_unchanged();
    fixture.assert_no_network();
}

#[test]
fn sigint_during_pricing_read_stops_before_credential_provider_or_claim() {
    let core = shared_core();
    let fixture = ControllerFixture::new(core);
    let blocked = blocked_step(&fixture, core);
    let output = blocked.interrupt();

    assert_cancelled_before_claim(&fixture, &output);
    let shown = successful_json(&fixture.show());
    assert_interrupted_journal(&shown);
    fixture.assert_workspace_unchanged();
    fixture.assert_no_network();
}

fn blocked_step(fixture: &ControllerFixture, core: &PinnedGoCore) -> BlockedStep {
    let (pricing, pricing_sha256) = exact_pricing(core);
    let started =
        successful_json(&fixture.start_with_pricing(&core.path, &core.sha256, &pricing_sha256));
    let awaiting = &started["awaiting_fresh_consent"];
    let fifo = fixture.source.cwd.path().join("controller-pricing.fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let ready = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let writer = spawn_fifo_writer(&fifo, pricing, ready.clone(), release.clone());

    let mut command = fixture.step_command(
        &core.path,
        &core.sha256,
        &text(&awaiting["awaiting_event_sha256"]),
        &text(&awaiting["provider_request_id"]),
        &text(&awaiting["authorization_sha256"]),
        &fifo,
    );
    let child = command
        .env_remove("OPENAI_API_KEY")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn interruptible controller step");
    assert!(
        wait_for_flag(&ready),
        "controller did not start reading pricing"
    );
    BlockedStep {
        child,
        writer,
        release,
    }
}

fn assert_cancelled_before_claim(fixture: &ControllerFixture, output: &Output) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("execution failed"), "{stderr}");
    assert!(!stderr.contains("credential"), "{stderr}");
    fixture.assert_private(output, &[]);
    assert_empty_owner_directory(fixture);
}

fn assert_interrupted_journal(shown: &Value) {
    assert_eq!(shown["state"], "stopped");
    assert_eq!(shown["stop_reason"], "claimed_unknown");
    assert!(shown["awaiting_fresh_consent"].is_null());
    assert_eq!(shown["effectful_steps_reserved"], 1);
    assert_eq!(shown["cost_usd_micros_reserved"], 10_000);
    assert_eq!(shown["head_sequence"], 8);
    assert!(shown["invocation"].is_null());
    assert_event_chain(shown);
}

struct BlockedStep {
    child: Child,
    writer: thread::JoinHandle<()>,
    release: Arc<AtomicBool>,
}

impl BlockedStep {
    fn interrupt(mut self) -> Output {
        send_sigint(self.child.id());
        thread::sleep(Duration::from_millis(100));
        assert!(
            self.child.try_wait().unwrap().is_none(),
            "SIGINT bypassed safe cancellation"
        );
        self.release.store(true, Ordering::Release);
        self.writer.join().expect("join pricing writer");
        wait(self.child)
    }
}

fn exact_pricing(core: &PinnedGoCore) -> (Vec<u8>, String) {
    let output = Command::new(&core.path)
        .args([
            "graph-node-pricing-snapshot",
            "--model",
            MODEL,
            "--input-usd-micros-per-token-unit",
            "1000000",
            "--output-usd-micros-per-token-unit",
            "976561523",
            "--max-input-tokens",
            "1",
        ])
        .env_clear()
        .output()
        .expect("build exact pricing");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("pricing JSON");
    (output.stdout, text(&value["pricing_snapshot_sha256"]))
}

fn spawn_fifo_writer(
    fifo: &Path,
    pricing: Vec<u8>,
    ready: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    let fifo = fifo.to_owned();
    thread::spawn(move || {
        let mut writer = OpenOptions::new()
            .write(true)
            .open(fifo)
            .expect("open pricing FIFO");
        ready.store(true, Ordering::Release);
        while !release.load(Ordering::Acquire) {
            thread::sleep(Duration::from_millis(5));
        }
        writer
            .write_all(&pricing)
            .expect("write exact pricing to FIFO");
    })
}

fn wait_for_flag(flag: &AtomicBool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if flag.load(Ordering::Acquire) {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

fn send_sigint(pid: u32) {
    let status = Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status()
        .expect("send SIGINT");
    assert!(status.success(), "SIGINT command failed");
}

fn assert_empty_owner_directory(fixture: &ControllerFixture) {
    let directory = fixture
        .source
        .state
        .path()
        .join("scheduled-executor-owners");
    let entries = fs::read_dir(directory)
        .map(std::iter::Iterator::count)
        .unwrap_or_default();
    assert_eq!(entries, 0, "cancellation created an executor owner");
}

fn spawn_advance(fixture: &ControllerFixture, core: &BlockingMaterializationCore) -> Child {
    fixture
        .advance_command(&core.path, &core.sha256)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn controller advance")
}

fn wait(child: Child) -> Output {
    child
        .wait_with_output()
        .expect("wait for controller advance")
}

fn assert_materialization_failure(output: &Output) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("materialization failed"), "{stderr}");
}

fn assert_planned_recovery(value: &Value) {
    assert_eq!(value["state"], "passive_recovery");
    assert_eq!(value["recovery_phase"], "materialize");
    assert_eq!(value["head_sequence"], 2);
    assert_eq!(value["effectful_steps_reserved"], 0);
    assert_eq!(value["cost_usd_micros_reserved"], 0);
}

fn assert_awaiting_after_cas(value: &Value, fixture: &ControllerFixture) {
    assert_eq!(value["state"], "awaiting_fresh_consent");
    assert_eq!(value["graph_run_id"], fixture.source.graph_run_id);
    assert_eq!(value["schedule_sha256"], fixture.schedule_sha256());
    assert_eq!(value["head_sequence"], 6);
    assert_eq!(value["effectful_steps_reserved"], 0);
    assert_eq!(value["cost_usd_micros_reserved"], 0);
    assert!(value["invocation"].is_null());
    let awaiting = &value["awaiting_fresh_consent"];
    assert_eq!(awaiting["node_id"], "build");
    assert_eq!(awaiting["execution_ordinal"], 0);
    assert_eq!(
        awaiting["awaiting_event_sha256"],
        value["head_event_sha256"]
    );
    assert!(text(&awaiting["provider_request_id"]).starts_with("scheduled-node-provider-request-"));
}

struct BlockingMaterializationCore {
    _directory: TempDir,
    path: PathBuf,
    sha256: String,
    fail: PathBuf,
    block: PathBuf,
    arrivals: PathBuf,
}

impl BlockingMaterializationCore {
    fn new(real: &PinnedGoCore) -> Self {
        let directory = tempdir().expect("blocking Core directory");
        let path = directory.path().join("blocking-core");
        let fail = directory.path().join("fail");
        let block = directory.path().join("block");
        let arrivals = directory.path().join("arrivals");
        let script = wrapper_script(&real.path, &fail, &block, &arrivals);
        fs::write(&path, script).expect("write blocking Core");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("make blocking Core executable");
        let path = path.canonicalize().expect("canonical blocking Core");
        let sha256 = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
        Self {
            _directory: directory,
            path,
            sha256,
            fail,
            block,
            arrivals,
        }
    }

    fn fail_materialization(&self) {
        fs::write(&self.fail, b"fail").expect("create materialization failure flag");
    }

    fn allow_materialization(&self) {
        fs::remove_file(&self.fail).expect("remove materialization failure flag");
    }

    fn block_materialization(&self) {
        fs::write(&self.block, b"block").expect("create materialization block flag");
    }

    fn unblock_materialization(&self) {
        fs::remove_file(&self.block).expect("remove materialization block flag");
    }

    fn wait_for_arrivals(&self, expected: usize) -> bool {
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            if self.arrival_count() >= expected {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn arrival_count(&self) -> usize {
        fs::read_to_string(&self.arrivals)
            .unwrap_or_default()
            .lines()
            .count()
    }
}

fn wrapper_script(real: &Path, fail: &Path, block: &Path, arrivals: &Path) -> String {
    let real = shell_quote(real);
    let fail = shell_quote(fail);
    let block = shell_quote(block);
    let arrivals = shell_quote(arrivals);
    format!(
        "#!/bin/sh\n\
         if [ \"$1\" = graph-scheduled-node-contract ] && [ \"$2\" != --protocol-version ]; then\n\
           if [ -e {fail} ]; then exit 97; fi\n\
           printf '%s\\n' arrived >> {arrivals}\n\
           while [ -e {block} ]; do /bin/sleep 0.01; done\n\
         fi\n\
         exec {real} \"$@\"\n"
    )
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}
