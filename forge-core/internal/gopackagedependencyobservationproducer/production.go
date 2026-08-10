package gopackagedependencyobservationproducer

import (
	"fmt"

	"forgeos/forge-core/internal/gitworktreesource"
	"forgeos/forge-core/internal/gopackagegraph"
)

func buildParameters(moduleDirectory string) (ParametersManifest, []byte, string, error) {
	if err := gopackagegraph.ValidateModuleDirectory(moduleDirectory); err != nil {
		return ParametersManifest{}, nil, "", err
	}
	value := ParametersManifest{
		APIVersion: ParametersAPIVersion, Canonicalization: Canonicalization,
		FileSelectionProfileID:    FileSelectionProfile,
		ImportResolutionProfileID: ImportResolutionProfile,
		ModuleDirectory:           moduleDirectory, ModuleProfileID: ModuleProfile,
		ParserProfileID: ParserProfile, SourceProfileID: gitworktreesource.ProfileID,
	}
	encoded, err := canonicalJSON(value, maxProductionBytes)
	if err != nil {
		return ParametersManifest{}, nil, "", fmt.Errorf("canonical capture parameters: %w", err)
	}
	return value, encoded, domainDigest(parametersDigestDomain, encoded), nil
}

func sealProduction(value ProductionPackage) (*Production, error) {
	if err := validateProductionPackage(value); err != nil {
		return nil, fmt.Errorf("validate local Go package dependency production: %w", err)
	}
	parametersJSON, err := canonicalJSON(value.ParametersManifest, maxProductionBytes)
	if err != nil {
		return nil, fmt.Errorf("canonical capture parameters: %w", err)
	}
	graphJSON, err := canonicalJSON(value.GraphObservation, maxProductionBytes)
	if err != nil {
		return nil, fmt.Errorf("canonical graph observation: %w", err)
	}
	encoded, err := canonicalJSON(value, maxProductionBytes)
	if err != nil {
		return nil, fmt.Errorf("canonical production: %w", err)
	}
	parametersSHA256 := domainDigest(parametersDigestDomain, parametersJSON)
	return &Production{
		canonicalGraphJSON:      append([]byte(nil), graphJSON...),
		canonicalParametersJSON: append([]byte(nil), parametersJSON...),
		canonicalProductionJSON: append([]byte(nil), encoded...),
		graphSHA256:             domainDigest(graphDigestDomain, graphJSON),
		packageValue:            cloneProductionPackage(value), parametersSHA256: parametersSHA256,
		productionSHA256: domainDigest(productionDigestDomain, encoded),
	}, nil
}

func cloneProductionPackage(value ProductionPackage) ProductionPackage {
	value.GraphObservation = gopackagegraph.CloneObservation(value.GraphObservation)
	value.SourceManifest = gitworktreesource.CloneManifest(value.SourceManifest)
	return value
}
