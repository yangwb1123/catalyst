use std::{
    env, fs,
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};

use rusqlite::Connection;

use crate::runtime_domain::{
    AppendScheduledGraphControllerDisposition, AppendScheduledGraphControllerResult, HubStoreError,
    ScheduledGraphControllerEvent, ScheduledGraphControllerEventPayload,
    ScheduledGraphControllerStore,
};

use super::super::write;
use super::{controller_fixture, event, start};

const PROCESS_CHILD_MARKER: &str = "FORGE_CONTROLLER_CAS_CHILD";
const PROCESS_DATABASE: &str = "FORGE_CONTROLLER_CAS_DATABASE";
const PROCESS_EVENT: &str = "FORGE_CONTROLLER_CAS_EVENT";
const PROCESS_OUTCOME: &str = "FORGE_CONTROLLER_CAS_OUTCOME";
const PROCESS_BARRIER: &str = "FORGE_CONTROLLER_CAS_BARRIER";

#[test]
fn two_connections_compare_and_append_exactly_one_same_sequence_winner() {
    let (fixture, header, started) = controller_fixture();
    start(&fixture, &header, &started);
    let left = terminal_event(&header, &started, "5", "6");
    let right = terminal_event(&header, &started, "7", "8");
    let barrier = Arc::new(Barrier::new(2));
    let left_thread = append_thread(
        fixture.graph.database.clone(),
        left.clone(),
        barrier.clone(),
    );
    let right_thread = append_thread(fixture.graph.database.clone(), right.clone(), barrier);

    let results = [
        left_thread.join().expect("left append thread"),
        right_thread.join().expect("right append thread"),
    ];
    assert_eq!(stored_count(&results), 1);
    assert_eq!(conflict_count(&results), 1);
    let durable = fixture
        .graph
        .store
        .inspect_scheduled_graph_controller(&header.graph_run_id)
        .expect("inspect append winner");
    assert!(durable.head() == &left || durable.head() == &right);
}

#[test]
fn two_processes_compare_and_append_exactly_one_same_sequence_winner() {
    let (fixture, header, started) = controller_fixture();
    start(&fixture, &header, &started);
    let left = terminal_event(&header, &started, "5", "6");
    let right = terminal_event(&header, &started, "7", "8");
    let root = tempfile::tempdir().expect("process contender coordination root");
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind process barrier");
    let address = listener.local_addr().expect("process barrier address");
    let left_child = spawn_process_contender(
        &fixture.graph.database,
        &left,
        root.path().join("left-event.json"),
        root.path().join("left-outcome"),
        address.to_string(),
    );
    let right_child = spawn_process_contender(
        &fixture.graph.database,
        &right,
        root.path().join("right-event.json"),
        root.path().join("right-outcome"),
        address.to_string(),
    );
    release_process_barrier(&listener);
    let outputs = wait_for_children(left_child, right_child);
    assert_child_succeeded("left", &outputs[0]);
    assert_child_succeeded("right", &outputs[1]);
    let outcomes = [
        fs::read_to_string(root.path().join("left-outcome")).expect("left outcome"),
        fs::read_to_string(root.path().join("right-outcome")).expect("right outcome"),
    ];
    assert_eq!(
        outcomes.iter().filter(|value| *value == "stored").count(),
        1
    );
    assert_eq!(
        outcomes.iter().filter(|value| *value == "conflict").count(),
        1
    );
    let durable = fixture
        .graph
        .store
        .inspect_scheduled_graph_controller(&header.graph_run_id)
        .expect("inspect process append winner");
    assert!(durable.head() == &left || durable.head() == &right);
}

