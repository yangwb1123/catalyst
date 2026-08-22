package authenticatedadrapprovalcontract

import (
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"regexp"

	"forgeos/forge-core/internal/adrv2"
)

var proposalNamePattern = regexp.MustCompile(`^ADR-([0-9]{4})-[a-z0-9]+(?:-[a-z0-9]+)*\.md$`)

func validateProposalBinding(value any) (map[string]any, error) {
	label := "ArchitectureDecisionProposalBinding"
	fields := []string{"adr_id", "api_version", "body_sha256", "canonicalization",
		"document_name", "kind", "physical_sha256", "profile_id",
		"proposal_binding_sha256", "self_sha256", "status"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxProposalBindingBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != proposalBindingAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID || node["status"] != "proposed" {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if err = validateProposalBindingIdentity(node); err != nil {
		return nil, err
	}
	for _, field := range []string{"body_sha256", "physical_sha256", "proposal_binding_sha256", "self_sha256"} {
		if _, err = shaValue(node[field], "proposal_binding."+field); err != nil {
			return nil, err
		}
	}
	digest, err := proposalBindingSHA256(node)
	if err != nil || node["proposal_binding_sha256"] != digest {
		return nil, fmt.Errorf("proposal binding self digest does not match")
	}
	return node, nil
}

func validateProposalBindingIdentity(node map[string]any) error {
	adrID, ok := node["adr_id"].(string)
	if !ok || !validProposalADRID(adrID) {
		return fmt.Errorf("proposal_binding.adr_id is malformed")
	}
	name, ok := node["document_name"].(string)
	match := proposalNamePattern.FindStringSubmatch(name)
	if !ok || match == nil || match[1] != adrID[4:] {
		return fmt.Errorf("proposal_binding.document_name is malformed")
	}
	return nil
}

func validProposalADRID(value string) bool {
	if len(value) != 8 || value[:4] != "ADR-" || value == "ADR-0000" {
		return false
	}
	for _, character := range value[4:] {
		if character < '0' || character > '9' {
			return false
		}
	}
	return true
}

func deriveProposalBinding(proposal []byte, documentName string) (map[string]any, proposalMetadata, error) {
	metadata, err := validateProposalDocument(proposal, documentName)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	physical := sha256.Sum256(proposal)
	binding := map[string]any{
		"adr_id": metadata.ADRID, "api_version": proposalBindingAPI,
		"body_sha256": metadata.BodySHA256, "canonicalization": canonicalization,
		"document_name": metadata.DocumentName, "kind": "ArchitectureDecisionProposalBinding",
		"physical_sha256": hex.EncodeToString(physical[:]), "profile_id": profileID,
		"proposal_binding_sha256": "", "self_sha256": metadata.SelfSHA256,
		"status": "proposed",
	}
	digest, err := proposalBindingSHA256(binding)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	binding["proposal_binding_sha256"] = digest
	_, err = validateProposalBinding(binding)
	return binding, metadata, err
}

func validateProposalDocument(proposal []byte, documentName string) (proposalMetadata, error) {
	if len(proposal) == 0 || len(proposal) > maxProposalBytes {
		return proposalMetadata{}, fmt.Errorf("proposal bytes must contain 1..%d bytes", maxProposalBytes)
	}
	document, err := adrv2.ValidateDocument(documentName, proposal)
	if err != nil {
		return proposalMetadata{}, fmt.Errorf("proposal is not a strict Proposed ADR v2: %w", err)
	}
	frontmatter := document.Frontmatter
	return proposalMetadata{ADRID: frontmatter.ADRID,
		ApproverRefs: append([]string(nil), frontmatter.ApproverRefs...),
		BodySHA256:   frontmatter.BodySHA256, DocumentName: frontmatter.DocumentName,
		ExpiresAtUnixMS:  copyInt64Pointer(frontmatter.ExpiresAtUnixMS),
		OwnerRefs:        append([]string(nil), frontmatter.OwnerRefs...),
		ProposedAtUnixMS: frontmatter.ProposedAtUnixMS, SelfSHA256: frontmatter.SelfSHA256,
		Status: frontmatter.Status}, nil
}

func validateProposalBytes(proposal []byte, binding map[string]any) (proposalMetadata, error) {
	if _, err := validateProposalBinding(binding); err != nil {
		return proposalMetadata{}, err
	}
	derived, metadata, err := deriveProposalBinding(proposal, binding["document_name"].(string))
	if err != nil {
		return proposalMetadata{}, err
	}
	if !canonicalEqual(derived, binding) {
		return proposalMetadata{}, fmt.Errorf("exact proposal bytes do not match ProposalBinding")
	}
	return metadata, nil
}

func decodeProposalDocument(value any, binding map[string]any, label string) ([]byte, proposalMetadata, error) {
	proposal, err := decodeBase64URL(value, label, maxProposalBytes)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	metadata, err := validateProposalBytes(proposal, binding)
	return proposal, metadata, err
}

func encodeProposalDocument(proposal []byte) (string, error) {
	if len(proposal) == 0 || len(proposal) > maxProposalBytes {
		return "", fmt.Errorf("proposal bytes must contain 1..%d bytes", maxProposalBytes)
	}
	return base64.RawURLEncoding.EncodeToString(proposal), nil
}

func copyInt64Pointer(value *int64) *int64 {
	if value == nil {
		return nil
	}
	result := *value
	return &result
}
