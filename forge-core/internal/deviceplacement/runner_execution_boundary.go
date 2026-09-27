package deviceplacement

import (
	"sort"
	"strings"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/runnertransport"
)

// RunnerExecutionBoundarySchemaVersion identifies the final pure join before
// a future Runner effect adapter. It is deliberately not a dispatch route.
const RunnerExecutionBoundarySchemaVersion = "forge.runner-execution-boundary/v1"

// RunnerExecutionBoundaryEvaluationMode makes the no-effect property visible
// to every consumer of this value.
const RunnerExecutionBoundaryEvaluationMode = "p4_runner_authority_execution_boundary_preview"

// RunnerExecutionBoundaryControls are caller-supplied lifecycle observations.
// A future authoritative implementation must source them from the Attempt,
// cancellation, and reconciliation stores; this pure adapter never infers or
// mutates them.
type RunnerExecutionBoundaryControls struct {
	EffectState           string `json:"effect_state"`
	CancellationRequested bool   `json:"cancellation_requested"`
}

// RunnerExecutionBoundaryRequest joins the already validated lease/transport
// previews with both activation gates and explicit effect controls.
type RunnerExecutionBoundaryRequest struct {
	Activation devicefabricgate.Request               `json:"activation"`
	Authority  devicefabricgate.RunnerAuthorityConfig `json:"authority"`
	Dispatch   RunnerDispatchAdmissionObservation     `json:"dispatch_admission"`
	Transport  RunnerTransportAdmissionObservation    `json:"transport_admission"`
	Controls   RunnerExecutionBoundaryControls        `json:"controls"`
}

// RunnerExecutionBoundaryPreviewRequest is the authenticated API projection
// of a boundary request. Activation and Runner authority are server-owned
// configuration and therefore never arrive from the caller. The command
// still carries a fencing proof so the route can re-read the exact lease;
// that proof is never copied to the observation.
type RunnerExecutionBoundaryPreviewRequest struct {
	Owner                 Owner                           `json:"owner"`
	ConversationID        string                          `json:"conversation_id"`
	RunID                 string                          `json:"run_id"`
	AttemptID             string                          `json:"attempt_id"`
	AttemptState          string                          `json:"attempt_state"`
	Command               RunnerExecutionCommand          `json:"command"`
	Transport             runnertransport.Observation     `json:"transport"`
	ExpectedPayloadSHA256 string                          `json:"expected_payload_sha256"`
	Controls              RunnerExecutionBoundaryControls `json:"controls"`
}

// RunnerExecutionBoundaryAuthority is fixed false. A boundary-ready
// observation is still a preview and never authorizes, reserves, dispatches,
// or audits a command.
type RunnerExecutionBoundaryAuthority struct {
	DeviceIdentityVerified bool `json:"device_identity_verified"`
	CommandPersisted       bool `json:"command_persisted"`
	ReservationCreated     bool `json:"reservation_created"`
	ExecutionAuthorized    bool `json:"execution_authorized"`
	DispatchPerformed      bool `json:"dispatch_performed"`
	AuditPublished         bool `json:"audit_published"`
}

// RunnerExecutionBoundaryObservation is safe to render or forward. It has
// no command argv, workspace, fencing token, payload, or Runner response.
type RunnerExecutionBoundaryObservation struct {
	SchemaVersion           string                           `json:"schema_version"`
	EvaluationMode          string                           `json:"evaluation_mode"`
	Mode                    devicefabricgate.Mode            `json:"mode"`
	Owner                   Owner                            `json:"owner"`
	ConversationID          string                           `json:"conversation_id"`
	RunID                   string                           `json:"run_id"`
	AttemptID               string                           `json:"attempt_id"`
	AttemptState            string                           `json:"attempt_state"`
	CommandID               string                           `json:"command_id"`
	CommandSHA256           string                           `json:"command_sha256"`
	TargetID                string                           `json:"target_id"`
	LeaseEpoch              uint64                           `json:"lease_epoch"`
	ActivationAllowed       bool                             `json:"activation_allowed"`
	RunnerAuthorityAccepted bool                             `json:"runner_authority_accepted"`
	DispatchAdmissionReady  bool                             `json:"dispatch_admission_ready"`
	TransportAdmissionReady bool                             `json:"transport_admission_ready"`
	EffectState             string                           `json:"effect_state"`
	EffectStateStartable    bool                             `json:"effect_state_startable"`
	CancellationClear       bool                             `json:"cancellation_clear"`
	ExecutionBoundaryReady  bool                             `json:"execution_boundary_ready"`
	RejectionReasons        []string                         `json:"rejection_reasons"`
	PreviewOnly             bool                             `json:"preview_only"`
	Authority               RunnerExecutionBoundaryAuthority `json:"authority"`
}

