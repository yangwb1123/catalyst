// Package localcommandobservationproducer freezes secret-scrubbed local
// gate/test capture profiles and builds exact CommandObservation v1 production
// packages. It does not grant process execution or attest pass, criterion,
// completion, truth, authority, identity, persistence, or external effects.
package localcommandobservationproducer

import (
	commandcontract "forgeos/forge-core/internal/commandobservationevidencecontract"
	"forgeos/forge-core/internal/gitworktreesource"
)

const (
	ProductionAPIVersion  = "forgeos.governance.local-gate-command-observation-production/v1"
	EnvironmentAPIVersion = "forgeos.command-capture.environment/v1"
	ToolAPIVersion        = "forgeos.command-capture.tool/v1"
	SourceTreeAPIVersion  = gitworktreesource.APIVersion
	Canonicalization      = "forgeos.canonical-json/v1"
	ProducerID            = "forgeos.local-gate-command-observer"
	ProducerVersion       = "v1"
	ObservedLocalProcess  = "OBSERVED_LOCAL_PROCESS (local process capture only; no pass, criterion, completion, truth, authority, identity, persistence, or external-effect attestation)"

	environmentProfileID = "scrubbed-parent-environment-v1"
	toolProfileID        = "resolved-top-level-executable-v1"
	sourceTreeProfileID  = gitworktreesource.ProfileID
	emptySHA256          = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
)

type EnvironmentVariable struct {
	Name  string `json:"name"`
	Value string `json:"value"`
}

type EnvironmentManifest struct {
	APIVersion       string                `json:"api_version"`
	Canonicalization string                `json:"canonicalization"`
	ProfileID        string                `json:"profile_id"`
	Variables        []EnvironmentVariable `json:"variables"`
}

type SymlinkHop struct {
	Path   string `json:"path"`
	Target string `json:"target"`
}

type ToolManifest struct {
	APIVersion       string       `json:"api_version"`
	Bytes            int64        `json:"bytes"`
	Canonicalization string       `json:"canonicalization"`
	FinalPath        string       `json:"final_path"`
	Mode             int64        `json:"mode"`
	ProfileID        string       `json:"profile_id"`
	RequestedPath    string       `json:"requested_path"`
	ResolvedPath     string       `json:"resolved_path"`
	SHA256           string       `json:"sha256"`
	SymlinkHops      []SymlinkHop `json:"symlink_hops"`
}

type SourceEntry = gitworktreesource.SourceEntry
type SourceManifest = gitworktreesource.SourceManifest

type commandProfile struct {
	Argv         []string
	Class        string
	EvidenceType string
}

type preparedProfiles struct {
	ChildEnvironment  []string
	Command           commandProfile
	Environment       EnvironmentManifest
	EnvironmentSHA256 string
	Root              string
	Source            SourceManifest
	SourceTreeSHA256  string
	TimeoutMS         *int64
	Tool              ToolManifest
	ToolSHA256        string
}

type capture struct {
	EndedAtUnixMS   int64
	StartedAtUnixMS int64
	Streams         commandcontract.Streams
	Termination     commandcontract.Termination
}

type ProductionPackage struct {
	APIVersion          string                      `json:"api_version"`
	Canonicalization    string                      `json:"canonicalization"`
	EnvironmentManifest EnvironmentManifest         `json:"environment_manifest"`
	Observation         commandcontract.Observation `json:"observation"`
	SourceManifest      SourceManifest              `json:"source_manifest"`
	ToolManifest        ToolManifest                `json:"tool_manifest"`
}

type Production struct {
	canonicalProductionJSON  []byte
	canonicalObservationJSON []byte
	packageValue             ProductionPackage
	productionSHA256         string
	result                   string
}

func (p *Production) ProductionJSON() []byte {
	if p == nil {
		return nil
	}
	return append([]byte(nil), p.canonicalProductionJSON...)
}

func (p *Production) ObservationJSON() []byte {
	if p == nil {
		return nil
	}
	return append([]byte(nil), p.canonicalObservationJSON...)
}

// Package returns a deep defensive copy of the sealed typed package.
func (p *Production) Package() ProductionPackage {
	if p == nil {
		return ProductionPackage{}
	}
	result := p.packageValue
	result.EnvironmentManifest = cloneEnvironmentManifest(result.EnvironmentManifest)
	result.ToolManifest = cloneToolManifest(result.ToolManifest)
	result.SourceManifest = cloneSourceManifest(result.SourceManifest)
	result.Observation.Command.Argv = append([]string(nil), result.Observation.Command.Argv...)
	result.Observation.Command.TimeoutMS = cloneInt64(result.Observation.Command.TimeoutMS)
	result.Observation.Termination.ExitCode = cloneInt64(result.Observation.Termination.ExitCode)
	return result
}

// SHA256 returns the domain-separated identity of ProductionJSON.
func (p *Production) SHA256() string {
	if p == nil {
		return ""
	}
	return p.productionSHA256
}

// Result returns the fixed non-capability result string.
func (p *Production) Result() string {
	if p == nil {
		return ""
	}
	return p.result
}
