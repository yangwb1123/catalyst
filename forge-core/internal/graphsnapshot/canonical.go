package graphsnapshot

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
)

type sourceIdentity struct {
	GraphAPIVersion        string `json:"graph_api_version"`
	GraphObservationSHA256 string `json:"graph_observation_sha256"`
	GraphProfileID         string `json:"graph_profile_id"`
	ObserverRunID          string `json:"observer_run_id"`
	SourceRevision         string `json:"source_revision"`
	SourceTreeSHA256       string `json:"source_tree_sha256"`
	SourceType             string `json:"source_type"`
}

type extractorIdentity struct {
	ExtractorType      string `json:"extractor_type"`
	ExtractorVersion   string `json:"extractor_version"`
	ProducerID         string `json:"producer_id"`
	ProjectorProfileID string `json:"projector_profile_id"`
}

type nodeIdentity struct {
	IdentityNamespace       string   `json:"identity_namespace"`
	IdentityProfileID       string   `json:"identity_profile_id"`
	NodeType                string   `json:"node_type"`
	ProjectID               string   `json:"project_id"`
	QualifiedNameComponents []string `json:"qualified_name_components"`
}

type edgeIdentity struct {
	CategoryAxes          []string `json:"category_axes"`
	FromNodeID            string   `json:"from_node_id"`
	IdentityProfileID     string   `json:"identity_profile_id"`
	ImportDiscriminator   *string  `json:"import_discriminator"`
	ParallelDiscriminator string   `json:"parallel_discriminator"`
	Relation              string   `json:"relation"`
	SourceRole            *string  `json:"source_role"`
	ToNodeID              string   `json:"to_node_id"`
}

type unresolvedNodeIdentity struct {
	CandidateIdentityNamespace       string   `json:"candidate_identity_namespace"`
	CandidateIdentityProfileID       string   `json:"candidate_identity_profile_id"`
	CandidateQualifiedNameComponents []string `json:"candidate_qualified_name_components"`
	Kind                             string   `json:"kind"`
	ProjectID                        string   `json:"project_id"`
	ReasonCode                       string   `json:"reason_code"`
}

type unresolvedEdgeIdentity struct {
	CategoryAxes          []string        `json:"category_axes"`
	FromNodeID            string          `json:"from_node_id"`
	IdentityProfileID     string          `json:"identity_profile_id"`
	ImportDiscriminator   string          `json:"import_discriminator"`
	ParallelDiscriminator string          `json:"parallel_discriminator"`
	ProjectID             string          `json:"project_id"`
	ReasonCode            string          `json:"reason_code"`
	Relation              string          `json:"relation"`
	Resolution            string          `json:"resolution"`
	ResolutionDetail      *string         `json:"resolution_detail"`
	SourceRole            string          `json:"source_role"`
	TargetCandidate       TargetCandidate `json:"target_candidate"`
}

type snapshotIdentity struct {
	CoverageSHA256          string `json:"coverage_sha256"`
	CrosswalkSetSHA256      string `json:"crosswalk_set_sha256"`
	EdgeSetSHA256           string `json:"edge_set_sha256"`
	ExtractorSetSHA256      string `json:"extractor_set_sha256"`
	NodeSetSHA256           string `json:"node_set_sha256"`
	ProfileID               string `json:"profile_id"`
	ProjectID               string `json:"project_id"`
	RequestSHA256           string `json:"request_sha256"`
	SourceSetSHA256         string `json:"source_set_sha256"`
	UnresolvedEdgeSetSHA256 string `json:"unresolved_edge_set_sha256"`
	UnresolvedNodeSetSHA256 string `json:"unresolved_node_set_sha256"`
}

type adr0062NodeIdentity struct {
	Directory   string  `json:"directory"`
	ImportPath  *string `json:"import_path"`
	ModulePath  string  `json:"module_path"`
	PackageName string  `json:"package_name"`
}

type setPreimage[T any] struct {
	ItemCount int64 `json:"item_count"`
	Items     []T   `json:"items"`
}

func canonicalJSON(value any, maxBytes int) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, fmt.Errorf("encode canonical JSON: %w", err)
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("canonical encoder did not terminate predictably")
	}
	encoded = encoded[:len(encoded)-1]
	if len(encoded) > maxBytes {
		return nil, fmt.Errorf("canonical JSON exceeds %d bytes", maxBytes)
	}
	return append([]byte(nil), encoded...), nil
}

func digestValue(domain string, value any) (string, error) {
	encoded, err := canonicalJSON(value, maxSnapshotBytes)
	if err != nil {
		return "", err
	}
	return domainDigest(domain, encoded), nil
}

func setDigest[T any](domain string, values []T) (string, error) {
	return digestValue(domain, setPreimage[T]{ItemCount: int64(len(values)), Items: values})
}

func domainDigest(domain string, encoded []byte) string {
	hasher := sha256.New()
	_, _ = hasher.Write([]byte(domain))
	_, _ = hasher.Write([]byte{0})
	_, _ = hasher.Write(encoded)
	return hex.EncodeToString(hasher.Sum(nil))
}
