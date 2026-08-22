// Package graphsnapshot projects exact ADR-0053 graph-observation bytes into
// the authority-free, deliberately partial GraphSnapshot v1 contract. It does
// not inspect a repository, toolchain, network, clock, environment, or store.
package graphsnapshot

type Request struct {
	APIVersion                string `json:"api_version"`
	Canonicalization          string `json:"canonicalization"`
	GraphObservationBase64URL string `json:"graph_observation_base64url"`
	GraphObservationSHA256    string `json:"graph_observation_sha256"`
	ProjectID                 string `json:"project_id"`
	ProjectorProfileID        string `json:"projector_profile_id"`
	RequestSHA256             string `json:"request_sha256"`
	RunID                     string `json:"run_id"`
}

type Source struct {
	GraphAPIVersion          string `json:"graph_api_version"`
	GraphObservationSHA256   string `json:"graph_observation_sha256"`
	GraphProfileID           string `json:"graph_profile_id"`
	ObservedAtUnixMS         int64  `json:"observed_at_unix_ms"`
	ObserverParametersSHA256 string `json:"observer_parameters_sha256"`
	ObserverProducerID       string `json:"observer_producer_id"`
	ObserverProducerType     string `json:"observer_producer_type"`
	ObserverProducerVersion  string `json:"observer_producer_version"`
	ObserverRunID            string `json:"observer_run_id"`
	SourceID                 string `json:"source_id"`
	SourceIdentitySHA256     string `json:"source_identity_sha256"`
	SourceRevision           string `json:"source_revision"`
	SourceSHA256             string `json:"source_sha256"`
	SourceTreeSHA256         string `json:"source_tree_sha256"`
	SourceType               string `json:"source_type"`
}

type Extractor struct {
	ExtractorID             string `json:"extractor_id"`
	ExtractorIdentitySHA256 string `json:"extractor_identity_sha256"`
	ExtractorSHA256         string `json:"extractor_sha256"`
	ExtractorType           string `json:"extractor_type"`
	ExtractorVersion        string `json:"extractor_version"`
	InputGraphAPIVersion    string `json:"input_graph_api_version"`
	InputGraphProfileID     string `json:"input_graph_profile_id"`
	InputSourceID           string `json:"input_source_id"`
	ProducerID              string `json:"producer_id"`
	ProducerType            string `json:"producer_type"`
	ProducerVersion         string `json:"producer_version"`
	ProjectorProfileID      string `json:"projector_profile_id"`
}

type SourceLocator struct {
	ContentSHA256 *string `json:"content_sha256"`
	Path          string  `json:"path"`
	Role          string  `json:"role"`
	SourceID      string  `json:"source_id"`
}

type Node struct {
	ClaimRecordIDs          []string        `json:"claim_record_ids"`
	DataClassification      string          `json:"data_classification"`
	EpistemicStatus         string          `json:"epistemic_status"`
	EvidenceRecordIDs       []string        `json:"evidence_record_ids"`
	ExtractorSHA256s        []string        `json:"extractor_sha256s"`
	FreshnessStatus         string          `json:"freshness_status"`
	IdentityNamespace       string          `json:"identity_namespace"`
	IdentityProfileID       string          `json:"identity_profile_id"`
	LifecycleStatus         string          `json:"lifecycle_status"`
	NodeID                  string          `json:"node_id"`
	NodeIdentitySHA256      string          `json:"node_identity_sha256"`
	NodeSHA256              string          `json:"node_sha256"`
	NodeType                string          `json:"node_type"`
	OwnerNodeIDs            []string        `json:"owner_node_ids"`
	OwnerStatus             string          `json:"owner_status"`
	ProjectID               string          `json:"project_id"`
	ProvenanceStatus        string          `json:"provenance_status"`
	QualifiedNameComponents []string        `json:"qualified_name_components"`
	SourceIDs               []string        `json:"source_ids"`
	SourceLocators          []SourceLocator `json:"source_locators"`
	ValidityStatus          string          `json:"validity_status"`
}

type Edge struct {
	CategoryAxes          []string        `json:"category_axes"`
	ClaimRecordIDs        []string        `json:"claim_record_ids"`
	DataClassification    string          `json:"data_classification"`
	EdgeID                string          `json:"edge_id"`
	EdgeIdentitySHA256    string          `json:"edge_identity_sha256"`
	EdgeSHA256            string          `json:"edge_sha256"`
	EpistemicStatus       string          `json:"epistemic_status"`
	EvidenceRecordIDs     []string        `json:"evidence_record_ids"`
	ExtractorSHA256s      []string        `json:"extractor_sha256s"`
	FreshnessStatus       string          `json:"freshness_status"`
	FromNodeID            string          `json:"from_node_id"`
	IdentityProfileID     string          `json:"identity_profile_id"`
	ImportDiscriminator   *string         `json:"import_discriminator"`
	LifecycleStatus       string          `json:"lifecycle_status"`
	OwnerNodeIDs          []string        `json:"owner_node_ids"`
	OwnerStatus           string          `json:"owner_status"`
	ParallelDiscriminator string          `json:"parallel_discriminator"`
	ProvenanceStatus      string          `json:"provenance_status"`
	Relation              string          `json:"relation"`
	SourceIDs             []string        `json:"source_ids"`
	SourceLocators        []SourceLocator `json:"source_locators"`
	SourceRole            *string         `json:"source_role"`
	ToNodeID              string          `json:"to_node_id"`
	ValidityStatus        string          `json:"validity_status"`
}

type TargetCandidate struct {
	IdentityNamespace       string   `json:"identity_namespace"`
	IdentityProfileID       string   `json:"identity_profile_id"`
	QualifiedNameComponents []string `json:"qualified_name_components"`
	TargetNodeIDs           []string `json:"target_node_ids"`
}

