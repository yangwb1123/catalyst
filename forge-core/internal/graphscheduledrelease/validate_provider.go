package graphscheduledrelease

import "reflect"

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

func validateProviderSource(value ReleaseControl) error {
	body := []byte(value.ProviderRequestJSON)
	bodySHA256, err := validateProviderBody(value, body)
	if err != nil || !validProviderRecord(value, body, bodySHA256) ||
		validateProviderIdentities(value.ProviderRequest) != nil {
		return errInvalidControl
	}
	return nil
}

func validateProviderBody(value ReleaseControl, body []byte) (string, error) {
	if len(body) == 0 || len(body) > maxProviderRequestBytes {
		return "", errInvalidControl
	}
	request, err := decodeExact[providerRequest](body)
	if err != nil {
		return "", errInvalidControl
	}
	contract := value.ScheduledContract
	expected := providerRequest{
		Include: []string{"reasoning.encrypted_content"},
		Input: []providerRequestInput{{
			Content: contract.Request.UserPrompt, Role: "user", Type: "message",
		}},
		Instructions: contract.Request.SystemPrompt, MaxOutputTokens: contract.Budgets.MaxOutputTokens,
		Model: contract.Provider.Model, Store: false, Stream: true, Tools: []string{},
	}
	if !reflect.DeepEqual(request, expected) {
		return "", errInvalidControl
	}
	return rawDomainDigest(providerRequestDomain, body), nil
}

func validProviderRecord(value ReleaseControl, body []byte, bodySHA256 string) bool {
	record, contract := value.ProviderRequest, value.ScheduledContract
	return validProviderRecordHeader(record, body, bodySHA256) &&
		record.GraphRunID == contract.GraphRunID && record.ScheduleID == contract.ScheduleID &&
		record.ScheduledContractID == contract.ContractID &&
		record.ExecutionOrdinal == uint64(contract.Node.ExecutionOrdinal) &&
		record.NodeID == contract.Node.NodeID && record.Attempt == contract.Node.Attempt &&
		record.ScheduledContractSHA256 == contract.ContractSHA256 &&
		record.LogicalRequestID == contract.Request.RequestID &&
		record.LogicalRequestSHA256 == contract.Request.RequestSHA256 &&
		record.ScheduleSHA256 == contract.ScheduleSHA256 &&
		record.ProjectLaneSHA256 == contract.Node.ProjectLaneSHA256 &&
		record.Provider == contract.Provider.Kind && record.Endpoint == contract.Provider.Endpoint &&
		record.Model == contract.Provider.Model &&
		record.PricingSnapshotSHA256 == contract.Budgets.PricingSnapshotSHA256 &&
		record.ExpectedLastEventSeq == contract.ExpectedLastEventSeq &&
		record.ExpectedLastEventSHA256 == contract.ExpectedLastEventSHA256
}

func validProviderRecordHeader(
	record ScheduledNodeProviderRequestRecord,
	body []byte,
	bodySHA256 string,
) bool {
	return record.V == 1 && record.ExecutionOrdinal == 0 && record.Attempt == 1 &&
		record.Provider == "openai_responses" && record.CodecProtocolVersion == 1 &&
		record.ProviderRequestSHA256 == bodySHA256 &&
		record.ProviderRequestBytes == uint64(len(body)) && record.ExpectedLastEventSeq == 1 &&
		record.ProviderRequestPrepared && !record.ProviderRequestSent &&
		!record.LifecycleContractAdmitted && !record.ExecutionAuthorityReleased &&
		!record.DispatchAuthorityReleased && !record.ProjectLaneClaimed &&
		!record.ProgressObserved && !record.SuccessorAdvanceAuthorized &&
		validSignedTime(record.CreatedAtMS)
}

func validateProviderIdentities(record ScheduledNodeProviderRequestRecord) error {
	destination := destinationPayload{
		V: 1, ProviderKind: record.Provider, Endpoint: record.Endpoint, Model: record.Model,
	}
	destinationSHA256, err := domainDigest(destinationDigestDomain, destination)
	if err != nil || destinationSHA256 != record.DestinationSHA256 {
		return errInvalidControl
	}
	digest, err := domainDigest(preparedRequestDomain, preparedPayload(record))
	if err != nil || digest != record.PreparedRequestSHA256 ||
		record.ProviderRequestID != "scheduled-node-provider-request-"+digest {
		return errInvalidControl
	}
	return nil
}
