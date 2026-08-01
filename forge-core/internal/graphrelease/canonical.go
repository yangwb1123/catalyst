package graphrelease

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"

	"forgeos/forge-core/internal/graphdispatch"
)

type authorizationPayload struct {
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
}

func canonicalBytes(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, errInvalidControl
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidControl
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func domainDigest(domain string, value any) (string, error) {
	encoded, err := canonicalBytes(value)
	if err != nil {
		return "", err
	}
	return rawDomainDigest(domain, encoded), nil
}

func rawDomainDigest(domain string, value []byte) string {
	digest := sha256.New()
	_, _ = digest.Write([]byte(domain))
	_, _ = digest.Write(value)
	return hex.EncodeToString(digest.Sum(nil))
}

func releasePayload(control ReleaseControl) releaseControlPayload {
	return releaseControlPayload{
		V: control.V, SchedulerProtocolVersion: control.SchedulerProtocolVersion,
		ReleaseControlProtocolVersion: control.ReleaseControlProtocolVersion,
		GraphRun:                      control.GraphRun, Plan: control.Plan, Manifest: control.Manifest,
		JournalEvents: control.JournalEvents, ContractRecord: control.ContractRecord,
		Contract: control.Contract, DispatchRequest: control.DispatchRequest,
		ProviderRequestJSON: control.ProviderRequestJSON,
	}
}

func authorizationPayloadFrom(value Authorization) authorizationPayload {
	return authorizationPayload{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		DispatchAuthorizationProtocolVersion: value.DispatchAuthorizationProtocolVersion,
		GraphRunID:                           value.GraphRunID, GraphID: value.GraphID, GroupRunID: value.GroupRunID,
		SourceSnapshotSHA256: value.SourceSnapshotSHA256,
		GraphManifestSHA256:  value.GraphManifestSHA256, CorePlanSHA256: value.CorePlanSHA256,
		ReleaseControlSnapshotSHA256: value.ReleaseControlSnapshotSHA256,
		ExpectedLastEventSeq:         value.ExpectedLastEventSeq,
		ExpectedLastEventSHA256:      value.ExpectedLastEventSHA256,
		ContractID:                   value.ContractID, ContractSHA256: value.ContractSHA256,
		DispatchRequestID:     value.DispatchRequestID,
		DispatchRequestSHA256: value.DispatchRequestSHA256,
		LogicalRequestSHA256:  value.LogicalRequestSHA256,
		RequestBodySHA256:     value.RequestBodySHA256, RequestBodyBytes: value.RequestBodyBytes,
		NodeID: value.NodeID, Attempt: value.Attempt, ProjectID: value.ProjectID,
		ProjectLaneSHA256: value.ProjectLaneSHA256, SameProjectPolicy: value.SameProjectPolicy,
		ProviderKind: value.ProviderKind, Endpoint: value.Endpoint, Model: value.Model,
		DestinationSHA256:     value.DestinationSHA256,
		PricingSnapshotSHA256: value.PricingSnapshotSHA256, Budgets: value.Budgets,
		ReleaseRequirements: value.ReleaseRequirements, Failure: value.Failure,
		ExecutionContractPresent:           value.ExecutionContractPresent,
		DispatchRequestPresent:             value.DispatchRequestPresent,
		DispatchAuthorityReleaseAuthorized: value.DispatchAuthorityReleaseAuthorized,
		DispatchAuthorityReleased:          value.DispatchAuthorityReleased,
	}
}

// MarshalAuthorization returns exact canonical JSON without a trailing LF.
func MarshalAuthorization(value Authorization) ([]byte, error) {
	if validateAuthorization(value) != nil {
		return nil, errInvalidControl
	}
	encoded, err := canonicalBytes(value)
	if err != nil || len(encoded) > maxAuthorizationBytes {
		return nil, errInvalidControl
	}
	return encoded, nil
}
