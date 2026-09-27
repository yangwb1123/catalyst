package deviceplacement

import (
	"strings"

	"forgeos/forge-core/internal/runnertransport"
)

// RunnerTransportAdmissionSchemaVersion identifies the value-only join
// between a current fenced lease, a bounded command, and an already verified
// D3 Runner transport envelope.
const RunnerTransportAdmissionSchemaVersion = "forge.runner-transport-admission/v1"

// RunnerTransportAdmissionEvaluationMode makes clear that this is an
// admission preview. It does not contact a Runner or authorize execution.
const RunnerTransportAdmissionEvaluationMode = "fenced_runner_transport_admission_preview"

// RunnerTransportAdmissionRequest contains caller-supplied command and
// transport observations. The transport observation must have been produced
// by runnertransport.Verify; this adapter only joins its metadata to the
// exact command/lease binding.
type RunnerTransportAdmissionRequest struct {
	Owner                 Owner                        `json:"owner"`
	ConversationID        string                       `json:"conversation_id"`
	RunID                 string                       `json:"run_id"`
	AttemptID             string                       `json:"attempt_id"`
	AttemptState          string                       `json:"attempt_state"`
	Command               RunnerExecutionCommand       `json:"command"`
	Lease                 RunnerDispatchAdmissionLease `json:"lease"`
	Transport             runnertransport.Observation  `json:"transport"`
	ExpectedPayloadSHA256 string                       `json:"expected_payload_sha256"`
	EvaluatedAtMS         uint64                       `json:"evaluated_at_ms"`
}

