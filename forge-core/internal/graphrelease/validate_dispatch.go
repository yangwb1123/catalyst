package graphrelease

import "reflect"

func validateDispatchBindings(control ReleaseControl, facts journalFacts) error {
	body := []byte(control.ProviderRequestJSON)
	bodySHA256, err := validateProviderRequest(control, body)
	if err != nil || validateDispatchRecord(control, facts, body, bodySHA256) != nil ||
		validateDispatchEvent(control, facts, body, bodySHA256) != nil {
		return errInvalidControl
	}
	return nil
}

func validateProviderRequest(control ReleaseControl, body []byte) (string, error) {
	if len(body) == 0 || len(body) > maxProviderRequestBytes {
		return "", errInvalidControl
	}
	request, err := decodeExact[providerRequest](body)
	if err != nil {
		return "", errInvalidControl
	}
	contract := control.Contract
	expected := providerRequest{
		Include: []string{"reasoning.encrypted_content"},
		Input: []providerRequestInput{{
			Content: contract.Request.UserPrompt, Role: "user", Type: "message",
		}},
		Instructions:    contract.Request.SystemPrompt,
		MaxOutputTokens: contract.Budgets.MaxOutputTokens,
		Model:           contract.Provider.Model, Store: false, Stream: true, Tools: []string{},
	}
	if !reflect.DeepEqual(request, expected) {
		return "", errInvalidControl
	}
	return rawDomainDigest(providerRequestDomain, body), nil
}

func validateDispatchRecord(
	control ReleaseControl,
	facts journalFacts,
	body []byte,
	bodySHA256 string,
) error {
	record := control.DispatchRequest
	contract := control.Contract
	valid := validDispatchRecordHeader(record, facts, body, bodySHA256) &&
		record.GraphRunID == contract.GraphRunID && record.ContractID == contract.ContractID &&
		record.NodeID == contract.Node.NodeID && record.Attempt == contract.Node.Attempt &&
		record.ContractSHA256 == contract.ContractSHA256 &&
		record.RequestSHA256 == contract.Request.RequestSHA256 &&
		record.ProjectLaneSHA256 == contract.Node.ProjectLaneSHA256 &&
		record.Provider == contract.Provider.Kind && record.Endpoint == contract.Provider.Endpoint &&
		record.Model == contract.Provider.Model &&
		record.PricingSnapshotSHA256 == contract.Budgets.PricingSnapshotSHA256
	if !valid || validateDispatchIdentity(record) != nil {
		return errInvalidControl
	}
	return nil
}

func validDispatchRecordHeader(
	record NodeDispatchRequestRecord,
	facts journalFacts,
	body []byte,
	bodySHA256 string,
) bool {
	return record.V == 1 && record.Attempt == 1 &&
		record.Provider == "openai_responses" && record.CodecProtocolVersion == 1 &&
		record.ProviderRequestSHA256 == bodySHA256 &&
		record.ProviderRequestBytes == uint64(len(body)) &&
		record.ExpectedLastEventSeq == 2 &&
		record.ExpectedLastEventSHA256 == facts.ContractSHA256 &&
		validSignedTime(record.CreatedAtMS)
}

func validateDispatchIdentity(record NodeDispatchRequestRecord) error {
	destination := destinationPayload{
		V: 1, ProviderKind: record.Provider, Endpoint: record.Endpoint, Model: record.Model,
	}
	destinationSHA256, err := domainDigest(destinationDigestDomain, destination)
	if err != nil || destinationSHA256 != record.DestinationSHA256 {
		return errInvalidControl
	}
	payload := dispatchPayloadFrom(record)
	digest, err := domainDigest(dispatchRequestDomain, payload)
	if err != nil || digest != record.DispatchRequestSHA256 ||
		record.DispatchRequestID != "node-dispatch-request-"+digest {
		return errInvalidControl
	}
	return nil
}

func dispatchPayloadFrom(record NodeDispatchRequestRecord) dispatchRequestPayload {
	return dispatchRequestPayload{
		V: record.V, CodecProtocolVersion: record.CodecProtocolVersion,
		GraphRunID: record.GraphRunID, ContractID: record.ContractID,
		ContractSHA256:          record.ContractSHA256,
		ExpectedLastEventSeq:    record.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: record.ExpectedLastEventSHA256,
		NodeID:                  record.NodeID, Attempt: record.Attempt,
		ProjectLaneSHA256: record.ProjectLaneSHA256, ProviderKind: record.Provider,
		Endpoint: record.Endpoint, Model: record.Model,
		DestinationSHA256:     record.DestinationSHA256,
		LogicalRequestSHA256:  record.RequestSHA256,
		PricingSnapshotSHA256: record.PricingSnapshotSHA256,
		RequestBodyBytes:      record.ProviderRequestBytes,
		RequestBodySHA256:     record.ProviderRequestSHA256,
	}
}

func validateDispatchEvent(
	control ReleaseControl,
	facts journalFacts,
	body []byte,
	bodySHA256 string,
) error {
	event := facts.Dispatch
	record := control.DispatchRequest
	valid := event.ContractID == record.ContractID && event.ContractSHA256 == record.ContractSHA256 &&
		event.DispatchRequestID == record.DispatchRequestID &&
		event.DispatchRequestSHA256 == record.DispatchRequestSHA256 &&
		event.RequestBodySHA256 == bodySHA256 && event.RequestBodyBytes == uint64(len(body)) &&
		event.LogicalRequestSHA256 == record.RequestSHA256 && event.NodeID == record.NodeID &&
		event.Attempt == record.Attempt && event.ProjectLaneSHA256 == record.ProjectLaneSHA256 &&
		event.CodecProtocolVersion == record.CodecProtocolVersion &&
		event.ProviderKind == record.Provider && event.DestinationSHA256 == record.DestinationSHA256 &&
		event.PricingSnapshotSHA256 == record.PricingSnapshotSHA256 &&
		event.PreparedAtMS == record.CreatedAtMS
	if !valid {
		return errInvalidControl
	}
	return nil
}
