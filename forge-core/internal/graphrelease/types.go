// Package graphrelease fully validates one passive Node Dispatch release
// control snapshot and emits a content-addressed authorization decision. It
// never reads credentials, claims a project lane, or releases any effect.
package graphrelease

import (
	"encoding/json"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
)

const (
	ReleaseControlVersion   uint16 = 1
	ReleaseControlProtocol  uint16 = 1
	AuthorizationVersion    uint16 = 1
	AuthorizationProtocol   uint16 = 1
	ConsentContractVersion  uint16 = 1
	MaxReleaseControlBytes         = 48 * 1024 * 1024
	maxAuthorizationBytes          = 1024 * 1024
	maxProviderRequestBytes        = 16 * 1024 * 1024
	maxGraphEventBytes             = 64 * 1024
	maxGraphJournalBytes           = 3 * maxGraphEventBytes
)

const (
	releaseControlDigestDomain = "forge.group-agent-node-dispatch-release-control.v1\x00"
	authorizationDigestDomain  = "forge.group-agent-node-dispatch-authorization.v1\x00"
	preparedEventDigestDomain  = "forge.group-agent-graph-run-event.v1\x00"
	controlEventDigestDomain   = "forge.group-agent-graph-run-control-event.v1\x00"
	providerRequestDomain      = "forge.group-agent-node-provider-request.v1\x00"
	destinationDigestDomain    = "forge.group-agent-node-destination.v1\x00"
	dispatchRequestDomain      = "forge.group-agent-node-dispatch-request.v1\x00"
	projectLaneDigestDomain    = "forge.group-agent-project-lane.v1\x00"
)

// ReleaseControl is Rust's exact private v1 dispatch release-control export.
type ReleaseControl struct {
	V                             uint16                              `json:"v"`
	SchedulerProtocolVersion      uint16                              `json:"scheduler_protocol_version"`
	ReleaseControlProtocolVersion uint16                              `json:"release_control_protocol_version"`
	GraphRun                      GraphRunRecord                      `json:"graph_run"`
	Plan                          graphplan.Plan                      `json:"plan"`
	Manifest                      graphdispatch.GraphManifest         `json:"manifest"`
	JournalEvents                 []json.RawMessage                   `json:"journal_events"`
	ContractRecord                NodeExecutionContractRecord         `json:"contract_record"`
	Contract                      graphdispatch.NodeExecutionContract `json:"contract"`
	DispatchRequest               NodeDispatchRequestRecord           `json:"dispatch_request"`
	ProviderRequestJSON           string                              `json:"provider_request_json"`
	SnapshotSHA256                string                              `json:"snapshot_sha256"`
}

type releaseControlPayload struct {
	V                             uint16                              `json:"v"`
	SchedulerProtocolVersion      uint16                              `json:"scheduler_protocol_version"`
	ReleaseControlProtocolVersion uint16                              `json:"release_control_protocol_version"`
	GraphRun                      GraphRunRecord                      `json:"graph_run"`
	Plan                          graphplan.Plan                      `json:"plan"`
	Manifest                      graphdispatch.GraphManifest         `json:"manifest"`
	JournalEvents                 []json.RawMessage                   `json:"journal_events"`
	ContractRecord                NodeExecutionContractRecord         `json:"contract_record"`
	Contract                      graphdispatch.NodeExecutionContract `json:"contract"`
	DispatchRequest               NodeDispatchRequestRecord           `json:"dispatch_request"`
	ProviderRequestJSON           string                              `json:"provider_request_json"`
}

// GraphRunRecord is the exact passive Rust v3 aggregate metadata.
type GraphRunRecord struct {
	V                         uint16 `json:"v"`
	GraphRunID                string `json:"graph_run_id"`
	GraphID                   string `json:"graph_id"`
	Status                    string `json:"status"`
	SourceSnapshotSHA256      string `json:"source_snapshot_sha256"`
	GraphManifestSHA256       string `json:"graph_manifest_sha256"`
	SchedulerProtocolVersion  uint16 `json:"scheduler_protocol_version"`
	PlanSHA256                string `json:"plan_sha256"`
	PlanBytes                 uint64 `json:"plan_bytes"`
	NodeCount                 uint64 `json:"node_count"`
	WaveCount                 uint64 `json:"wave_count"`
	ExecutionContractPresent  bool   `json:"execution_contract_present"`
	DispatchRequestPresent    bool   `json:"dispatch_request_present"`
	DispatchAuthorityReleased bool   `json:"dispatch_authority_released"`
	LastEventSeq              uint64 `json:"last_event_seq"`
	JournalBytes              uint64 `json:"journal_bytes"`
	CreatedAtMS               uint64 `json:"created_at_ms"`
}

