package commandobservationevidencecontract

import (
	"fmt"
	"math"
	"regexp"
	"sort"
	"strings"
	"unicode/utf8"
)

const (
	maxArgvItems = 64
	maxTextRunes = 4096
	maxTimeoutMS = int64(86400000)
	maxExitCode  = int64(2147483647)
	emptySHA256  = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
)

var (
	hashPattern       = regexp.MustCompile(`^[a-f0-9]{64}$`)
	identifierPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)
)

func validateRequest(request *Request) error {
	if request == nil {
		return fmt.Errorf("command observation evidence request is nil")
	}
	if request.APIVersion != APIVersion {
		return fmt.Errorf("api_version must be %q", APIVersion)
	}
	if request.Canonicalization != Canonicalization {
		return fmt.Errorf("canonicalization must be %q", Canonicalization)
	}
	if err := validateBinding(request.Binding); err != nil {
		return fmt.Errorf("binding: %w", err)
	}
	if err := validateObservationSemantics(request.Observation); err != nil {
		return fmt.Errorf("observation: %w", err)
	}
	if request.Observation.Termination.Kind != "exited" {
		return fmt.Errorf("observation.termination: only exited observations are projectable")
	}
	return nil
}

// ValidateObservation validates the standalone observation wire. Timeout and
// cancellation are valid observations even though this adapter cannot project
// them into EvidenceRecord v1.
func ValidateObservation(observation Observation) error {
	if err := validateObservationSemantics(observation); err != nil {
		return err
	}
	_, err := canonicalBounded(observationNode(observation), "observation")
	return err
}

func validateObservationSemantics(observation Observation) error {
	if observation.APIVersion != ObservationAPIVersion {
		return fmt.Errorf("api_version must be %q", ObservationAPIVersion)
	}
	if observation.Canonicalization != Canonicalization {
		return fmt.Errorf("canonicalization must be %q", Canonicalization)
	}
	if err := validateCommand(observation.Command); err != nil {
		return fmt.Errorf("command: %w", err)
	}
	if observation.StartedAtUnixMS < 0 {
		return fmt.Errorf("started_at_unix_ms must be nonnegative")
	}
	if observation.EndedAtUnixMS < observation.StartedAtUnixMS {
		return fmt.Errorf("ended_at_unix_ms cannot precede started_at_unix_ms")
	}
	if !inSet(observation.EvidenceType, "gate_result", "test_run") {
		return fmt.Errorf("unsupported evidence_type %q", observation.EvidenceType)
	}
	if err := validateProducer(observation.Producer); err != nil {
		return fmt.Errorf("producer: %w", err)
	}
	if err := validateSource(observation.Source); err != nil {
		return fmt.Errorf("source: %w", err)
	}
	if err := validateStreams(observation.Streams); err != nil {
		return fmt.Errorf("streams: %w", err)
	}
	if err := validateTermination(observation.Termination); err != nil {
		return fmt.Errorf("termination: %w", err)
	}
	return nil
}

func validateCommand(command Command) error {
	if len(command.Argv) < 1 || len(command.Argv) > maxArgvItems {
		return fmt.Errorf("argv must contain 1..%d exact arguments", maxArgvItems)
	}
	for index, argument := range command.Argv {
		if err := validateBoundedString(argument, true); err != nil {
			return fmt.Errorf("argv[%d]: %w", index, err)
		}
	}
	if command.Argv[0] == "" {
		return fmt.Errorf("argv[0] executable must be nonempty")
	}
	if err := validateCWD(command.CWD); err != nil {
		return err
	}
	for name, value := range map[string]string{
		"environment_sha256":   command.EnvironmentSHA256,
		"stdin_sha256":         command.StdinSHA256,
		"tool_snapshot_sha256": command.ToolSnapshotSHA256,
	} {
		if err := validateHash(name, value); err != nil {
			return err
		}
	}
	if command.StdinBytes < 0 {
		return fmt.Errorf("stdin_bytes must be nonnegative")
	}
	if command.StdinBytes == 0 && command.StdinSHA256 != emptySHA256 {
		return fmt.Errorf("stdin_sha256: empty stdin must use SHA-256(empty)")
	}
	if command.TimeoutMS != nil && (*command.TimeoutMS < 1 || *command.TimeoutMS > maxTimeoutMS) {
		return fmt.Errorf("timeout_ms must be null or integer 1..%d", maxTimeoutMS)
	}
	return nil
}