// ObserveRunnerExecutionBoundary performs the last metadata-only join before
// a future live Runner adapter. It validates both admission observations,
// re-evaluates P4 plus the independent Runner authority decision, and blocks
// cancellation or an unreconciled uncertain effect.
func ObserveRunnerExecutionBoundary(
	input RunnerExecutionBoundaryRequest,
) (RunnerExecutionBoundaryObservation, error) {
	if err := validateRunnerExecutionBoundaryInputs(input); err != nil {
		return RunnerExecutionBoundaryObservation{}, err
	}
	gate := devicefabricgate.EvaluateRunnerExecution(devicefabricgate.RunnerExecutionGateRequest{
		Activation: input.Activation,
		Authority:  input.Authority,
	})
	activationAllowed := len(devicefabricgate.Evaluate(input.Activation).Reasons) == 0
	runnerAuthorityAccepted := runnerAuthorityAccepted(input.Authority, input.Activation.P4.AcceptanceID)
	effectStateStartable := input.Controls.EffectState == "not_started" || input.Controls.EffectState == "reconciled"
	cancellationClear := !input.Controls.CancellationRequested
	// Keep the wire shape canonical for strict clients: an accepted preview
	// carries an empty JSON array rather than a nil slice serialized as null.
	reasons := append([]string{}, gate.Reasons...)
	if !input.Dispatch.AdmissionReady {
		reasons = append(reasons, "dispatch_admission_not_ready")
		reasons = append(reasons, input.Dispatch.RejectionReasons...)
	}
	if !input.Transport.AdmissionReady {
		reasons = append(reasons, "transport_admission_not_ready")
		reasons = append(reasons, input.Transport.RejectionReasons...)
	}
	if !cancellationClear {
		reasons = append(reasons, "cancellation_requested")
	}
	if !effectStateStartable {
		if input.Controls.EffectState == "uncertain" {
			reasons = append(reasons, "uncertain_effect_requires_reconciliation")
		} else {
			reasons = append(reasons, "effect_state_not_startable")
		}
	}
	sort.Strings(reasons)
	reasons = uniqueRunnerExecutionBoundaryReasons(reasons)
	return RunnerExecutionBoundaryObservation{
		SchemaVersion: RunnerExecutionBoundarySchemaVersion, EvaluationMode: RunnerExecutionBoundaryEvaluationMode,
		Mode: gate.Mode, Owner: input.Dispatch.Owner, ConversationID: input.Dispatch.ConversationID,
		RunID: input.Dispatch.RunID, AttemptID: input.Dispatch.AttemptID, AttemptState: input.Dispatch.AttemptState,
		CommandID: input.Dispatch.CommandID, CommandSHA256: input.Dispatch.CommandSHA256,
		TargetID: input.Dispatch.TargetID, LeaseEpoch: input.Dispatch.LeaseEpoch,
		ActivationAllowed: activationAllowed, RunnerAuthorityAccepted: runnerAuthorityAccepted,
		DispatchAdmissionReady: input.Dispatch.AdmissionReady, TransportAdmissionReady: input.Transport.AdmissionReady,
		EffectState: input.Controls.EffectState, EffectStateStartable: effectStateStartable,
		CancellationClear: cancellationClear, ExecutionBoundaryReady: gate.Allowed && len(reasons) == 0,
		RejectionReasons: reasons, PreviewOnly: true, Authority: RunnerExecutionBoundaryAuthority{},
	}, nil
}