// NodeExecutionContractRecord is Rust's exact admitted-contract record.
type NodeExecutionContractRecord struct {
	V                       uint16 `json:"v"`
	ContractID              string `json:"contract_id"`
	GraphRunID              string `json:"graph_run_id"`
	NodeID                  string `json:"node_id"`
	Attempt                 uint16 `json:"attempt"`
	ControlSnapshotSHA256   string `json:"control_snapshot_sha256"`
	ContractSHA256          string `json:"contract_sha256"`
	ContractBytes           uint64 `json:"contract_bytes"`
	RequestSHA256           string `json:"request_sha256"`
	ProjectLaneSHA256       string `json:"project_lane_sha256"`
	ExpectedLastEventSeq    uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256 string `json:"expected_last_event_sha256"`
	CreatedAtMS             uint64 `json:"created_at_ms"`
}

// NodeDispatchRequestRecord is Rust's exact passive prepared-request record.
type NodeDispatchRequestRecord struct {
	V                       uint16 `json:"v"`
	DispatchRequestID       string `json:"dispatch_request_id"`
	GraphRunID              string `json:"graph_run_id"`
	ContractID              string `json:"contract_id"`
	NodeID                  string `json:"node_id"`
	Attempt                 uint16 `json:"attempt"`
	ContractSHA256          string `json:"contract_sha256"`
	RequestSHA256           string `json:"request_sha256"`
	ProjectLaneSHA256       string `json:"project_lane_sha256"`
	Provider                string `json:"provider"`
	Endpoint                string `json:"endpoint"`
	Model                   string `json:"model"`
	PricingSnapshotSHA256   string `json:"pricing_snapshot_sha256"`
	ProviderRequestSHA256   string `json:"provider_request_sha256"`
	ProviderRequestBytes    uint64 `json:"provider_request_bytes"`
	DestinationSHA256       string `json:"destination_sha256"`
	DispatchRequestSHA256   string `json:"dispatch_request_sha256"`
	CodecProtocolVersion    uint16 `json:"codec_protocol_version"`
	ExpectedLastEventSeq    uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256 string `json:"expected_last_event_sha256"`
	CreatedAtMS             uint64 `json:"created_at_ms"`
}

type preparedEvent struct {
	V                        uint16 `json:"v"`
	GraphRunID               string `json:"graph_run_id"`
	Seq                      uint64 `json:"seq"`
	Type                     string `json:"type"`
	GraphID                  string `json:"graph_id"`
	GraphManifestSHA256      string `json:"graph_manifest_sha256"`
	PlanSHA256               string `json:"plan_sha256"`
	SchedulerProtocolVersion uint16 `json:"scheduler_protocol_version"`
	PreparedAtMS             uint64 `json:"prepared_at_ms"`
}

type contractEvent struct {
	V                     uint16 `json:"v"`
	GraphRunID            string `json:"graph_run_id"`
	Seq                   uint64 `json:"seq"`
	Type                  string `json:"type"`
	PreviousEventSHA256   string `json:"previous_event_sha256"`
	ControlSnapshotSHA256 string `json:"control_snapshot_sha256"`
	ContractID            string `json:"contract_id"`
	ContractSHA256        string `json:"contract_sha256"`
	ContractBytes         uint64 `json:"contract_bytes"`
	NodeID                string `json:"node_id"`
	Attempt               uint16 `json:"attempt"`
	RequestSHA256         string `json:"request_sha256"`
	ProjectLaneSHA256     string `json:"project_lane_sha256"`
	AdmittedAtMS          uint64 `json:"admitted_at_ms"`
}

type dispatchEvent struct {
	V                     uint16 `json:"v"`
	GraphRunID            string `json:"graph_run_id"`
	Seq                   uint64 `json:"seq"`
	Type                  string `json:"type"`
	PreviousEventSHA256   string `json:"previous_event_sha256"`
	ContractID            string `json:"contract_id"`
	ContractSHA256        string `json:"contract_sha256"`
	DispatchRequestID     string `json:"dispatch_request_id"`
	DispatchRequestSHA256 string `json:"dispatch_request_sha256"`
	RequestBodySHA256     string `json:"request_body_sha256"`
	RequestBodyBytes      uint64 `json:"request_body_bytes"`
	LogicalRequestSHA256  string `json:"logical_request_sha256"`
	NodeID                string `json:"node_id"`
	Attempt               uint16 `json:"attempt"`
	ProjectLaneSHA256     string `json:"project_lane_sha256"`
	CodecProtocolVersion  uint16 `json:"codec_protocol_version"`
	ProviderKind          string `json:"provider_kind"`
	DestinationSHA256     string `json:"destination_sha256"`
	PricingSnapshotSHA256 string `json:"pricing_snapshot_sha256"`
	PreparedAtMS          uint64 `json:"prepared_at_ms"`
}

type providerRequest struct {
	Include         []string               `json:"include"`
	Input           []providerRequestInput `json:"input"`
	Instructions    string                 `json:"instructions"`
	MaxOutputTokens uint64                 `json:"max_output_tokens"`
	Model           string                 `json:"model"`
	Store           bool                   `json:"store"`
	Stream          bool                   `json:"stream"`
	Tools           []string               `json:"tools"`
}

