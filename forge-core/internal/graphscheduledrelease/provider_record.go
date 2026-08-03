package graphscheduledrelease

// ScheduledNodeProviderRequestRecord is Rust's exact passive request sidecar.
type ScheduledNodeProviderRequestRecord struct {
	V                          uint16 `json:"v"`
	ProviderRequestID          string `json:"provider_request_id"`
	GraphRunID                 string `json:"graph_run_id"`
	ScheduleID                 string `json:"schedule_id"`
	ScheduledContractID        string `json:"scheduled_contract_id"`
	ExecutionOrdinal           uint64 `json:"execution_ordinal"`
	NodeID                     string `json:"node_id"`
	Attempt                    uint16 `json:"attempt"`
	ScheduledContractSHA256    string `json:"scheduled_contract_sha256"`
	LogicalRequestID           string `json:"logical_request_id"`
	LogicalRequestSHA256       string `json:"logical_request_sha256"`
	ScheduleSHA256             string `json:"schedule_sha256"`
	ProjectLaneSHA256          string `json:"project_lane_sha256"`
	Provider                   string `json:"provider"`
	Endpoint                   string `json:"endpoint"`
	Model                      string `json:"model"`
	DestinationSHA256          string `json:"destination_sha256"`
	PricingSnapshotSHA256      string `json:"pricing_snapshot_sha256"`
	ProviderRequestSHA256      string `json:"provider_request_sha256"`
	ProviderRequestBytes       uint64 `json:"provider_request_bytes"`
	PreparedRequestSHA256      string `json:"prepared_request_sha256"`
	CodecProtocolVersion       uint16 `json:"codec_protocol_version"`
	ExpectedLastEventSeq       uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256    string `json:"expected_last_event_sha256"`
	ProviderRequestPrepared    bool   `json:"provider_request_prepared"`
	ProviderRequestSent        bool   `json:"provider_request_sent"`
	LifecycleContractAdmitted  bool   `json:"lifecycle_contract_admitted"`
	ExecutionAuthorityReleased bool   `json:"execution_authority_released"`
	DispatchAuthorityReleased  bool   `json:"dispatch_authority_released"`
	ProjectLaneClaimed         bool   `json:"project_lane_claimed"`
	ProgressObserved           bool   `json:"progress_observed"`
	SuccessorAdvanceAuthorized bool   `json:"successor_advance_authorized"`
	CreatedAtMS                uint64 `json:"created_at_ms"`
}

type preparedRequestPayload struct {
	V                          uint16 `json:"v"`
	CodecProtocolVersion       uint16 `json:"codec_protocol_version"`
	GraphRunID                 string `json:"graph_run_id"`
	ScheduleID                 string `json:"schedule_id"`
	ScheduleSHA256             string `json:"schedule_sha256"`
	ScheduledContractID        string `json:"scheduled_contract_id"`
	ScheduledContractSHA256    string `json:"scheduled_contract_sha256"`
	ExpectedLastEventSeq       uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256    string `json:"expected_last_event_sha256"`
	ExecutionOrdinal           uint64 `json:"execution_ordinal"`
	NodeID                     string `json:"node_id"`
	Attempt                    uint16 `json:"attempt"`
	ProjectLaneSHA256          string `json:"project_lane_sha256"`
	ProviderKind               string `json:"provider_kind"`
	Endpoint                   string `json:"endpoint"`
	Model                      string `json:"model"`
	DestinationSHA256          string `json:"destination_sha256"`
	LogicalRequestID           string `json:"logical_request_id"`
	LogicalRequestSHA256       string `json:"logical_request_sha256"`
	PricingSnapshotSHA256      string `json:"pricing_snapshot_sha256"`
	RequestBodyBytes           uint64 `json:"request_body_bytes"`
	RequestBodySHA256          string `json:"request_body_sha256"`
	ProviderRequestPrepared    bool   `json:"provider_request_prepared"`
	ProviderRequestSent        bool   `json:"provider_request_sent"`
	LifecycleContractAdmitted  bool   `json:"lifecycle_contract_admitted"`
	ExecutionAuthorityReleased bool   `json:"execution_authority_released"`
	DispatchAuthorityReleased  bool   `json:"dispatch_authority_released"`
	ProjectLaneClaimed         bool   `json:"project_lane_claimed"`
	ProgressObserved           bool   `json:"progress_observed"`
	SuccessorAdvanceAuthorized bool   `json:"successor_advance_authorized"`
}
