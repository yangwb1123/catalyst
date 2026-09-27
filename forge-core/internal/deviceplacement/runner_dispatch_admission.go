package deviceplacement

// RunnerDispatchAdmissionSchemaVersion identifies the bounded, read-only
// boundary between a durable fenced lease and a future Runner transport.
const RunnerDispatchAdmissionSchemaVersion = "forge.runner-dispatch-admission/v1"

// RunnerDispatchAdmissionEvaluationMode makes it explicit that this value is
// a recheck and preview. It does not authorize a command or contact a Runner.
const RunnerDispatchAdmissionEvaluationMode = "durable_lease_bound_dispatch_admission_preview"

// RunnerDispatchAdmissionRequest carries the command declaration that a
// caller wants to dispatch. The command contains a lease proof, but this
// package only compares it with the separately supplied registry observation.
type RunnerDispatchAdmissionRequest struct {
	Owner          Owner                  `json:"owner"`
	ConversationID string                 `json:"conversation_id"`
	RunID          string                 `json:"run_id"`
	AttemptID      string                 `json:"attempt_id"`
	AttemptState   string                 `json:"attempt_state"`
	Command        RunnerExecutionCommand `json:"command"`
	EvaluatedAtMS  uint64                 `json:"evaluated_at_ms"`
}

// RunnerDispatchAdmissionLease is the metadata read from the durable lease
// registry. FencingToken is deliberately absent so the observation can be
// rendered or forwarded without exposing a reusable proof.
type RunnerDispatchAdmissionLease struct {
	TargetID    string `json:"target_id"`
	Epoch       uint64 `json:"epoch"`
	IssuedAtMS  uint64 `json:"issued_at_ms"`
	ExpiresAtMS uint64 `json:"expires_at_ms"`
	Current     bool   `json:"current"`
	Active      bool   `json:"active"`
}

// RunnerDispatchAdmissionAuthority records capabilities that remain absent
// at this boundary. Every field is fixed false for a valid observation.
type RunnerDispatchAdmissionAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// RunnerDispatchAdmissionObservation is the metadata-only result of checking
// one command against the current fenced lease. It contains no token, argv,
// workspace, output, or Runner response.
type RunnerDispatchAdmissionObservation struct {
	SchemaVersion          string                           `json:"schema_version"`
	EvaluationMode         string                           `json:"evaluation_mode"`
	Owner                  Owner                            `json:"owner"`
	ConversationID         string                           `json:"conversation_id"`
	RunID                  string                           `json:"run_id"`
	AttemptID              string                           `json:"attempt_id"`
	AttemptState           string                           `json:"attempt_state"`
	AttemptStateAdmissible bool                             `json:"attempt_state_admissible"`
	CommandID              string                           `json:"command_id"`
	CommandSHA256          string                           `json:"command_sha256"`
	TargetID               string                           `json:"target_id"`
	LeaseEpoch             uint64                           `json:"lease_epoch"`
	LeaseIssuedAtMS        uint64                           `json:"lease_issued_at_ms"`
	LeaseExpiresAtMS       uint64                           `json:"lease_expires_at_ms"`
	EvaluatedAtMS          uint64                           `json:"evaluated_at_ms"`
	LeaseProofCurrent      bool                             `json:"lease_proof_current"`
	LeaseActive            bool                             `json:"lease_active"`
	CommandBindingValid    bool                             `json:"command_binding_valid"`
	AdmissionReady         bool                             `json:"admission_ready"`
	RejectionReasons       []string                         `json:"rejection_reasons"`
	PreviewOnly            bool                             `json:"preview_only"`
	Authority              RunnerDispatchAdmissionAuthority `json:"authority"`
}