type providerRequestInput struct {
	Content string `json:"content"`
	Role    string `json:"role"`
	Type    string `json:"type"`
}

type destinationPayload struct {
	V            uint16 `json:"v"`
	ProviderKind string `json:"provider_kind"`
	Endpoint     string `json:"endpoint"`
	Model        string `json:"model"`
}

type dispatchRequestPayload struct {
	V                       uint16 `json:"v"`
	CodecProtocolVersion    uint16 `json:"codec_protocol_version"`
	GraphRunID              string `json:"graph_run_id"`
	ContractID              string `json:"contract_id"`
	ContractSHA256          string `json:"contract_sha256"`
	ExpectedLastEventSeq    uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256 string `json:"expected_last_event_sha256"`
	NodeID                  string `json:"node_id"`
	Attempt                 uint16 `json:"attempt"`
	ProjectLaneSHA256       string `json:"project_lane_sha256"`
	ProviderKind            string `json:"provider_kind"`
	Endpoint                string `json:"endpoint"`
	Model                   string `json:"model"`
	DestinationSHA256       string `json:"destination_sha256"`
	LogicalRequestSHA256    string `json:"logical_request_sha256"`
	PricingSnapshotSHA256   string `json:"pricing_snapshot_sha256"`
	RequestBodyBytes        uint64 `json:"request_body_bytes"`
	RequestBodySHA256       string `json:"request_body_sha256"`
}

// ReleaseRequirements freezes the checks a future effectful claimant must run.
type ReleaseRequirements struct {
	Consent                string `json:"consent"`
	ConsentContractVersion uint16 `json:"consent_contract_version"`
	CredentialPreflight    string `json:"credential_preflight"`
	DestinationPreflight   string `json:"destination_preflight"`
	PricingPreflight       string `json:"pricing_preflight"`
	ProjectLaneClaim       string `json:"project_lane_claim"`
	ProviderHealthCheck    string `json:"provider_health_check"`
}

// Authorization is a passive decision artifact, never consent, a signature,
// a lane claim, or evidence that dispatch authority has been released.
type Authorization struct {
	V                                    uint16                         `json:"v"`
	SchedulerProtocolVersion             uint16                         `json:"scheduler_protocol_version"`
	DispatchAuthorizationProtocolVersion uint16                         `json:"dispatch_authorization_protocol_version"`
	GraphRunID                           string                         `json:"graph_run_id"`
	GraphID                              string                         `json:"graph_id"`
	GroupRunID                           string                         `json:"group_run_id"`
	SourceSnapshotSHA256                 string                         `json:"source_snapshot_sha256"`
	GraphManifestSHA256                  string                         `json:"graph_manifest_sha256"`
	CorePlanSHA256                       string                         `json:"core_plan_sha256"`
	ReleaseControlSnapshotSHA256         string                         `json:"release_control_snapshot_sha256"`
	ExpectedLastEventSeq                 uint64                         `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256              string                         `json:"expected_last_event_sha256"`
	ContractID                           string                         `json:"contract_id"`
	ContractSHA256                       string                         `json:"contract_sha256"`
	DispatchRequestID                    string                         `json:"dispatch_request_id"`
	DispatchRequestSHA256                string                         `json:"dispatch_request_sha256"`
	LogicalRequestSHA256                 string                         `json:"logical_request_sha256"`
	RequestBodySHA256                    string                         `json:"request_body_sha256"`
	RequestBodyBytes                     uint64                         `json:"request_body_bytes"`
	NodeID                               string                         `json:"node_id"`
	Attempt                              uint16                         `json:"attempt"`
	ProjectID                            string                         `json:"project_id"`
	ProjectLaneSHA256                    string                         `json:"project_lane_sha256"`
	SameProjectPolicy                    string                         `json:"same_project_policy"`
	ProviderKind                         string                         `json:"provider_kind"`
	Endpoint                             string                         `json:"endpoint"`
	Model                                string                         `json:"model"`
	DestinationSHA256                    string                         `json:"destination_sha256"`
	PricingSnapshotSHA256                string                         `json:"pricing_snapshot_sha256"`
	Budgets                              graphdispatch.ExecutionBudgets `json:"budgets"`
	ReleaseRequirements                  ReleaseRequirements            `json:"release_requirements"`
	Failure                              graphdispatch.FailurePolicy    `json:"failure"`
	ExecutionContractPresent             bool                           `json:"execution_contract_present"`
	DispatchRequestPresent               bool                           `json:"dispatch_request_present"`
	DispatchAuthorityReleaseAuthorized   bool                           `json:"dispatch_authority_release_authorized"`
	DispatchAuthorityReleased            bool                           `json:"dispatch_authority_released"`
	AuthorizationID                      string                         `json:"authorization_id"`
	AuthorizationSHA256                  string                         `json:"authorization_sha256"`
}
