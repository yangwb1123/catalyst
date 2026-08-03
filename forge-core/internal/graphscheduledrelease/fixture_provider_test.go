package graphscheduledrelease

import (
	"encoding/json"
	"testing"

	"forgeos/forge-core/internal/graphscheduledcontract"
)

func providerBodyTest(
	t *testing.T,
	contract graphscheduledcontract.ScheduledNodeContractCandidate,
) []byte {
	t.Helper()
	return mustCanonicalTest(t, providerRequest{
		Include: []string{"reasoning.encrypted_content"},
		Input: []providerRequestInput{{
			Content: contract.Request.UserPrompt, Role: "user", Type: "message",
		}},
		Instructions: contract.Request.SystemPrompt, MaxOutputTokens: contract.Budgets.MaxOutputTokens,
		Model: contract.Provider.Model, Store: false, Stream: true, Tools: []string{},
	})
}

func providerRecordTest(
	t *testing.T,
	contract graphscheduledcontract.ScheduledNodeContractCandidate,
	body []byte,
) ScheduledNodeProviderRequestRecord {
	t.Helper()
	destination := destinationPayload{
		V: 1, ProviderKind: contract.Provider.Kind,
		Endpoint: contract.Provider.Endpoint, Model: contract.Provider.Model,
	}
	value := ScheduledNodeProviderRequestRecord{
		V: 1, GraphRunID: contract.GraphRunID, ScheduleID: contract.ScheduleID,
		ScheduledContractID: contract.ContractID, ExecutionOrdinal: uint64(contract.Node.ExecutionOrdinal),
		NodeID: contract.Node.NodeID, Attempt: contract.Node.Attempt,
		ScheduledContractSHA256: contract.ContractSHA256,
		LogicalRequestID:        contract.Request.RequestID, LogicalRequestSHA256: contract.Request.RequestSHA256,
		ScheduleSHA256: contract.ScheduleSHA256, ProjectLaneSHA256: contract.Node.ProjectLaneSHA256,
		Provider: contract.Provider.Kind, Endpoint: contract.Provider.Endpoint, Model: contract.Provider.Model,
		DestinationSHA256:     mustDomainDigestTest(t, destinationDigestDomain, destination),
		PricingSnapshotSHA256: contract.Budgets.PricingSnapshotSHA256,
		ProviderRequestSHA256: rawDomainDigest(providerRequestDomain, body),
		ProviderRequestBytes:  uint64(len(body)), CodecProtocolVersion: 1,
		ExpectedLastEventSeq:    contract.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: contract.ExpectedLastEventSHA256,
		ProviderRequestPrepared: true, CreatedAtMS: 76,
	}
	value.PreparedRequestSHA256 = mustDomainDigestTest(t, preparedRequestDomain, preparedPayload(value))
	value.ProviderRequestID = "scheduled-node-provider-request-" + value.PreparedRequestSHA256
	return value
}

func cloneControlTest(t *testing.T, value ReleaseControl) ReleaseControl {
	t.Helper()
	encoded := mustCanonicalTest(t, value)
	var clone ReleaseControl
	if err := jsonUnmarshalTest(encoded, &clone); err != nil {
		t.Fatalf("clone control: %v", err)
	}
	return clone
}

func jsonUnmarshalTest(data []byte, destination any) error {
	return json.Unmarshal(data, destination)
}
