use std::{
    io::ErrorKind,
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

use super::{LocalResponses, read_request, write_response};

const GATE_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct ResponseGate {
    request_arrived: mpsc::Receiver<()>,
    release_response: mpsc::Sender<()>,
}

impl ResponseGate {
    pub(crate) fn wait_for_request(&self) {
        self.request_arrived
            .recv_timeout(GATE_TIMEOUT)
            .expect("first Agent request must arrive before the response is released");
    }

    pub(crate) fn release(self) {
        self.release_response
            .send(())
            .expect("release the pending first Agent response");
    }
}

pub(super) fn start(response: String) -> (LocalResponses, ResponseGate) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind gated Responses server");
    listener
        .set_nonblocking(true)
        .expect("set bounded gated Responses accept");
    let address = listener.local_addr().expect("gated Responses address");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let (arrival_sender, arrival_receiver) = mpsc::channel();
    let (release_sender, release_receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        serve(
            &listener,
            &response,
            &captured,
            &arrival_sender,
            &release_receiver,
        );
    });
    (
        LocalResponses {
            endpoint: format!("http://{address}/v1"),
            requests,
            worker: Some(worker),
        },
        ResponseGate {
            request_arrived: arrival_receiver,
            release_response: release_sender,
        },
    )
}

fn serve(
    listener: &TcpListener,
    response: &str,
    requests: &Mutex<Vec<Value>>,
    arrived: &mpsc::Sender<()>,
    release: &mpsc::Receiver<()>,
) {
    let Some(mut stream) = accept_request(listener, release) else {
        return;
    };
    let body = read_request(&mut stream);
    requests
        .lock()
        .expect("gated request capture lock")
        .push(serde_json::from_slice(&body).expect("gated Responses request JSON"));
    if arrived.send(()).is_err() {
        return;
    }
    match release.recv_timeout(GATE_TIMEOUT) {
        Ok(()) => write_response(&mut stream, response),
        Err(mpsc::RecvTimeoutError::Disconnected) => {}
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("gated Agent response was not released"),
    }
}

fn accept_request(listener: &TcpListener, release: &mpsc::Receiver<()>) -> Option<TcpStream> {
    let deadline = Instant::now() + GATE_TIMEOUT;
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_nonblocking(false)
                    .expect("set gated request stream blocking");
                return Some(stream);
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {}
            Err(error) => panic!("accept gated Responses request: {error}"),
        }
        match release.try_recv() {
            Err(mpsc::TryRecvError::Disconnected) => return None,
            Err(mpsc::TryRecvError::Empty) => {}
            Ok(()) => panic!("gated response released before request arrival"),
        }
        assert!(
            Instant::now() < deadline,
            "gated Agent request did not arrive"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
