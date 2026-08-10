// Package gopackagedependencyobservationproducer explicitly captures one
// bounded local Git worktree source interval and emits an in-memory lexical Go
// package dependency observation. It grants no build, architecture, impact,
// truth, authority, persistence, or effect attestation.
package gopackagedependencyobservationproducer

import (
	"forgeos/forge-core/internal/gitworktreesource"
	"forgeos/forge-core/internal/gopackagegraph"
)

const (
	ProductionAPIVersion    = "forgeos.governance.local-go-package-dependency-graph-observation-production/v1"
	ParametersAPIVersion    = "forgeos.go-package-dependency-capture.parameters/v1"
	Canonicalization        = "forgeos.canonical-json/v1"
	ProducerID              = "forgeos.local-go-package-dependency-graph-observer"
	ProducerVersion         = "v1"
	FileSelectionProfile    = "selected-module-all-regular-go-files-union-v1"
	ImportResolutionProfile = "selected-module-lexical-import-resolution-v1"
	ModuleProfile           = "selected-go-mod-module-directive-v1"
	ParserProfile           = "go-parser-imports-only-no-partial-facts-v1"
	Result                  = "OBSERVED_LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH (all-regular-Go-file lexical import-header/source observation only; no selected build, dependency availability, compile success, architecture judgment, impact closure, completeness, truth, authority, claim, atom, persistence, or effect attestation)"

	parametersDigestDomain = "forgeos.governance.local-go-package-dependency-graph-parameters.v1"
	graphDigestDomain      = "forgeos.governance.local-go-package-dependency-graph-observation.v1"
	productionDigestDomain = "forgeos.governance.local-go-package-dependency-graph-observation-production.v1"
	maxProductionBytes     = 16 << 20
)

type ParametersManifest struct {
	APIVersion                string `json:"api_version"`
	Canonicalization          string `json:"canonicalization"`
	FileSelectionProfileID    string `json:"file_selection_profile_id"`
	ImportResolutionProfileID string `json:"import_resolution_profile_id"`
	ModuleDirectory           string `json:"module_directory"`
	ModuleProfileID           string `json:"module_profile_id"`
	ParserProfileID           string `json:"parser_profile_id"`
	SourceProfileID           string `json:"source_profile_id"`
}

type ProductionPackage struct {
	APIVersion         string                           `json:"api_version"`
	Canonicalization   string                           `json:"canonicalization"`
	GraphObservation   gopackagegraph.Observation       `json:"graph_observation"`
	ParametersManifest ParametersManifest               `json:"parameters_manifest"`
	SourceManifest     gitworktreesource.SourceManifest `json:"source_manifest"`
}

type Production struct {
	canonicalGraphJSON      []byte
	canonicalParametersJSON []byte
	canonicalProductionJSON []byte
	graphSHA256             string
	packageValue            ProductionPackage
	parametersSHA256        string
	productionSHA256        string
}

func (production *Production) ProductionJSON() []byte {
	if production == nil {
		return nil
	}
	return append([]byte(nil), production.canonicalProductionJSON...)
}

func (production *Production) GraphObservationJSON() []byte {
	if production == nil {
		return nil
	}
	return append([]byte(nil), production.canonicalGraphJSON...)
}

func (production *Production) ParametersManifestJSON() []byte {
	if production == nil {
		return nil
	}
	return append([]byte(nil), production.canonicalParametersJSON...)
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

func (production *Production) GraphSHA256() string {
	if production == nil {
		return ""
	}
	return production.graphSHA256
}

func (production *Production) ParametersSHA256() string {
	if production == nil {
		return ""
	}
	return production.parametersSHA256
}

func (production *Production) Result() string {
	if production == nil {
		return ""
	}
	return Result
}