// RunnerTransportAdmissionAuthority is intentionally all false. Transport
// authentication is not device identity, reservation, execution, dispatch,
// or Audit authority.
type RunnerTransportAdmissionAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	TransportAuthenticated bool `json:"transport_authenticated"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// RunnerTransportAdmissionObservation is safe to render or forward. It
// contains no fencing token, argv, workspace, payload bytes, or output.
type RunnerTransportAdmissionObservation struct {
	SchemaVersion          string                            `json:"schema_version"`
	EvaluationMode         string                            `json:"evaluation_mode"`
	Owner                  Owner                             `json:"owner"`
	ConversationID         string                            `json:"conversation_id"`
	RunID                  string                            `json:"run_id"`
	AttemptID              string                            `json:"attempt_id"`
	AttemptState           string                            `json:"attempt_state"`
	AttemptStateAdmissible bool                              `json:"attempt_state_admissible"`
	CommandID              string                            `json:"command_id"`
	CommandSHA256          string                            `json:"command_sha256"`
	TargetID               string                            `json:"target_id"`
	LeaseEpoch             uint64                            `json:"lease_epoch"`
	LeaseIssuedAtMS        uint64                            `json:"lease_issued_at_ms"`
	LeaseExpiresAtMS       uint64                            `json:"lease_expires_at_ms"`
	EvaluatedAtMS          uint64                            `json:"evaluated_at_ms"`
	TransportMethod        string                            `json:"transport_method"`
	TransportPath          string                            `json:"transport_path"`
	TransportTimestamp     int64                             `json:"transport_timestamp"`
	TransportNonce         string                            `json:"transport_nonce"`
	TransportPayloadSHA256 string                            `json:"transport_payload_sha256"`
	TransportPayloadBytes  int                               `json:"transport_payload_bytes"`
	TransportReplayChecked bool                              `json:"transport_replay_checked"`
	LeaseProofCurrent      bool                              `json:"lease_proof_current"`
	LeaseActive            bool                              `json:"lease_active"`
	CommandBindingValid    bool                              `json:"command_binding_valid"`
	TransportBindingValid  bool                              `json:"transport_binding_valid"`
	AdmissionReady         bool                              `json:"admission_ready"`
	RejectionReasons       []string                          `json:"rejection_reasons"`
	PreviewOnly            bool                              `json:"preview_only"`
	Authority              RunnerTransportAdmissionAuthority `json:"authority"`
}

// ObserveRunnerTransportAdmission combines the current lease/command
// admission with a verified D3 transport observation. It has no clock or I/O
// side effect and never turns readiness into execution authority.
func ObserveRunnerTransportAdmission(
	input RunnerTransportAdmissionRequest,
) (RunnerTransportAdmissionObservation, error) {
	if !validOwner(input.Owner) ||
		!validSessionIdentifier(input.ConversationID) ||
		!validSessionIdentifier(input.RunID) ||
		!validSessionIdentifier(input.AttemptID) ||
		!validDispatchAttemptState(input.AttemptState) ||
		input.EvaluatedAtMS == 0 || input.EvaluatedAtMS > uint64(MaxSafeIntegerMS) ||
		!validRunnerDigest(input.ExpectedPayloadSHA256) {
		return RunnerTransportAdmissionObservation{}, errInvalidRequest
	}
	if err := input.Transport.Validate(); err != nil {
		return RunnerTransportAdmissionObservation{}, errInvalidRequest
	}
	admission, err := ObserveRunnerDispatchAdmission(
		RunnerDispatchAdmissionRequest{
			Owner: input.Owner, ConversationID: input.ConversationID, RunID: input.RunID,
			AttemptID: input.AttemptID, AttemptState: input.AttemptState,
			Command: input.Command, EvaluatedAtMS: input.EvaluatedAtMS,
		},
		input.Lease,
	)
	if err != nil {
		return RunnerTransportAdmissionObservation{}, err
	}
	transportBindingValid := input.Transport.Method == "POST" &&
		input.Transport.Path == runnerDispatchTransportPath(input.Command.LeaseProof.TargetID) &&
		input.Transport.PayloadSHA256 == input.ExpectedPayloadSHA256
	reasons := append([]string{}, admission.RejectionReasons...)
	if !transportBindingValid {
		reasons = append(reasons, "transport_binding_invalid")
	}
	reasons = sortedUniqueStrings(reasons)
	return RunnerTransportAdmissionObservation{
		SchemaVersion:  RunnerTransportAdmissionSchemaVersion,
		EvaluationMode: RunnerTransportAdmissionEvaluationMode,
		Owner:          input.Owner, ConversationID: input.ConversationID, RunID: input.RunID,
		AttemptID: input.AttemptID, AttemptState: input.AttemptState,
		AttemptStateAdmissible: admission.AttemptStateAdmissible,
		CommandID:              admission.CommandID, CommandSHA256: admission.CommandSHA256,
		TargetID: admission.TargetID, LeaseEpoch: admission.LeaseEpoch,
		LeaseIssuedAtMS: admission.LeaseIssuedAtMS, LeaseExpiresAtMS: admission.LeaseExpiresAtMS,
		EvaluatedAtMS:   admission.EvaluatedAtMS,
		TransportMethod: input.Transport.Method, TransportPath: input.Transport.Path,
		TransportTimestamp: input.Transport.Timestamp, TransportNonce: input.Transport.Nonce,
		TransportPayloadSHA256: input.Transport.PayloadSHA256,
		TransportPayloadBytes:  input.Transport.PayloadBytes,
		TransportReplayChecked: input.Transport.ReplayChecked,
		LeaseProofCurrent:      admission.LeaseProofCurrent, LeaseActive: admission.LeaseActive,
		CommandBindingValid:   admission.CommandBindingValid,
		TransportBindingValid: transportBindingValid,
		AdmissionReady:        admission.AdmissionReady && transportBindingValid,
		RejectionReasons:      reasons, PreviewOnly: true,
		Authority: RunnerTransportAdmissionAuthority{},
	}, nil
}

// Validate rejects any transported value that could be mistaken for a live
// dispatch grant.
func (observation RunnerTransportAdmissionObservation) Validate() error {
	transport := runnertransport.Observation{
		SchemaVersion:  runnertransport.SchemaVersion,
		EvaluationMode: runnertransport.EvaluationMode,
		Method:         observation.TransportMethod,
		Path:           observation.TransportPath,
		Timestamp:      observation.TransportTimestamp,
		Nonce:          observation.TransportNonce,
		PayloadSHA256:  observation.TransportPayloadSHA256,
		PayloadBytes:   observation.TransportPayloadBytes,
		ReplayChecked:  observation.TransportReplayChecked,
		PreviewOnly:    true,
	}
	if transport.Validate() != nil {
		return errInvalidRequest
	}
	if observation.SchemaVersion != RunnerTransportAdmissionSchemaVersion ||
		observation.EvaluationMode != RunnerTransportAdmissionEvaluationMode ||
		!validOwner(observation.Owner) ||
		!validSessionIdentifier(observation.ConversationID) ||
		!validSessionIdentifier(observation.RunID) ||
		!validSessionIdentifier(observation.AttemptID) ||
		!validDispatchAttemptState(observation.AttemptState) ||
		observation.AttemptStateAdmissible != dispatchableAttemptState(observation.AttemptState) ||
		!validSessionIdentifier(observation.CommandID) ||
		!validRunnerDigest(observation.CommandSHA256) ||
		!validSessionIdentifier(observation.TargetID) || observation.LeaseEpoch == 0 ||
		observation.LeaseEpoch > uint64(MaxSafeIntegerMS) || observation.LeaseIssuedAtMS == 0 ||
		observation.LeaseIssuedAtMS > uint64(MaxSafeIntegerMS) || observation.LeaseExpiresAtMS <= observation.LeaseIssuedAtMS ||
		observation.LeaseExpiresAtMS > uint64(MaxSafeIntegerMS) || observation.EvaluatedAtMS == 0 ||
		observation.EvaluatedAtMS > uint64(MaxSafeIntegerMS) || observation.TransportMethod != "POST" ||
		observation.TransportPath != runnerDispatchTransportPath(observation.TargetID) ||
		observation.TransportTimestamp <= 0 || observation.TransportTimestamp > MaxSafeIntegerMS ||
		!validRunnerDigest(observation.TransportPayloadSHA256) || observation.TransportPayloadBytes <= 0 ||
		observation.TransportPayloadBytes > runnertransport.MaxPayloadBytes ||
		!observation.PreviewOnly || observation.Authority != (RunnerTransportAdmissionAuthority{}) ||
		observation.AdmissionReady != (observation.CommandBindingValid && observation.LeaseProofCurrent && observation.LeaseActive && observation.AttemptStateAdmissible && observation.TransportBindingValid) ||
		!samePreflightStrings(observation.RejectionReasons, transportAdmissionRejectionReasons(observation.CommandBindingValid, observation.LeaseProofCurrent, observation.LeaseActive, observation.AttemptStateAdmissible, observation.TransportBindingValid)) {
		return errInvalidRequest
	}
	if observation.AdmissionReady && len(observation.RejectionReasons) != 0 {
		return errInvalidRequest
	}
	return nil
}

func runnerDispatchTransportPath(targetID string) string {
	return "/api/v1/runners/" + targetID + "/dispatch"
}

func transportAdmissionRejectionReasons(commandBindingValid, leaseCurrent, leaseActive, attemptStateAdmissible, transportBindingValid bool) []string {
	reasons := make([]string, 0, 5)
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
	if !transportBindingValid {
		reasons = append(reasons, "transport_binding_invalid")
	}
	return sortedUniqueStrings(reasons)
}

// TransportPayloadBindingPath is intentionally exported for contract
// producers without exposing command or fencing bytes.
func TransportPayloadBindingPath(targetID string) string {
	return runnerDispatchTransportPath(strings.TrimSpace(targetID))
}
