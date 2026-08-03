package graphscheduledrelease

import "forgeos/forge-core/internal/graphdispatch"

type authorizationPayload struct {
	V                                    uint16                         `json:"v"`
	SchedulerProtocolVersion             uint16                         `json:"scheduler_protocol_version"`
	DispatchAuthorizationProtocolVersion uint16                         `json:"dispatch_authorization_protocol_version"`
	GraphRunID                           string                         `json:"graph_run_id"`
	GraphID                              string                         `json:"graph_id"`
	GroupRunID                           string                         `json:"group_run_id"`
	GroupID                              string                         `json:"group_id"`
	SourceSnapshotSHA256                 string                         `json:"source_snapshot_sha256"`
	GraphManifestSHA256                  string                         `json:"graph_manifest_sha256"`
	CorePlanSHA256                       string                         `json:"core_plan_sha256"`
	ControlSnapshotSHA256                string                         `json:"control_snapshot_sha256"`
	ReleaseControlSnapshotSHA256         string                         `json:"release_control_snapshot_sha256"`
	ScheduleID                           string                         `json:"schedule_id"`
	ScheduleSHA256                       string                         `json:"schedule_sha256"`
	ScheduledContractID                  string                         `json:"scheduled_contract_id"`
	ScheduledContractSHA256              string                         `json:"scheduled_contract_sha256"`
	ScheduledProviderRequestID           string                         `json:"scheduled_provider_request_id"`
	ScheduledProviderRequestSHA256       string                         `json:"scheduled_provider_request_sha256"`
	LogicalRequestID                     string                         `json:"logical_request_id"`
	LogicalRequestSHA256                 string                         `json:"logical_request_sha256"`
	RequestBodySHA256                    string                         `json:"request_body_sha256"`
	RequestBodyBytes                     uint64                         `json:"request_body_bytes"`
	ExpectedLastEventSeq                 uint64                         `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256              string                         `json:"expected_last_event_sha256"`
	ExecutionOrdinal                     uint64                         `json:"execution_ordinal"`
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
	LifecycleContractAdmissionAuthorized bool                           `json:"lifecycle_contract_admission_authorized"`
	ExecutionAuthorityReleaseAuthorized  bool                           `json:"execution_authority_release_authorized"`
	DispatchAuthorityReleaseAuthorized   bool                           `json:"dispatch_authority_release_authorized"`
	ScheduledContractCandidatePresent    bool                           `json:"scheduled_contract_candidate_present"`
	ProviderRequestPrepared              bool                           `json:"provider_request_prepared"`
	LifecycleContractAdmitted            bool                           `json:"lifecycle_contract_admitted"`
	ExecutionAuthorityReleased           bool                           `json:"execution_authority_released"`
	DispatchAuthorityReleased            bool                           `json:"dispatch_authority_released"`
	ProjectLaneClaimed                   bool                           `json:"project_lane_claimed"`
	ProviderRequestSent                  bool                           `json:"provider_request_sent"`
	ProgressObserved                     bool                           `json:"progress_observed"`
	TerminalReceiptRecorded              bool                           `json:"terminal_receipt_recorded"`
	SuccessorAdvanceAuthorized           bool                           `json:"successor_advance_authorized"`
}
