package evolvelocatorobservationproducer

import (
	"fmt"

	locatorcontract "forgeos/forge-core/internal/evolverepolocatorevidencecontract"
	"forgeos/forge-core/internal/evolvescan"
	"forgeos/forge-core/internal/gitworktreesource"
)

func buildParameters(depth string) (ParametersManifest, string, error) {
	manifest := ParametersManifest{
		APIVersion: ParametersAPIVersion, Canonicalization: Canonicalization,
		Contract: evolvescan.ContractV1, ExpectedDepth: depth,
		ReportProfileID: ReportProfileID, SourceProfileID: gitworktreesource.ProfileID,
	}
	encoded, err := canonicalJSON(manifest)
	if err != nil {
		return ParametersManifest{}, "", fmt.Errorf("canonical capture parameters: %w", err)
	}
	return manifest, domainDigest(parametersDigestDomain, encoded), nil
}

func sealProduction(value ProductionPackage) (*Production, error) {
	if err := validateProductionPackage(value); err != nil {
		return nil, fmt.Errorf("validate local Evolve locator production: %w", err)
	}
	encoded, err := canonicalJSON(value)
	if err != nil {
		return nil, fmt.Errorf("canonical local Evolve locator production: %w", err)
	}
	if len(encoded) > maxProductionBytes {
		return nil, fmt.Errorf("canonical production exceeds %d bytes", maxProductionBytes)
	}
	observationJSON := make([][]byte, len(value.Observations))
	for index, observation := range value.Observations {
		observationJSON[index], err = locatorcontract.CanonicalObservationJSON(observation)
		if err != nil {
			return nil, fmt.Errorf("canonical observation[%d]: %w", index, err)
		}
	}
	return &Production{
		canonicalObservationJSON: observationJSON,
		canonicalProductionJSON:  append([]byte(nil), encoded...),
		packageValue:             cloneProductionPackage(value),
		productionSHA256:         domainDigest(productionDigestDomain, encoded),
	}, nil
}

func cloneProductionPackage(value ProductionPackage) ProductionPackage {
	value.Observations = cloneObservations(value.Observations)
	value.SourceManifest = gitworktreesource.CloneManifest(value.SourceManifest)
	return value
}

func cloneObservations(values []locatorcontract.Observation) []locatorcontract.Observation {
	result := make([]locatorcontract.Observation, len(values))
	for index, value := range values {
		value.ScanContext.OpportunityID = cloneString(value.ScanContext.OpportunityID)
		result[index] = value
	}
	return result
}

func validDepth(value string) bool {
	switch value {
	case evolvescan.DepthAdvisory, evolvescan.DepthOpportunistic,
		evolvescan.DepthStandard, evolvescan.DepthThorough:
		return true
	default:
		return false
	}
}
