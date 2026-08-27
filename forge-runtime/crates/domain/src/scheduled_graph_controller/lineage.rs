use super::{
    ScheduledGraphControllerEventPayload, ScheduledGraphControllerStopReason,
    ScheduledGraphControllerValidationError, validation::invalid,
};

pub(super) fn validate_dispatch_lineage<'a>(
    pending: &mut Option<&'a ScheduledGraphControllerEventPayload>,
    payload: &'a ScheduledGraphControllerEventPayload,
) -> Result<(), ScheduledGraphControllerValidationError> {
    use ScheduledGraphControllerEventPayload as Payload;
    match payload {
        Payload::DispatchPlanned { .. } => *pending = Some(payload),
        Payload::NodeCompleted { .. } => {
            let valid = pending
                .take()
                .is_some_and(|dispatch| completed_matches_dispatch(dispatch, payload));
            if !valid {
                return Err(invalid("controller completion has no pending dispatch"));
            }
        }
        Payload::Completed { .. } if pending.is_some() => {
            return Err(invalid("controller completed with an unresolved dispatch"));
        }
        Payload::Stopped {
            reason,
            provider_request_id,
            snapshot_sha256: None,
            ..
        } if *reason != ScheduledGraphControllerStopReason::IncompatibleSchedule => {
            let valid = pending.as_ref().is_some_and(|dispatch| {
                stopped_matches_dispatch(dispatch, provider_request_id.as_deref())
            });
            if !valid {
                return Err(invalid(
                    "unanchored controller stop has no pending dispatch",
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn stopped_matches_dispatch(
    dispatch: &ScheduledGraphControllerEventPayload,
    stopped_provider: Option<&str>,
) -> bool {
    let ScheduledGraphControllerEventPayload::DispatchPlanned {
        provider_request_id,
        ..
    } = dispatch
    else {
        return false;
    };
    stopped_provider == Some(provider_request_id)
}

fn completed_matches_dispatch(
    dispatch: &ScheduledGraphControllerEventPayload,
    completed: &ScheduledGraphControllerEventPayload,
) -> bool {
    let ScheduledGraphControllerEventPayload::DispatchPlanned {
        execution_ordinal,
        node_id,
        provider_request_id,
        ..
    } = dispatch
    else {
        return false;
    };
    let ScheduledGraphControllerEventPayload::NodeCompleted {
        execution_ordinal: completed_ordinal,
        node_id: completed_node,
        provider_request_id: completed_provider,
        ..
    } = completed
    else {
        return false;
    };
    execution_ordinal == completed_ordinal
        && node_id == completed_node
        && provider_request_id == completed_provider
}
