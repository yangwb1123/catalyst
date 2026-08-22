package authenticatedadrlifecyclecontract

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"

	"forgeos/forge-core/internal/adrv2"
)

const (
	proposalBindingAPI = "forgeos.architecture-decision-proposal-binding/v1"
	approvalProfileID  = "authenticated_architecture_decision_approval_v1"
)

func validateProposalBinding(value any) (map[string]any, error) {
	label := "ArchitectureDecisionProposalBinding"
	fields := []string{"adr_id", "api_version", "body_sha256", "canonicalization",
		"document_name", "kind", "physical_sha256", "profile_id",
		"proposal_binding_sha256", "self_sha256", "status"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, 64*1024, label); err != nil {
		return nil, err
	}
	if node["api_version"] != proposalBindingAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != approvalProfileID ||
		node["status"] != "proposed" {
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
	adrID, err := adrIDValue(node["adr_id"], "proposal_binding.adr_id")
	if err != nil {
		return err
	}
	name, ok := node["document_name"].(string)
	match := proposalNamePattern.FindStringSubmatch(name)
	if !ok || match == nil || match[1] != adrID[4:] {
		return fmt.Errorf("proposal_binding.document_name is malformed")
	}
	return nil
}

func deriveProposalBinding(proposal []byte, documentName string) (map[string]any, proposalMetadata, error) {
	document, err := adrv2.ValidateDocument(documentName, proposal)
	if err != nil {
		return nil, proposalMetadata{}, fmt.Errorf("proposal is not a strict Proposed ADR v2: %w", err)
	}
	frontmatter := document.Frontmatter
	physical := sha256.Sum256(proposal)
	binding := map[string]any{
		"adr_id": frontmatter.ADRID, "api_version": proposalBindingAPI,
		"body_sha256": frontmatter.BodySHA256, "canonicalization": canonicalization,
		"document_name": frontmatter.DocumentName, "kind": "ArchitectureDecisionProposalBinding",
		"physical_sha256": hex.EncodeToString(physical[:]), "profile_id": approvalProfileID,
		"proposal_binding_sha256": "", "self_sha256": frontmatter.SelfSHA256,
		"status": "proposed",
	}
	digest, err := proposalBindingSHA256(binding)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	binding["proposal_binding_sha256"] = digest
	metadata := metadataFromDocument(document, hex.EncodeToString(physical[:]))
	if _, err = validateProposalBinding(binding); err != nil {
		return nil, proposalMetadata{}, err
	}
	return binding, metadata, nil
}

func metadataFromDocument(document *adrv2.Document, physical string) proposalMetadata {
	frontmatter := document.Frontmatter
	return proposalMetadata{ADRID: frontmatter.ADRID, BodySHA256: frontmatter.BodySHA256,
		DocumentName:    frontmatter.DocumentName,
		ExpiresAtUnixMS: copyInt64Pointer(frontmatter.ExpiresAtUnixMS),
		PhysicalSHA256:  physical, ProposedAtUnixMS: frontmatter.ProposedAtUnixMS,
		SelfSHA256: frontmatter.SelfSHA256,
		Supersedes: append([]string(nil), frontmatter.Supersedes...)}
}

func decodeProposalDocument(value any, binding map[string]any,
	label string) ([]byte, proposalMetadata, error) {
	proposal, err := decodeBase64URL(value, label, maxProposalBytes)
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	derived, metadata, err := deriveProposalBinding(proposal, binding["document_name"].(string))
	if err != nil {
		return nil, proposalMetadata{}, err
	}
	validated, err := validateProposalBinding(binding)
	if err != nil || !canonicalEqual(derived, validated) {
		return nil, proposalMetadata{}, fmt.Errorf("%s does not match the exact ProposalBinding", label)
	}
	return proposal, metadata, nil
}

func copyInt64Pointer(value *int64) *int64 {
	if value == nil {
		return nil
	}
	result := *value
	return &result
}