func validateProducer(producer Producer) error {
	for name, value := range map[string]string{
		"producer_id": producer.ProducerID, "producer_version": producer.ProducerVersion,
		"run_id": producer.RunID,
	} {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	if !inSet(producer.ProducerType, "service", "tool") {
		return fmt.Errorf("unsupported producer_type %q", producer.ProducerType)
	}
	return nil
}

func validateSource(source Source) error {
	if err := validateIdentifier("source_revision", source.SourceRevision); err != nil {
		return err
	}
	return validateHash("source_tree_sha256", source.SourceTreeSHA256)
}

func validateStreams(streams Streams) error {
	for name, stream := range map[string]Stream{
		"combined": streams.Combined, "stderr": streams.Stderr, "stdout": streams.Stdout,
	} {
		if err := validateStream(name, stream); err != nil {
			return err
		}
	}
	if streams.Stdout.Bytes > math.MaxInt64-streams.Stderr.Bytes {
		return fmt.Errorf("stdout plus stderr bytes exceeds signed int64")
	}
	if streams.Combined.Bytes != streams.Stdout.Bytes+streams.Stderr.Bytes {
		return fmt.Errorf("combined.bytes must equal stdout.bytes + stderr.bytes")
	}
	return nil
}

func validateStream(name string, stream Stream) error {
	if stream.Bytes < 0 || stream.RetainedBytes < 0 {
		return fmt.Errorf("%s bytes and retained_bytes must be nonnegative", name)
	}
	if stream.RetainedBytes > stream.Bytes {
		return fmt.Errorf("%s retained_bytes cannot exceed bytes", name)
	}
	if err := validateHash(name+".retained_sha256", stream.RetainedSHA256); err != nil {
		return err
	}
	if err := validateHash(name+".sha256", stream.SHA256); err != nil {
		return err
	}
	if stream.Bytes == 0 && (stream.SHA256 != emptySHA256 || stream.RetainedSHA256 != emptySHA256) {
		return fmt.Errorf("%s empty stream must use SHA-256(empty)", name)
	}
	if stream.RetainedBytes == 0 && stream.RetainedSHA256 != emptySHA256 {
		return fmt.Errorf("%s empty retained prefix must use SHA-256(empty)", name)
	}
	if stream.RetainedBytes == stream.Bytes && stream.RetainedSHA256 != stream.SHA256 {
		return fmt.Errorf("%s fully retained stream digests must match", name)
	}
	return nil
}

func validateTermination(termination Termination) error {
	switch termination.Kind {
	case "exited":
		if termination.ExitCode == nil || *termination.ExitCode < 0 || *termination.ExitCode > maxExitCode {
			return fmt.Errorf("exited requires exit_code integer 0..%d", maxExitCode)
		}
	case "cancelled", "timed_out":
		if termination.ExitCode != nil {
			return fmt.Errorf("%s requires null exit_code", termination.Kind)
		}
	default:
		return fmt.Errorf("unsupported termination kind %q", termination.Kind)
	}
	return nil
}

func validateBinding(binding Binding) error {
	for name, value := range map[string]string{
		"aggregate_id": binding.AggregateID, "project_id": binding.ProjectID, "scope": binding.Scope,
	} {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	if err := validateHash("context_sha256", binding.ContextSHA256); err != nil {
		return err
	}
	if err := validateHash("policy_sha256", binding.PolicySHA256); err != nil {
		return err
	}
	if binding.Sequence < 1 {
		return fmt.Errorf("sequence must be positive")
	}
	if !inSet(binding.Sensitivity, "public", "internal", "confidential", "restricted") {
		return fmt.Errorf("unsupported sensitivity %q", binding.Sensitivity)
	}
	if err := validateIdentifierList("subjects", binding.Subjects, true); err != nil {
		return err
	}
	return validateIdentifierList("supersedes_record_ids", binding.SupersedesRecordIDs, false)
}

func validateIdentifier(name, value string) error {
	if len(value) > 160 || !identifierPattern.MatchString(value) {
		return fmt.Errorf("%s is not a valid identifier", name)
	}
	return nil
}

func validateHash(name, value string) error {
	if !hashPattern.MatchString(value) {
		return fmt.Errorf("%s must be a lowercase bare SHA-256", name)
	}
	return nil
}

func validateIdentifierList(name string, values []string, nonempty bool) error {
	if len(values) > maxItems {
		return fmt.Errorf("%s exceeds %d items", name, maxItems)
	}
	if nonempty && len(values) == 0 {
		return fmt.Errorf("%s must be nonempty", name)
	}
	for _, value := range values {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	if !sort.StringsAreSorted(values) {
		return fmt.Errorf("%s must already be UTF-8-byte sorted", name)
	}
	for index := 1; index < len(values); index++ {
		if values[index] == values[index-1] {
			return fmt.Errorf("%s contains duplicate %q", name, values[index])
		}
	}
	return nil
}

func validateCWD(value string) error {
	if err := validateBoundedString(value, false); err != nil {
		return fmt.Errorf("cwd: %w", err)
	}
	if value == "." {
		return nil
	}
	if value == "" || strings.HasPrefix(value, "/") || strings.Contains(value, `\`) {
		return fmt.Errorf("cwd must be '.' or a safe normalized repo-relative path")
	}
	if len(value) >= 2 && isASCIIAlpha(value[0]) && value[1] == ':' {
		return fmt.Errorf("cwd must be '.' or a safe normalized repo-relative path")
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "" || segment == "." || segment == ".." {
			return fmt.Errorf("cwd must be '.' or a safe normalized repo-relative path")
		}
	}
	return nil
}

func validateBoundedString(value string, allowEmpty bool) error {
	if err := validateString(value); err != nil {
		return err
	}
	length := utf8.RuneCountInString(value)
	if (!allowEmpty && length == 0) || length > maxTextRunes {
		return fmt.Errorf("must contain %s%d Unicode scalars", map[bool]string{true: "0..", false: "1.."}[allowEmpty], maxTextRunes)
	}
	return nil
}

func isASCIIAlpha(value byte) bool {
	return value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
}

func inSet(value string, candidates ...string) bool {
	for _, candidate := range candidates {
		if value == candidate {
			return true
		}
	}
	return false
}
