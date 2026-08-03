// Package graphscheduledrelease validates one exact, pristine scheduled-node
// dispatch control and emits a passive, content-addressed release decision.
// It performs no consent, credential, lane, provider, or persistence effect.
package graphscheduledrelease

import (
	"encoding/json"
	"errors"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledcontract"
)

const (
	ReleaseControlVersion   uint16 = 1
	ReleaseControlProtocol  uint16 = 1
	AuthorizationVersion    uint16 = 1
	AuthorizationProtocol   uint16 = 1
	ConsentContractVersion  uint16 = 1
	MaxReleaseControlBytes         = 64 * 1024 * 1024
	maxAuthorizationBytes          = 1024 * 1024
	maxProviderRequestBytes        = 16 * 1024 * 1024
	maxGraphEventBytes             = 64 * 1024
)

const (
	releaseControlDigestDomain = "forge.group-agent-scheduled-node-dispatch-release-control.v1\x00"
	authorizationDigestDomain  = "forge.group-agent-scheduled-node-dispatch-authorization.v1\x00"
	preparedEventDigestDomain  = "forge.group-agent-graph-run-event.v1\x00"
	providerRequestDomain      = "forge.group-agent-node-provider-request.v1\x00"
	destinationDigestDomain    = "forge.group-agent-node-destination.v1\x00"
	preparedRequestDomain      = "forge.group-agent-scheduled-node-provider-request.v1\x00"
	authorizationIDPrefix      = "scheduled-node-dispatch-authorization-"
)

var errInvalidControl = errors.New("invalid scheduled-node dispatch release control")

// ReleaseControl is Rust's private, exact scheduled-node release export.
type ReleaseControl struct {
	V                             uint16                                                `json:"v"`
	SchedulerProtocolVersion      uint16                                                `json:"scheduler_protocol_version"`
	ReleaseControlProtocolVersion uint16                                                `json:"release_control_protocol_version"`
	GraphRun                      GraphRunRecord                                        `json:"graph_run"`
	JournalEvents                 []json.RawMessage                                     `json:"journal_events"`
	ControlSnapshot               graphdispatch.ControlSnapshot                         `json:"control_snapshot"`
	ScheduleRecord                ExecutionScheduleRecord                               `json:"schedule_record"`
	Schedule                      graphschedule.ExecutionSchedule                       `json:"schedule"`
	ScheduledContractRecord       ScheduledNodeContractRecord                           `json:"scheduled_contract_record"`
	ScheduledContract             graphscheduledcontract.ScheduledNodeContractCandidate `json:"scheduled_contract"`
	ProviderRequest               ScheduledNodeProviderRequestRecord                    `json:"provider_request"`
	ProviderRequestJSON           string                                                `json:"provider_request_json"`
	SnapshotSHA256                string                                                `json:"snapshot_sha256"`
}

type releaseControlPayload struct {
	V                             uint16                                                `json:"v"`
	SchedulerProtocolVersion      uint16                                                `json:"scheduler_protocol_version"`
	ReleaseControlProtocolVersion uint16                                                `json:"release_control_protocol_version"`
	GraphRun                      GraphRunRecord                                        `json:"graph_run"`
	JournalEvents                 []json.RawMessage                                     `json:"journal_events"`
	ControlSnapshot               graphdispatch.ControlSnapshot                         `json:"control_snapshot"`
	ScheduleRecord                ExecutionScheduleRecord                               `json:"schedule_record"`
	Schedule                      graphschedule.ExecutionSchedule                       `json:"schedule"`
	ScheduledContractRecord       ScheduledNodeContractRecord                           `json:"scheduled_contract_record"`
	ScheduledContract             graphscheduledcontract.ScheduledNodeContractCandidate `json:"scheduled_contract"`
	ProviderRequest               ScheduledNodeProviderRequestRecord                    `json:"provider_request"`
	ProviderRequestJSON           string                                                `json:"provider_request_json"`
}
