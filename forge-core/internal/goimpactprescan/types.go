// Package goimpactprescan derives a bounded, advisory reverse-dependency
// prescan from exact ADR-0053 graph-observation bytes. It performs no source,
// Go toolchain, network, persistence, or authority effect.
package goimpactprescan

const (
	APIVersion        = "forgeos.governance.local-go-package-impact-prescan/v1"
	RequestAPIVersion = "forgeos.governance.local-go-package-impact-prescan-request/v1"
	ReportAPIVersion  = "forgeos.governance.local-go-package-impact-prescan-report/v1"
	Canonicalization  = "forgeos.canonical-json/v1"
	Complete          = "complete_within_observation"
	Unknown           = "unknown"
	Result            = "LOCAL_GO_PACKAGE_IMPACT_PRESCAN_ONLY (exact ADR-0053 lexical reverse dependency closure; system impact unknown; no selected-build, truth, authority, completion, persistence, execution, or effect attestation)"

	maxChangedPaths       = 256
	maxGraphBytes         = 16 << 20
	maxRequestBytes       = 24 << 20
	maxReportBytes        = 16 << 20
	maxEnvelopeBytes      = 48 << 20
	maxReachableNodes     = 16_384
	maxReachableEdges     = 65_536
	maxWitnessHops        = 1_024
	maxAggregateWitnesses = 65_536
	requestDigestDomain   = "forgeos.governance.local-go-package-impact-prescan-request.v1"
	nodeDigestDomain      = "forgeos.governance.local-go-package-impact-prescan-node.v1"
	edgeDigestDomain      = "forgeos.governance.local-go-package-impact-prescan-edge.v1"
	reportDigestDomain    = "forgeos.governance.local-go-package-impact-prescan-report.v1"
	envelopeDigestDomain  = "forgeos.governance.local-go-package-impact-prescan.v1"
)

var systemUnknownReasons = []string{
	"api_event_contract_surfaces_not_observed",
	"call_and_runtime_reachability_not_observed",
	"data_and_migration_surfaces_not_observed",
	"deployment_and_operations_surfaces_not_observed",
	"owner_adr_policy_surfaces_not_observed",
	"selected_build_semantics_not_observed",
}

type Request struct {
	APIVersion                string   `json:"api_version"`
	Canonicalization          string   `json:"canonicalization"`
	ChangedPaths              []string `json:"changed_paths"`
	GraphObservationBase64URL string   `json:"graph_observation_base64url"`
	GraphObservationSHA256    string   `json:"graph_observation_sha256"`
	RequestSHA256             string   `json:"request_sha256"`
	RunID                     string   `json:"run_id"`
}

type ResolvedSeed struct {
	ChangedPaths []string `json:"changed_paths"`
	NodeID       string   `json:"node_id"`
}

type UnresolvedSeed struct {
	ChangedPath    string  `json:"changed_path"`
	DiagnosticCode *string `json:"diagnostic_code"`
	Reason         string  `json:"reason"`
}

type Witness struct {
	EdgeIDs    []string `json:"edge_ids"`
	HopCount   int      `json:"hop_count"`
	NodeIDs    []string `json:"node_ids"`
	SeedNodeID string   `json:"seed_node_id"`
}

type ReachableNode struct {
	Directory   string  `json:"directory"`
	ImportPath  *string `json:"import_path"`
	ModulePath  string  `json:"module_path"`
	NodeID      string  `json:"node_id"`
	NodeSHA256  string  `json:"node_sha256"`
	PackageName string  `json:"package_name"`
	Witness     Witness `json:"witness"`
}

type ReachableEdge struct {
	EdgeID      string   `json:"edge_id"`
	EdgeSHA256  string   `json:"edge_sha256"`
	FromNodeID  string   `json:"from_node_id"`
	ImportPath  string   `json:"import_path"`
	Relation    string   `json:"relation"`
	Role        string   `json:"role"`
	SourcePaths []string `json:"source_paths"`
	ToNodeID    string   `json:"to_node_id"`
}

type Report struct {
	APIVersion                  string           `json:"api_version"`
	Canonicalization            string           `json:"canonicalization"`
	ClosureReasonCodes          []string         `json:"closure_reason_codes"`
	GraphObservationSHA256      string           `json:"graph_observation_sha256"`
	PackageLexicalClosureStatus string           `json:"package_lexical_closure_status"`
	ReachableEdges              []ReachableEdge  `json:"reachable_edges"`
	ReachableNodes              []ReachableNode  `json:"reachable_nodes"`
	ReportSHA256                string           `json:"report_sha256"`
	RequestSHA256               string           `json:"request_sha256"`
	ResolvedSeeds               []ResolvedSeed   `json:"resolved_seeds"`
	Result                      string           `json:"result"`
	RunID                       string           `json:"run_id"`
	SystemImpactStatus          string           `json:"system_impact_status"`
	SystemUnknownReasonCodes    []string         `json:"system_unknown_reason_codes"`
	UnresolvedSeeds             []UnresolvedSeed `json:"unresolved_seeds"`
}

type Envelope struct {
	APIVersion       string  `json:"api_version"`
	Canonicalization string  `json:"canonicalization"`
	EnvelopeSHA256   string  `json:"envelope_sha256"`
	Report           Report  `json:"report"`
	Request          Request `json:"request"`
}

type Production struct {
	envelope     Envelope
	envelopeJSON []byte
	reportJSON   []byte
	requestJSON  []byte
}

func (value *Production) JSON() []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value.envelopeJSON...)
}

func (value *Production) RequestJSON() []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value.requestJSON...)
}

func (value *Production) ReportJSON() []byte {
	if value == nil {
		return nil
	}
	return append([]byte(nil), value.reportJSON...)
}

func (value *Production) SHA256() string {
	if value == nil {
		return ""
	}
	return value.envelope.EnvelopeSHA256
}

func (value *Production) ReportSHA256() string {
	if value == nil {
		return ""
	}
	return value.envelope.Report.ReportSHA256
}

func (value *Production) Envelope() Envelope {
	if value == nil {
		return Envelope{}
	}
	return cloneEnvelope(value.envelope)
}