type UnresolvedNode struct {
	CandidateIdentityNamespace       string          `json:"candidate_identity_namespace"`
	CandidateIdentityProfileID       string          `json:"candidate_identity_profile_id"`
	CandidateQualifiedNameComponents []string        `json:"candidate_qualified_name_components"`
	DiagnosticCode                   *string         `json:"diagnostic_code"`
	ExtractorSHA256s                 []string        `json:"extractor_sha256s"`
	Kind                             string          `json:"kind"`
	ProjectID                        string          `json:"project_id"`
	ReasonCode                       string          `json:"reason_code"`
	SourceIDs                        []string        `json:"source_ids"`
	SourceLocators                   []SourceLocator `json:"source_locators"`
	UnresolvedNodeID                 string          `json:"unresolved_node_id"`
	UnresolvedNodeIdentitySHA256     string          `json:"unresolved_node_identity_sha256"`
	UnresolvedNodeSHA256             string          `json:"unresolved_node_sha256"`
}

type UnresolvedEdge struct {
	CategoryAxes                 []string        `json:"category_axes"`
	EpistemicStatus              string          `json:"epistemic_status"`
	ExtractorSHA256s             []string        `json:"extractor_sha256s"`
	FromNodeID                   string          `json:"from_node_id"`
	IdentityProfileID            string          `json:"identity_profile_id"`
	ImportDiscriminator          string          `json:"import_discriminator"`
	ParallelDiscriminator        string          `json:"parallel_discriminator"`
	ProjectID                    string          `json:"project_id"`
	ReasonCode                   string          `json:"reason_code"`
	Relation                     string          `json:"relation"`
	Resolution                   string          `json:"resolution"`
	ResolutionDetail             *string         `json:"resolution_detail"`
	SourceIDs                    []string        `json:"source_ids"`
	SourceLocators               []SourceLocator `json:"source_locators"`
	SourceRole                   string          `json:"source_role"`
	TargetCandidate              TargetCandidate `json:"target_candidate"`
	UnresolvedEdgeID             string          `json:"unresolved_edge_id"`
	UnresolvedEdgeIdentitySHA256 string          `json:"unresolved_edge_identity_sha256"`
	UnresolvedEdgeSHA256         string          `json:"unresolved_edge_sha256"`
}

type Crosswalk struct {
	ADR0062NodeID     string `json:"adr_0062_node_id"`
	ADR0062NodeSHA256 string `json:"adr_0062_node_sha256"`
	GraphNodeID       string `json:"graph_node_id"`
}

type CoverageSurface struct {
	EdgeCount   int64    `json:"edge_count"`
	NodeCount   int64    `json:"node_count"`
	ReasonCodes []string `json:"reason_codes"`
	Status      string   `json:"status"`
	Surface     string   `json:"surface"`
}

type Coverage struct {
	Status       string            `json:"status"`
	SurfaceCount int64             `json:"surface_count"`
	Surfaces     []CoverageSurface `json:"surfaces"`
}

type Freshness struct {
	ExpiresAtUnixMS  int64    `json:"expires_at_unix_ms"`
	ObservedAtUnixMS int64    `json:"observed_at_unix_ms"`
	ReasonCodes      []string `json:"reason_codes"`
	Status           string   `json:"status"`
}

type Snapshot struct {
	ADR0062NodeCrosswalk     []Crosswalk      `json:"adr_0062_node_crosswalk"`
	APIVersion               string           `json:"api_version"`
	Canonicalization         string           `json:"canonicalization"`
	Coverage                 Coverage         `json:"coverage"`
	CoverageSHA256           string           `json:"coverage_sha256"`
	CrosswalkSetSHA256       string           `json:"crosswalk_set_sha256"`
	EdgeSetSHA256            string           `json:"edge_set_sha256"`
	Edges                    []Edge           `json:"edges"`
	ExtractorSetSHA256       string           `json:"extractor_set_sha256"`
	Extractors               []Extractor      `json:"extractors"`
	Freshness                Freshness        `json:"freshness"`
	NodeSetSHA256            string           `json:"node_set_sha256"`
	Nodes                    []Node           `json:"nodes"`
	ProfileID                string           `json:"profile_id"`
	ProjectID                string           `json:"project_id"`
	RequestSHA256            string           `json:"request_sha256"`
	Result                   string           `json:"result"`
	SnapshotID               string           `json:"snapshot_id"`
	SnapshotIdentitySHA256   string           `json:"snapshot_identity_sha256"`
	SnapshotSHA256           string           `json:"snapshot_sha256"`
	SourceSetSHA256          string           `json:"source_set_sha256"`
	Sources                  []Source         `json:"sources"`
	SystemKnowledgeStatus    string           `json:"system_knowledge_status"`
	SystemUnknownReasonCodes []string         `json:"system_unknown_reason_codes"`
	UnresolvedEdgeSetSHA256  string           `json:"unresolved_edge_set_sha256"`
	UnresolvedEdges          []UnresolvedEdge `json:"unresolved_edges"`
	UnresolvedNodeSetSHA256  string           `json:"unresolved_node_set_sha256"`
	UnresolvedNodes          []UnresolvedNode `json:"unresolved_nodes"`
}

type Envelope struct {
	APIVersion       string   `json:"api_version"`
	Canonicalization string   `json:"canonicalization"`
	EnvelopeSHA256   string   `json:"envelope_sha256"`
	Request          Request  `json:"request"`
	Snapshot         Snapshot `json:"snapshot"`
}

type Production struct {
	envelope     Envelope
	envelopeJSON []byte
	requestJSON  []byte
	snapshotJSON []byte
}