// ObserveRunnerExecutionBoundaryPreview joins one API projection to the
// caller-supplied durable lease observation. It performs no I/O; the
// app-server route is responsible for obtaining the lease and Coordinator
// clock value before calling this function.
func ObserveRunnerExecutionBoundaryPreview(
	input RunnerExecutionBoundaryPreviewRequest,
	lease RunnerDispatchAdmissionLease,
	activation devicefabricgate.Request,
	authority devicefabricgate.RunnerAuthorityConfig,
	evaluatedAtMS uint64,
) (RunnerExecutionBoundaryObservation, error) {
	if evaluatedAtMS == 0 || evaluatedAtMS > uint64(MaxSafeIntegerMS) {
		return RunnerExecutionBoundaryObservation{}, errInvalidRequest
	}
	transportAdmission, err := ObserveRunnerTransportAdmission(RunnerTransportAdmissionRequest{
		Owner: input.Owner, ConversationID: input.ConversationID, RunID: input.RunID,
		AttemptID: input.AttemptID, AttemptState: input.AttemptState, Command: input.Command,
		Lease: lease, Transport: input.Transport,
		ExpectedPayloadSHA256: input.ExpectedPayloadSHA256, EvaluatedAtMS: evaluatedAtMS,
	})
	if err != nil {
		return RunnerExecutionBoundaryObservation{}, err
	}
	dispatchAdmission, err := ObserveRunnerDispatchAdmission(RunnerDispatchAdmissionRequest{
		Owner: input.Owner, ConversationID: input.ConversationID, RunID: input.RunID,
		AttemptID: input.AttemptID, AttemptState: input.AttemptState, Command: input.Command,
		EvaluatedAtMS: evaluatedAtMS,
	}, lease)
	if err != nil {
		return RunnerExecutionBoundaryObservation{}, err
	}
	return ObserveRunnerExecutionBoundary(RunnerExecutionBoundaryRequest{
		Activation: activation, Authority: authority, Dispatch: dispatchAdmission,
		Transport: transportAdmission, Controls: input.Controls,
	})
}

// Validate checks an observation before a display or forwarding consumer uses
// it. It never turns readiness into effect authority.
func (observation RunnerExecutionBoundaryObservation) Validate() error {
	if observation.SchemaVersion != RunnerExecutionBoundarySchemaVersion ||
		observation.EvaluationMode != RunnerExecutionBoundaryEvaluationMode ||
		!validRunnerExecutionBoundaryMode(observation.Mode) || !validOwner(observation.Owner) ||
		!validSessionIdentifier(observation.ConversationID) || !validSessionIdentifier(observation.RunID) ||
		!validSessionIdentifier(observation.AttemptID) || !validDispatchAttemptState(observation.AttemptState) ||
		!validSessionIdentifier(observation.CommandID) || !validRunnerDigest(observation.CommandSHA256) ||
		!validSessionIdentifier(observation.TargetID) || observation.LeaseEpoch == 0 ||
		!validRunnerExecutionEffectState(observation.EffectState) ||
		observation.EffectStateStartable != runnerExecutionEffectStateStartable(observation.EffectState) ||
		!isSortedUniqueStrings(observation.RejectionReasons) || !observation.PreviewOnly ||
		observation.Authority != (RunnerExecutionBoundaryAuthority{}) {
		return errInvalidRequest
	}
	ready := observation.Mode == devicefabricgate.ModeExecute && observation.ActivationAllowed && observation.RunnerAuthorityAccepted &&
		observation.DispatchAdmissionReady && observation.TransportAdmissionReady &&
		observation.EffectStateStartable && observation.CancellationClear
	if observation.ExecutionBoundaryReady != ready || (ready && len(observation.RejectionReasons) != 0) ||
		(!ready && len(observation.RejectionReasons) == 0) ||
		(!observation.DispatchAdmissionReady && !containsRunnerExecutionBoundaryReason(observation.RejectionReasons, "dispatch_admission_not_ready")) ||
		(!observation.TransportAdmissionReady && !containsRunnerExecutionBoundaryReason(observation.RejectionReasons, "transport_admission_not_ready")) ||
		(!observation.CancellationClear && !containsRunnerExecutionBoundaryReason(observation.RejectionReasons, "cancellation_requested")) ||
		(!observation.EffectStateStartable && !containsRunnerExecutionBoundaryReason(observation.RejectionReasons, "effect_state_not_startable") &&
			!containsRunnerExecutionBoundaryReason(observation.RejectionReasons, "uncertain_effect_requires_reconciliation")) {
		return errInvalidRequest
	}
	return nil
}