// ObserveRunnerDispatchAdmission validates the command declaration and joins
// it with a caller-held current lease observation. It never reads a clock,
// persists state, opens Runner transport, or grants execution authority.
func ObserveRunnerDispatchAdmission(
	input RunnerDispatchAdmissionRequest,
	lease RunnerDispatchAdmissionLease,
) (RunnerDispatchAdmissionObservation, error) {
	if !validOwner(input.Owner) ||
		!validSessionIdentifier(input.ConversationID) ||
		!validSessionIdentifier(input.RunID) ||
		!validSessionIdentifier(input.AttemptID) ||
		!validDispatchAttemptState(input.AttemptState) ||
		input.EvaluatedAtMS == 0 || input.EvaluatedAtMS > uint64(MaxSafeIntegerMS) {
		return RunnerDispatchAdmissionObservation{}, errInvalidRequest
	}
	commandSHA256, commandErr := input.Command.commandSHA256()
	commandBindingValid := commandErr == nil &&
		input.Command.LeaseProof.AttemptID == input.AttemptID &&
		validSessionIdentifier(input.Command.LeaseProof.TargetID)
	if !commandBindingValid {
		commandSHA256 = ""
	}
	if lease.TargetID == "" || lease.Epoch == 0 || lease.IssuedAtMS == 0 ||
		lease.ExpiresAtMS <= lease.IssuedAtMS || lease.ExpiresAtMS > uint64(MaxSafeIntegerMS) ||
		lease.TargetID != input.Command.LeaseProof.TargetID ||
		lease.Epoch != input.Command.LeaseProof.Epoch {
		return RunnerDispatchAdmissionObservation{}, errInvalidRequest
	}
	attemptStateAdmissible := dispatchableAttemptState(input.AttemptState)
	reasons := dispatchAdmissionRejectionReasons(
		commandBindingValid, lease.Current, lease.Active, attemptStateAdmissible,
	)
	return RunnerDispatchAdmissionObservation{
		SchemaVersion:          RunnerDispatchAdmissionSchemaVersion,
		EvaluationMode:         RunnerDispatchAdmissionEvaluationMode,
		Owner:                  input.Owner,
		ConversationID:         input.ConversationID,
		RunID:                  input.RunID,
		AttemptID:              input.AttemptID,
		AttemptState:           input.AttemptState,
		AttemptStateAdmissible: attemptStateAdmissible,
		CommandID:              input.Command.CommandID,
		CommandSHA256:          commandSHA256,
		TargetID:               input.Command.LeaseProof.TargetID,
		LeaseEpoch:             lease.Epoch,
		LeaseIssuedAtMS:        lease.IssuedAtMS,
		LeaseExpiresAtMS:       lease.ExpiresAtMS,
		EvaluatedAtMS:          input.EvaluatedAtMS,
		LeaseProofCurrent:      lease.Current,
		LeaseActive:            lease.Active,
		CommandBindingValid:    commandBindingValid,
		AdmissionReady:         commandBindingValid && lease.Current && lease.Active && attemptStateAdmissible,
		RejectionReasons:       reasons,
		PreviewOnly:            true,
		Authority:              RunnerDispatchAdmissionAuthority{},
	}, nil
}

// Validate prevents a client from treating a recheck as command authority.
func (observation RunnerDispatchAdmissionObservation) Validate() error {
	if observation.SchemaVersion != RunnerDispatchAdmissionSchemaVersion ||
		observation.EvaluationMode != RunnerDispatchAdmissionEvaluationMode ||
		!validOwner(observation.Owner) ||
		!validSessionIdentifier(observation.ConversationID) ||
		!validSessionIdentifier(observation.RunID) ||
		!validSessionIdentifier(observation.AttemptID) ||
		!validDispatchAttemptState(observation.AttemptState) ||
		observation.AttemptStateAdmissible != dispatchableAttemptState(observation.AttemptState) ||
		!validSessionIdentifier(observation.CommandID) ||
		!validRunnerDigest(observation.CommandSHA256) ||
		!validSessionIdentifier(observation.TargetID) || observation.LeaseEpoch == 0 ||
		observation.LeaseIssuedAtMS == 0 || observation.LeaseExpiresAtMS <= observation.LeaseIssuedAtMS ||
		observation.LeaseExpiresAtMS > uint64(MaxSafeIntegerMS) || observation.EvaluatedAtMS == 0 ||
		observation.EvaluatedAtMS > uint64(MaxSafeIntegerMS) || !observation.PreviewOnly ||
		observation.Authority != (RunnerDispatchAdmissionAuthority{}) ||
		observation.AdmissionReady != (observation.CommandBindingValid && observation.LeaseProofCurrent && observation.LeaseActive && observation.AttemptStateAdmissible) ||
		!samePreflightStrings(observation.RejectionReasons, dispatchAdmissionRejectionReasons(
			observation.CommandBindingValid, observation.LeaseProofCurrent,
			observation.LeaseActive, observation.AttemptStateAdmissible,
		)) {
		return errInvalidRequest
	}
	if observation.AdmissionReady && len(observation.RejectionReasons) != 0 {
		return errInvalidRequest
	}
	return nil
}

func dispatchAdmissionRejectionReasons(commandBindingValid, leaseCurrent, leaseActive, attemptStateAdmissible bool) []string {
	reasons := make([]string, 0, 4)
	if !commandBindingValid {
		reasons = append(reasons, "command_binding_invalid")
	}
	if !leaseCurrent {
		reasons = append(reasons, "lease_proof_not_current")
	}
	if !leaseActive {
		reasons = append(reasons, "lease_inactive_at_evaluated_time")
	}
	if !attemptStateAdmissible {
		reasons = append(reasons, "attempt_state_not_dispatchable")
	}
	return sortedUniqueStrings(reasons)
}
