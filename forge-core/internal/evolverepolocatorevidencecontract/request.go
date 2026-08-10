package evolverepolocatorevidencecontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

// DecodeRequest accepts only exact compact canonical request bytes.
func DecodeRequest(data []byte) (*Request, error) {
	node, err := parseStrictRequestJSON(data)
	if err != nil {
		return nil, fmt.Errorf("evolve repository locator evidence request JSON: %w", err)
	}
	root, ok := node.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("evolve repository locator evidence request root must be an object")
	}
	if err := validateRequestShape(root); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(root)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("evolve repository locator evidence request is not exact compact canonical JSON")
	}
	request, err := decodeTypedRequest(data)
	if err != nil {
		return nil, err
	}
	if err := validateRequest(request); err != nil {
		return nil, err
	}
	typedCanonical, err := canonicalRequestJSON(*request)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, typedCanonical) {
		return nil, fmt.Errorf("typed request does not preserve exact canonical input")
	}
	return request, nil
}

func decodeTypedRequest(data []byte) (*Request, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	decoder.UseNumber()
	var request Request
	if err := decoder.Decode(&request); err != nil {
		return nil, fmt.Errorf("evolve repository locator evidence request: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return nil, fmt.Errorf("evolve repository locator evidence request has trailing JSON value")
	}
	return &request, nil
}

func canonicalRequestJSON(request Request) ([]byte, error) {
	if err := validateRequest(&request); err != nil {
		return nil, err
	}
	return canonicalBounded(requestNode(request), "request")
}

func canonicalObservationJSON(observation Observation) ([]byte, error) {
	if err := validateObservationSemantics(observation); err != nil {
		return nil, err
	}
	return canonicalBounded(observationNode(observation), "observation")
}

func canonicalLocatorJSON(locator Locator) ([]byte, error) {
	if err := validateLocator(locator); err != nil {
		return nil, err
	}
	return canonicalBounded(locatorNode(locator), "locator")
}

func canonicalBounded(node map[string]any, label string) ([]byte, error) {
	encoded, err := canonicalJSON(node)
	if err != nil {
		return nil, err
	}
	if len(encoded) > maxRequestBytes {
		return nil, fmt.Errorf("%s JSON byte length exceeds %d", label, maxRequestBytes)
	}
	return encoded, nil
}

func requestNode(request Request) map[string]any {
	return map[string]any{
		"api_version":      request.APIVersion,
		"binding":          bindingNode(request.Binding),
		"canonicalization": request.Canonicalization,
		"observation":      observationNode(request.Observation),
	}
}

func bindingNode(binding Binding) map[string]any {
	return map[string]any{
		"aggregate_id":          binding.AggregateID,
		"context_sha256":        binding.ContextSHA256,
		"policy_sha256":         binding.PolicySHA256,
		"project_id":            binding.ProjectID,
		"scope":                 binding.Scope,
		"sensitivity":           binding.Sensitivity,
		"sequence":              binding.Sequence,
		"subjects":              stringsNode(binding.Subjects),
		"supersedes_record_ids": stringsNode(binding.SupersedesRecordIDs),
	}
}

func observationNode(observation Observation) map[string]any {
	return map[string]any{
		"api_version":         observation.APIVersion,
		"canonicalization":    observation.Canonicalization,
		"content":             contentNode(observation.Content),
		"locator":             locatorNode(observation.Locator),
		"observed_at_unix_ms": observation.ObservedAtUnixMS,
		"producer":            producerNode(observation.Producer),
		"scan_context":        scanContextNode(observation.ScanContext),
		"source":              sourceNode(observation.Source),
	}
}

func contentNode(content Content) map[string]any {
	return map[string]any{"bytes": content.Bytes, "sha256": content.SHA256}
}

func locatorNode(locator Locator) map[string]any {
	return map[string]any{"detail": locator.Detail, "line": locator.Line, "path": locator.Path}
}

func producerNode(producer Producer) map[string]any {
	return map[string]any{
		"parameters_sha256": producer.ParametersSHA256,
		"producer_id":       producer.ProducerID,
		"producer_type":     producer.ProducerType,
		"producer_version":  producer.ProducerVersion,
		"run_id":            producer.RunID,
	}
}

func scanContextNode(context ScanContext) map[string]any {
	var opportunityID any
	if context.OpportunityID != nil {
		opportunityID = *context.OpportunityID
	}
	return map[string]any{
		"contract":       context.Contract,
		"depth":          context.Depth,
		"dimension":      context.Dimension,
		"opportunity_id": opportunityID,
		"relation":       context.Relation,
		"report_sha256":  context.ReportSHA256,
	}
}

func sourceNode(source Source) map[string]any {
	return map[string]any{
		"source_revision":    source.SourceRevision,
		"source_tree_sha256": source.SourceTreeSHA256,
	}
}

func stringsNode(values []string) []any {
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}
