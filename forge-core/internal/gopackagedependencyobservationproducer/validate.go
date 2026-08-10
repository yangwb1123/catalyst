package gopackagedependencyobservationproducer

import (
	"fmt"
	"regexp"

	"forgeos/forge-core/internal/gitworktreesource"
	"forgeos/forge-core/internal/gopackagegraph"
)

var runIDPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]{0,159}$`)

func validateProductionPackage(value ProductionPackage) error {
	if value.APIVersion != ProductionAPIVersion || value.Canonicalization != Canonicalization {
		return fmt.Errorf("production identity is invalid")
	}
	parametersJSON, err := validateParameters(value.ParametersManifest)
	if err != nil {
		return err
	}
	parametersSHA256 := domainDigest(parametersDigestDomain, parametersJSON)
	sourceSHA256, err := gitworktreesource.Digest(value.SourceManifest)
	if err != nil {
		return fmt.Errorf("source manifest: %w", err)
	}
	if err := gitworktreesource.Validate(value.SourceManifest, sourceSHA256); err != nil {
		return fmt.Errorf("source manifest: %w", err)
	}
	return validateGraphBindings(value, parametersSHA256, sourceSHA256)
}

func validateParameters(value ParametersManifest) ([]byte, error) {
	if value.APIVersion != ParametersAPIVersion || value.Canonicalization != Canonicalization ||
		value.FileSelectionProfileID != FileSelectionProfile ||
		value.ImportResolutionProfileID != ImportResolutionProfile ||
		value.ModuleProfileID != ModuleProfile || value.ParserProfileID != ParserProfile ||
		value.SourceProfileID != gitworktreesource.ProfileID {
		return nil, fmt.Errorf("capture parameters manifest is invalid")
	}
	if err := gopackagegraph.ValidateModuleDirectory(value.ModuleDirectory); err != nil {
		return nil, fmt.Errorf("capture parameters module_directory: %w", err)
	}
	encoded, err := canonicalJSON(value, maxProductionBytes)
	if err != nil {
		return nil, fmt.Errorf("canonical capture parameters: %w", err)
	}
	return encoded, nil
}

func validateGraphBindings(
	value ProductionPackage,
	parametersSHA256, sourceSHA256 string,
) error {
	graph := value.GraphObservation
	if graph.APIVersion != gopackagegraph.APIVersion ||
		graph.Canonicalization != gopackagegraph.Canonicalization ||
		graph.ProfileID != gopackagegraph.ProfileID || graph.ObservedAtUnixMS < 0 {
		return fmt.Errorf("graph observation identity or time is invalid")
	}
	if err := gopackagegraph.ValidateObservation(
		graph, value.ParametersManifest.ModuleDirectory,
		graphSourceEntries(value.SourceManifest),
	); err != nil {
		return fmt.Errorf("graph observation: %w", err)
	}
	if graph.Module.Directory != value.ParametersManifest.ModuleDirectory ||
		graph.Producer.ParametersSHA256 != parametersSHA256 ||
		graph.Producer.ProducerID != ProducerID || graph.Producer.ProducerType != "tool" ||
		graph.Producer.ProducerVersion != ProducerVersion ||
		!runIDPattern.MatchString(graph.Producer.RunID) {
		return fmt.Errorf("graph module or producer binding is invalid")
	}
	if graph.Source.SourceRevision != value.SourceManifest.SourceRevision ||
		graph.Source.SourceTreeSHA256 != sourceSHA256 {
		return fmt.Errorf("graph source binding is invalid")
	}
	return nil
}
