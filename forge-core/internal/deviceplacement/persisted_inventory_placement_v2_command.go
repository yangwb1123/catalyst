package deviceplacement

import (
	"bytes"
	"encoding/json"
	"io"
	"unicode/utf8"
)

// persistedInventoryPlacementV2Fixture is the bounded, caller-supplied
// comparison contract. Expected decisions make the command a fixture preview,
// not a target-selection API.
type persistedInventoryPlacementV2PreviewFixture struct {
	SchemaVersion          string                                     `json:"schema_version"`
	EvaluationMode         string                                     `json:"evaluation_mode"`
	SourceSchemaVersion    string                                     `json:"source_schema_version"`
	EvaluationOwner        Owner                                      `json:"evaluation_owner"`
	EvaluatedAtMS          int64                                      `json:"evaluated_at_ms"`
	Notice                 string                                     `json:"notice"`
	Requirements           Requirements                               `json:"requirements"`
	Observation            SessionDeviceObservationInventoryV2        `json:"observation"`
	Expected               []persistedInventoryPlacementV2Expectation `json:"expected"`
	EligibleCandidateCount int                                        `json:"eligible_candidate_count"`
	SelectedDeviceID       *string                                    `json:"selected_device_id"`
	SelectedInstanceID     *string                                    `json:"selected_instance_id"`
	Authority              PersistedInventoryPlacementBatchAuthority  `json:"authority"`
}

type persistedInventoryPlacementV2Expectation struct {
	Revision                uint64   `json:"revision"`
	Generation              uint64   `json:"generation"`
	HeartbeatSequence       uint64   `json:"heartbeat_sequence"`
	DeviceID                string   `json:"device_id"`
	InstanceID              string   `json:"instance_id"`
	ReservationState        string   `json:"reservation_state"`
	GPUCount                int      `json:"gpu_count"`
	AvailableGPUMemoryBytes uint64   `json:"available_gpu_memory_bytes"`
	MatchesRequirements     bool     `json:"matches_requirements"`
	ExclusionReasons        []string `json:"exclusion_reasons"`
}

func persistedObservationV2Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	if len(args) == 1 && (args[0] == "--help" || args[0] == "-h") {
		_, _ = io.WriteString(stderr, commandUsage)
		return 0
	}
	input, err := parseInputFlag(args)
	if err != nil {
		return commandFailure(stderr, 2, "expected --input FILE|-")
	}
	reader, closeInput, err := openInput(input, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "cannot read bounded input")
	}
	fixture, decodeErr := decodePersistedInventoryPlacementV2Fixture(reader)
	closeErr := closeInput()
	if decodeErr != nil || closeErr != nil {
		return commandFailure(stderr, 1, "invalid persisted-observation-v2 input")
	}
	result, err := evaluatePersistedInventoryPlacementV2Fixture(fixture)
	if err != nil {
		return commandFailure(stderr, 1, "invalid persisted-observation-v2 fixture")
	}
	encoded, err := marshalPersistedInventoryPlacementV2(result)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode persisted-observation-v2 result")
	}
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write persisted-observation-v2 result")
	}
	return 0
}

func decodePersistedInventoryPlacementV2Fixture(reader io.Reader) (persistedInventoryPlacementV2PreviewFixture, error) {
	data, err := io.ReadAll(io.LimitReader(reader, MaxRequestBytes+1))
	if err != nil || len(data) == 0 || len(data) > MaxRequestBytes || !utf8.Valid(data) || rejectDuplicateFields(data) != nil || !exactPersistedInventoryPlacementV2FixtureShape(data) {
		return persistedInventoryPlacementV2PreviewFixture{}, errInvalidRequest
	}
	var fixture persistedInventoryPlacementV2PreviewFixture
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if decoder.Decode(&fixture) != nil {
		return persistedInventoryPlacementV2PreviewFixture{}, errInvalidRequest
	}
	var trailing any
	if decoder.Decode(&trailing) != io.EOF {
		return persistedInventoryPlacementV2PreviewFixture{}, errInvalidRequest
	}
	return fixture, nil
}

func evaluatePersistedInventoryPlacementV2Fixture(fixture persistedInventoryPlacementV2PreviewFixture) (PersistedInventoryPlacementV2Evaluation, error) {
	if fixture.SchemaVersion != PersistedInventoryPlacementV2SchemaVersion || fixture.EvaluationMode != PersistedInventoryPlacementV2EvaluationMode ||
		fixture.SourceSchemaVersion != SessionDeviceObservationInventoryV2SchemaVersion || fixture.Notice != PersistedInventoryPlacementV2Notice ||
		fixture.SelectedDeviceID != nil || fixture.SelectedInstanceID != nil || fixture.Authority != (PersistedInventoryPlacementBatchAuthority{}) ||
		len(fixture.Expected) != len(fixture.Observation.Devices) {
		return PersistedInventoryPlacementV2Evaluation{}, errInvalidRequest
	}
	result, err := EvaluatePersistedInventoryObservationV2(fixture.Observation, fixture.EvaluationOwner, fixture.Requirements, fixture.EvaluatedAtMS)
	if err != nil || result.EligibleCandidateCount != fixture.EligibleCandidateCount || len(result.Decisions) != len(fixture.Expected) {
		return PersistedInventoryPlacementV2Evaluation{}, errInvalidRequest
	}
	for index, expected := range fixture.Expected {
		actual := result.Decisions[index]
		if actual.Revision != expected.Revision || actual.Generation != expected.Generation || actual.HeartbeatSequence != expected.HeartbeatSequence ||
			actual.DeviceID != expected.DeviceID || actual.InstanceID != expected.InstanceID || actual.ReservationState != expected.ReservationState ||
			actual.GPUCount != expected.GPUCount || actual.AvailableGPUMemoryBytes != expected.AvailableGPUMemoryBytes ||
			actual.MatchesRequirements != expected.MatchesRequirements || !sameStrings(actual.ExclusionReasons, expected.ExclusionReasons) {
			return PersistedInventoryPlacementV2Evaluation{}, errInvalidRequest
		}
	}
	return result, nil
}

func marshalPersistedInventoryPlacementV2(result PersistedInventoryPlacementV2Evaluation) ([]byte, error) {
	var output bytes.Buffer
	encoder := jsonEncoder(&output)
	if encoder.Encode(result) != nil || output.Len() == 0 || output.Len() > MaxRequestBytes {
		return nil, errInvalidRequest
	}
	encoded := output.Bytes()
	if encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidRequest
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func sameStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
