package commandobservationevidencecontract

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
		return nil, fmt.Errorf("command observation evidence request JSON: %w", err)
	}
	root, ok := node.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("command observation evidence request root must be an object")
	}
	if err := validateRequestShape(root); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(root)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("command observation evidence request is not exact compact canonical JSON")
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
		return nil, fmt.Errorf("command observation evidence request: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return nil, fmt.Errorf("command observation evidence request has trailing JSON value")
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

// CanonicalObservationJSON returns the exact compact canonical bytes for one
// standalone command observation. Unlike Adapt, this API intentionally accepts
// honest timed_out and cancelled terminations: local capture producers need to
// seal those observations even though EvidenceRecord v1 can project only a
// real process exit. The returned slice is newly allocated and carries no
// execution, verdict, persistence, or authority semantics.
func CanonicalObservationJSON(observation Observation) ([]byte, error) {
	encoded, err := canonicalObservationJSON(observation)
	if err != nil {
		return nil, err
	}
	return append([]byte(nil), encoded...), nil
}

func canonicalCommandJSON(command Command) ([]byte, error) {
	if err := validateCommand(command); err != nil {
		return nil, err
	}
	return canonicalBounded(commandNode(command), "command")
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
		"api_version": request.APIVersion, "binding": bindingNode(request.Binding),
		"canonicalization": request.Canonicalization,
		"observation":      observationNode(request.Observation),
	}
}

func bindingNode(binding Binding) map[string]any {
	return map[string]any{
		"aggregate_id": binding.AggregateID, "context_sha256": binding.ContextSHA256,
		"policy_sha256": binding.PolicySHA256, "project_id": binding.ProjectID,
		"scope": binding.Scope, "sensitivity": binding.Sensitivity,
		"sequence": binding.Sequence, "subjects": stringsNode(binding.Subjects),
		"supersedes_record_ids": stringsNode(binding.SupersedesRecordIDs),
	}
}

func observationNode(observation Observation) map[string]any {
	return map[string]any{
		"api_version": observation.APIVersion, "canonicalization": observation.Canonicalization,
		"command":          observationCommandNode(observation.Command),
		"ended_at_unix_ms": observation.EndedAtUnixMS, "evidence_type": observation.EvidenceType,
		"producer": producerNode(observation.Producer), "source": sourceNode(observation.Source),
		"started_at_unix_ms": observation.StartedAtUnixMS,
		"streams":            streamsNode(observation.Streams), "termination": terminationNode(observation.Termination),
	}
}

func observationCommandNode(command Command) map[string]any { return commandNode(command) }

func commandNode(command Command) map[string]any {
	var timeout any
	if command.TimeoutMS != nil {
		timeout = *command.TimeoutMS
	}
	return map[string]any{
		"argv": stringsNode(command.Argv), "cwd": command.CWD,
		"environment_sha256": command.EnvironmentSHA256, "stdin_bytes": command.StdinBytes,
		"stdin_sha256": command.StdinSHA256, "timeout_ms": timeout,
		"tool_snapshot_sha256": command.ToolSnapshotSHA256,
	}
}

func producerNode(producer Producer) map[string]any {
	return map[string]any{
		"producer_id": producer.ProducerID, "producer_type": producer.ProducerType,
		"producer_version": producer.ProducerVersion, "run_id": producer.RunID,
	}
}

func sourceNode(source Source) map[string]any {
	return map[string]any{
		"source_revision": source.SourceRevision, "source_tree_sha256": source.SourceTreeSHA256,
	}
}

func streamsNode(streams Streams) map[string]any {
	return map[string]any{
		"combined": streamNode(streams.Combined), "stderr": streamNode(streams.Stderr),
		"stdout": streamNode(streams.Stdout),
	}
}

func streamNode(stream Stream) map[string]any {
	return map[string]any{
		"bytes": stream.Bytes, "retained_bytes": stream.RetainedBytes,
		"retained_sha256": stream.RetainedSHA256, "sha256": stream.SHA256,
	}
}

func terminationNode(termination Termination) map[string]any {
	var exitCode any
	if termination.ExitCode != nil {
		exitCode = *termination.ExitCode
	}
	return map[string]any{"exit_code": exitCode, "kind": termination.Kind}
}

func stringsNode(values []string) []any {
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}
