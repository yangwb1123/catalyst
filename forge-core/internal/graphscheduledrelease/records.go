package graphscheduledrelease

// GraphRunRecord is the exact pristine Rust Graph Run metadata.
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

// ExecutionScheduleRecord is Rust's immutable schedule sidecar metadata.
type ExecutionScheduleRecord struct {
	V                         uint16 `json:"v"`
	ScheduleID                string `json:"schedule_id"`
	GraphRunID                string `json:"graph_run_id"`
	GraphID                   string `json:"graph_id"`
	ControlSnapshotSHA256     string `json:"control_snapshot_sha256"`
	ScheduleSHA256            string `json:"schedule_sha256"`
	ScheduleBytes             uint64 `json:"schedule_bytes"`
	NodeCount                 uint64 `json:"node_count"`
	WaveCount                 uint64 `json:"wave_count"`
	ExpectedLastEventSeq      uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256   string `json:"expected_last_event_sha256"`
	ExecutionContractPresent  bool   `json:"execution_contract_present"`
	DispatchAuthorityReleased bool   `json:"dispatch_authority_released"`
	CreatedAtMS               uint64 `json:"created_at_ms"`
}

// ScheduledNodeContractRecord is Rust's passive ordinal-zero candidate row.
type ScheduledNodeContractRecord struct {
	V                          uint16 `json:"v"`
	ContractID                 string `json:"contract_id"`
	GraphRunID                 string `json:"graph_run_id"`
	ScheduleID                 string `json:"schedule_id"`
	NodeID                     string `json:"node_id"`
	ExecutionOrdinal           uint64 `json:"execution_ordinal"`
	Attempt                    uint16 `json:"attempt"`
	ControlSnapshotSHA256      string `json:"control_snapshot_sha256"`
	ScheduleSHA256             string `json:"schedule_sha256"`
	ContractSHA256             string `json:"contract_sha256"`
	ContractBytes              uint64 `json:"contract_bytes"`
	RequestID                  string `json:"request_id"`
	RequestSHA256              string `json:"request_sha256"`
	ProjectLaneSHA256          string `json:"project_lane_sha256"`
	ExpectedLastEventSeq       uint64 `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256    string `json:"expected_last_event_sha256"`
	PredecessorReceiptCount    uint64 `json:"predecessor_receipt_count"`
	LifecycleContractAdmitted  bool   `json:"lifecycle_contract_admitted"`
	ProviderRequestPresent     bool   `json:"provider_request_present"`
	ExecutionAuthorityReleased bool   `json:"execution_authority_released"`
	DispatchAuthorityReleased  bool   `json:"dispatch_authority_released"`
	ProgressObserved           bool   `json:"progress_observed"`
	SuccessorAdvanceAuthorized bool   `json:"successor_advance_authorized"`
	CreatedAtMS                uint64 `json:"created_at_ms"`
}
