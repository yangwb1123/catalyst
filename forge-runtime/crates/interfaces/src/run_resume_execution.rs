use std::{error::Error, io};

use forge_runtime_infrastructure::{DurableFirstEventSink, JsonlEventSink};

use super::{
    Cancellation, HumanEventSink, PreparedResume, RunOutcome, cancellation_listener,
    reconcile_terminal_by_id,
};

pub(super) async fn execute_resume(
    prepared: PreparedResume,
    run_id: &str,
    human_output: bool,
) -> Result<RunOutcome, Box<dyn Error>> {
    let cancellation = Cancellation::default();
    let listener = cancellation_listener(cancellation.clone());
    let stdout = io::stdout();
    let result = if human_output {
        let mut downstream = HumanEventSink::new(stdout.lock());
        let mut sink = DurableFirstEventSink::new(prepared.store.as_ref(), &mut downstream);
        prepared
            .runtime
            .resume_with_inspection(
                prepared.request,
                prepared.inspection,
                prepared.history,
                cancellation,
                &mut sink,
            )
            .await
    } else {
        let mut downstream = JsonlEventSink::new(stdout.lock());
        let mut sink = DurableFirstEventSink::new(prepared.store.as_ref(), &mut downstream);
        prepared
            .runtime
            .resume_with_inspection(
                prepared.request,
                prepared.inspection,
                prepared.history,
                cancellation,
                &mut sink,
            )
            .await
    };
    listener.abort();
    match result {
        Ok(_) => reconcile_terminal_by_id(&prepared.store, run_id),
        Err(error) => {
            let _ = reconcile_terminal_by_id(&prepared.store, run_id);
            Err(Box::new(error))
        }
    }
}
