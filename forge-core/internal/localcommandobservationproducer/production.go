package localcommandobservationproducer

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"reflect"

	commandcontract "forgeos/forge-core/internal/commandobservationevidencecontract"
)

// prepareProfiles freezes the pre-execution command, child environment,
// top-level executable and repository source manifests.
func prepareProfiles(ctx context.Context, root, class string, timeoutMS *int64) (*preparedProfiles, error) {
	command, err := commandForClass(class)
	if err != nil {
		return nil, err
	}
	environment, environmentDigest, childEnvironment, err := environmentSnapshot(os.Environ())
	if err != nil {
		return nil, err
	}
	tool, toolDigest, err := toolSnapshot(ctx, command, environment)
	if err != nil {
		return nil, err
	}
	canonicalRoot, err := filepath.Abs(root)
	if err != nil {
		return nil, fmt.Errorf("resolve repository root: %w", err)
	}
	canonicalRoot, err = filepath.EvalSymlinks(canonicalRoot)
	if err != nil {
		return nil, fmt.Errorf("resolve repository root symlinks: %w", err)
	}
	source, sourceDigest, err := sourceSnapshot(ctx, canonicalRoot, childEnvironment)
	if err != nil {
		return nil, err
	}
	if timeoutMS != nil && (*timeoutMS < 1 || *timeoutMS > 86_400_000) {
		return nil, fmt.Errorf("timeout_ms must be null or integer 1..86400000")
	}
	return &preparedProfiles{
		ChildEnvironment: childEnvironment, Command: command, Environment: environment,
		EnvironmentSHA256: environmentDigest, Root: canonicalRoot, Source: source,
		SourceTreeSHA256: sourceDigest, TimeoutMS: cloneInt64(timeoutMS), Tool: tool,
		ToolSHA256: toolDigest,
	}, nil
}

// buildProduction verifies post-execution tool/source stability, builds one
// existing CommandObservation v1, and seals the additive production package.
func buildProduction(ctx context.Context, prepared *preparedProfiles, runID string, capture capture) (*Production, error) {
	if prepared == nil {
		return nil, fmt.Errorf("prepared profiles are nil")
	}
	if err := validatePreparedProfiles(*prepared); err != nil {
		return nil, err
	}
	if err := verifyStableProfiles(ctx, *prepared); err != nil {
		return nil, err
	}
	observation := buildObservation(*prepared, runID, capture)
	observationJSON, err := commandcontract.CanonicalObservationJSON(observation)
	if err != nil {
		return nil, fmt.Errorf("canonical command observation: %w", err)
	}
	productionPackage := buildProductionPackage(*prepared, observation)
	if err := validateProductionPackage(productionPackage); err != nil {
		return nil, fmt.Errorf("validate command observation production: %w", err)
	}
	productionJSON, err := canonicalManifest(productionPackage)
	if err != nil {
		return nil, err
	}
	return &Production{
		canonicalObservationJSON: append([]byte(nil), observationJSON...),
		canonicalProductionJSON:  append([]byte(nil), productionJSON...),
		packageValue:             productionPackage,
		productionSHA256:         domainDigest(productionDigestDomain, productionJSON),
		result:                   ObservedLocalProcess,
	}, nil
}

func verifyStableProfiles(ctx context.Context, prepared preparedProfiles) error {
	tool, toolDigest, err := toolSnapshot(ctx, prepared.Command, prepared.Environment)
	if err != nil {
		return fmt.Errorf("post-execution tool snapshot: %w", err)
	}
	if toolDigest != prepared.ToolSHA256 || !reflect.DeepEqual(tool, prepared.Tool) {
		return fmt.Errorf("top-level executable changed during command observation")
	}
	source, sourceDigest, err := sourceSnapshot(ctx, prepared.Root, prepared.ChildEnvironment)
	if err != nil {
		return fmt.Errorf("post-execution source snapshot: %w", err)
	}
	if sourceDigest != prepared.SourceTreeSHA256 || !reflect.DeepEqual(source, prepared.Source) {
		return fmt.Errorf("repository source changed during command observation")
	}
	return nil
}

func buildObservation(prepared preparedProfiles, runID string, capture capture) commandcontract.Observation {
	return commandcontract.Observation{
		APIVersion: commandcontract.ObservationAPIVersion, Canonicalization: Canonicalization,
		Command: commandcontract.Command{
			Argv: append([]string(nil), prepared.Command.Argv...), CWD: ".",
			EnvironmentSHA256: prepared.EnvironmentSHA256, StdinBytes: 0,
			StdinSHA256: emptySHA256, TimeoutMS: cloneInt64(prepared.TimeoutMS),
			ToolSnapshotSHA256: prepared.ToolSHA256,
		},
		EndedAtUnixMS: capture.EndedAtUnixMS, EvidenceType: prepared.Command.EvidenceType,
		Producer: commandcontract.Producer{
			ProducerID: ProducerID, ProducerType: "tool", ProducerVersion: ProducerVersion, RunID: runID,
		},
		Source: commandcontract.Source{
			SourceRevision: prepared.Source.SourceRevision, SourceTreeSHA256: prepared.SourceTreeSHA256,
		},
		StartedAtUnixMS: capture.StartedAtUnixMS, Streams: capture.Streams,
		Termination: capture.Termination,
	}
}

func buildProductionPackage(prepared preparedProfiles, observation commandcontract.Observation) ProductionPackage {
	return ProductionPackage{
		APIVersion: ProductionAPIVersion, Canonicalization: Canonicalization,
		EnvironmentManifest: cloneEnvironmentManifest(prepared.Environment),
		Observation:         observation, SourceManifest: cloneSourceManifest(prepared.Source),
		ToolManifest: cloneToolManifest(prepared.Tool),
	}
}

func cloneEnvironmentManifest(value EnvironmentManifest) EnvironmentManifest {
	value.Variables = append([]EnvironmentVariable(nil), value.Variables...)
	return value
}

func cloneToolManifest(value ToolManifest) ToolManifest {
	value.SymlinkHops = append(make([]SymlinkHop, 0, len(value.SymlinkHops)), value.SymlinkHops...)
	return value
}

func cloneSourceManifest(value SourceManifest) SourceManifest {
	entries := make([]SourceEntry, len(value.Entries))
	for index, entry := range value.Entries {
		entries[index] = cloneSourceEntry(entry)
	}
	value.Entries = entries
	return value
}

func cloneSourceEntry(value SourceEntry) SourceEntry {
	value.ContentSHA256 = cloneString(value.ContentSHA256)
	value.Executable = cloneBool(value.Executable)
	value.IndexMode = cloneString(value.IndexMode)
	value.SymlinkTarget = cloneString(value.SymlinkTarget)
	return value
}

func cloneString(value *string) *string {
	if value == nil {
		return nil
	}
	return stringPointer(*value)
}

func cloneBool(value *bool) *bool {
	if value == nil {
		return nil
	}
	return boolPointer(*value)
}

func cloneInt64(value *int64) *int64 {
	if value == nil {
		return nil
	}
	result := *value
	return &result
}
