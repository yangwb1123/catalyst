// Package evolvelocatorobservationproducer captures a validated local Evolve
// report and its current repository locator files into exact ADR-0050
// observations. Capture is explicit, in-memory, and grants no scan judgment,
// completion, truth, authority, persistence, or external-effect attestation.
package evolvelocatorobservationproducer

import (
	locatorcontract "forgeos/forge-core/internal/evolverepolocatorevidencecontract"
	"forgeos/forge-core/internal/gitworktreesource"
)

const (
	ProductionAPIVersion = "forgeos.governance.local-evolve-repo-locator-observation-production/v1"
	ParametersAPIVersion = "forgeos.evolve-capture.parameters/v1"
	ReportAPIVersion     = "forgeos.evolve-capture.report/v1"
	Canonicalization     = "forgeos.canonical-json/v1"
	ProducerID           = "forgeos.local-evolve-repo-locator-observer"
	ProducerVersion      = "v1"
	ReportProfileID      = "evolve-scan-canonical-marker-v1"
	CapturedLocatorSet   = "CAPTURED_LOCAL_EVOLVE_LOCATOR_SET (local report/source capture only; locator set may be empty; no scan judgment, completion, truth, authority, claim, atom, persistence, or effect attestation)"

	parametersDigestDomain = "forgeos.governance.local-evolve-repo-locator-parameters.v1"
	productionDigestDomain = "forgeos.governance.local-evolve-repo-locator-observation-production.v1"
	maxProductionBytes     = 16 << 20
	maxEvidenceFileBytes   = int64(1 << 20)
	maxObservations        = 240
)

type ParametersManifest struct {
	APIVersion       string `json:"api_version"`
	Canonicalization string `json:"canonicalization"`
	Contract         string `json:"contract"`
	ExpectedDepth    string `json:"expected_depth"`
	ReportProfileID  string `json:"report_profile_id"`
	SourceProfileID  string `json:"source_profile_id"`
}

type ReportManifest struct {
	APIVersion       string `json:"api_version"`
	Bytes            int64  `json:"bytes"`
	CanonicalReport  string `json:"canonical_report"`
	Canonicalization string `json:"canonicalization"`
	ProfileID        string `json:"profile_id"`
	SHA256           string `json:"sha256"`
}

type ProductionPackage struct {
	APIVersion         string                           `json:"api_version"`
	Canonicalization   string                           `json:"canonicalization"`
	Observations       []locatorcontract.Observation    `json:"observations"`
	ParametersManifest ParametersManifest               `json:"parameters_manifest"`
	ReportManifest     ReportManifest                   `json:"report_manifest"`
	SourceManifest     gitworktreesource.SourceManifest `json:"source_manifest"`
}

type Production struct {
	canonicalObservationJSON [][]byte
	canonicalProductionJSON  []byte
	packageValue             ProductionPackage
	productionSHA256         string
}

func (production *Production) ProductionJSON() []byte {
	if production == nil {
		return nil
	}
	return append([]byte(nil), production.canonicalProductionJSON...)
}

func (production *Production) ObservationJSON(index int) []byte {
	if production == nil || index < 0 || index >= len(production.canonicalObservationJSON) {
		return nil
	}
	return append([]byte(nil), production.canonicalObservationJSON[index]...)
}

func (production *Production) Package() ProductionPackage {
	if production == nil {
		return ProductionPackage{}
	}
	return cloneProductionPackage(production.packageValue)
}

func (production *Production) SHA256() string {
	if production == nil {
		return ""
	}
	return production.productionSHA256
}

func (production *Production) Result() string {
	if production == nil {
		return ""
	}
	return CapturedLocatorSet
}