func validateRunnerExecutionBoundaryInputs(input RunnerExecutionBoundaryRequest) error {
	if input.Controls.EffectState == "" || !validRunnerExecutionEffectState(input.Controls.EffectState) {
		return errInvalidRequest
	}
	if err := input.Dispatch.Validate(); err != nil {
		return err
	}
	if err := input.Transport.Validate(); err != nil {
		return err
	}
	if input.Dispatch.Owner != input.Transport.Owner || input.Dispatch.ConversationID != input.Transport.ConversationID ||
		input.Dispatch.RunID != input.Transport.RunID || input.Dispatch.AttemptID != input.Transport.AttemptID ||
		input.Dispatch.CommandID != input.Transport.CommandID || input.Dispatch.CommandSHA256 != input.Transport.CommandSHA256 ||
		input.Dispatch.TargetID != input.Transport.TargetID || input.Dispatch.LeaseEpoch != input.Transport.LeaseEpoch ||
		input.Dispatch.AttemptState != input.Transport.AttemptState {
		return errInvalidRequest
	}
	return nil
}

func runnerAuthorityAccepted(config devicefabricgate.RunnerAuthorityConfig, p4AcceptanceID string) bool {
	return config.Enabled && validRunnerAuthorityID(config.AuthorityID) && config.Decision.Status == "accepted" && config.Decision.AcceptanceID != "" &&
		config.Decision.AcceptedAtUnixMS > 0 && !config.Decision.PlanningOnly &&
		config.AuthorityID == strings.TrimSpace(config.AuthorityID) &&
		(config.Decision.AcceptanceID != p4AcceptanceID || p4AcceptanceID == "")
}

func validRunnerExecutionBoundaryMode(mode devicefabricgate.Mode) bool {
	switch mode {
	case devicefabricgate.ModeOff, devicefabricgate.ModeInventory, devicefabricgate.ModeObserve,
		devicefabricgate.ModeExecute, devicefabricgate.ModeMigrate, devicefabricgate.ModeFederate:
		return true
	default:
		return false
	}
}

func validRunnerAuthorityID(value string) bool {
	if value == "" || len(value) > 256 || value != strings.TrimSpace(value) {
		return false
	}
	for index, character := range value {
		if (character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z') ||
			(character >= '0' && character <= '9') || (index > 0 && (character == '.' || character == '_' || character == ':' || character == '/' || character == '#' || character == '-')) {
			continue
		}
		return false
	}
	return true
}

func validRunnerExecutionEffectState(value string) bool {
	switch value {
	case "not_started", "reconciled", "in_flight", "completed", "failed", "cancelled", "uncertain":
		return true
	default:
		return false
	}
}

func runnerExecutionEffectStateStartable(value string) bool {
	return value == "not_started" || value == "reconciled"
}

func uniqueRunnerExecutionBoundaryReasons(values []string) []string {
	if len(values) < 2 {
		return values
	}
	result := values[:1]
	for _, value := range values[1:] {
		if value != result[len(result)-1] {
			result = append(result, value)
		}
	}
	return result
}

func containsRunnerExecutionBoundaryReason(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}