#[test]
fn scheduled_graph_controller_append_process_contender() {
    if env::var_os(PROCESS_CHILD_MARKER).is_none() {
        return;
    }
    let database = required_process_path(PROCESS_DATABASE);
    let event_json = fs::read_to_string(required_process_path(PROCESS_EVENT))
        .expect("read process contender event");
    let candidate = ScheduledGraphControllerEvent::decode_exact(&event_json)
        .expect("decode process contender event");
    let mut connection = Connection::open(database).expect("open process contender connection");
    connection
        .busy_timeout(Duration::from_secs(5))
        .expect("configure process contender busy timeout");
    let mut barrier = TcpStream::connect(required_process_value(PROCESS_BARRIER))
        .expect("connect process barrier");
    barrier.write_all(&[1]).expect("signal process ready");
    let mut release = [0_u8; 1];
    barrier
        .read_exact(&mut release)
        .expect("await process release");
    assert_eq!(release, [1]);
    let outcome = match write::append(&mut connection, &candidate) {
        Ok(value) if value.disposition == AppendScheduledGraphControllerDisposition::Stored => {
            "stored"
        }
        Err(HubStoreError::Conflict { .. }) => "conflict",
        other => panic!("unexpected process contender result: {other:?}"),
    };
    fs::write(required_process_path(PROCESS_OUTCOME), outcome)
        .expect("write process contender outcome");
}

fn terminal_event(
    header: &crate::runtime_domain::ScheduledGraphControllerHeader,
    started: &ScheduledGraphControllerEvent,
    snapshot_digit: &str,
    decision_digit: &str,
) -> ScheduledGraphControllerEvent {
    event(
        header,
        2,
        Some(started.event_sha256.clone()),
        ScheduledGraphControllerEventPayload::Completed {
            snapshot_sha256: snapshot_digit.repeat(64),
            decision_sha256: decision_digit.repeat(64),
        },
    )
}

fn append_thread(
    database: PathBuf,
    candidate: ScheduledGraphControllerEvent,
    barrier: Arc<Barrier>,
) -> thread::JoinHandle<Result<AppendScheduledGraphControllerResult, HubStoreError>> {
    thread::spawn(move || {
        let mut connection = Connection::open(database).expect("open independent connection");
        connection
            .busy_timeout(Duration::from_secs(5))
            .expect("configure busy timeout");
        barrier.wait();
        write::append(&mut connection, &candidate)
    })
}

fn spawn_process_contender(
    database: &std::path::Path,
    candidate: &ScheduledGraphControllerEvent,
    event_path: PathBuf,
    outcome_path: PathBuf,
    barrier: String,
) -> Child {
    fs::write(
        &event_path,
        candidate
            .canonical_json()
            .expect("encode process contender event"),
    )
    .expect("write process contender event");
    Command::new(env::current_exe().expect("current test executable"))
        .arg("scheduled_graph_controller_append_process_contender")
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env(PROCESS_CHILD_MARKER, "1")
        .env(PROCESS_DATABASE, database)
        .env(PROCESS_EVENT, event_path)
        .env(PROCESS_OUTCOME, outcome_path)
        .env(PROCESS_BARRIER, barrier)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn process contender")
}

fn release_process_barrier(listener: &TcpListener) {
    listener
        .set_nonblocking(true)
        .expect("configure process barrier");
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut contenders = Vec::with_capacity(2);
    while contenders.len() < 2 {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut ready = [0_u8; 1];
                stream.read_exact(&mut ready).expect("process ready signal");
                assert_eq!(ready, [1]);
                contenders.push(stream);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "process barrier timed out");
                thread::yield_now();
            }
            Err(error) => panic!("accept process contender: {error}"),
        }
    }
    for contender in &mut contenders {
        contender
            .write_all(&[1])
            .expect("release process contender");
    }
}

fn wait_for_children(left: Child, right: Child) -> [Output; 2] {
    [
        left.wait_with_output().expect("wait for left contender"),
        right.wait_with_output().expect("wait for right contender"),
    ]
}

fn assert_child_succeeded(label: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{label} contender failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn required_process_path(name: &str) -> PathBuf {
    PathBuf::from(required_process_value(name))
}

fn required_process_value(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("missing process contender environment: {name}"))
}

fn stored_count(
    results: &[Result<AppendScheduledGraphControllerResult, HubStoreError>; 2],
) -> usize {
    results
        .iter()
        .filter(|result| {
            matches!(
                result,
                Ok(value)
                    if value.disposition == AppendScheduledGraphControllerDisposition::Stored
            )
        })
        .count()
}

fn conflict_count(
    results: &[Result<AppendScheduledGraphControllerResult, HubStoreError>; 2],
) -> usize {
    results
        .iter()
        .filter(|result| matches!(result, Err(HubStoreError::Conflict { .. })))
        .count()
}
