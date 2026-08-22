package knowledgeupdateproposalcontract

import "fmt"

var proposalKeys = []string{
	"api_version", "bindings", "canonicalization", "capability_grant_ref", "kind",
	"knowledge_scope", "mutations", "proposal_id", "proposal_sha256", "proposer",
	"record_set_sha256", "records", "submitted_at_unix_ms", "task_binding",
}

type proposalParts struct {
	bindings, grantRef, scope, proposer, task map[string]any
	mutations, records                        []any
	submitted                                 int64
}

func validateProposal(proposal map[string]any) error {
	if err := validateCanonicalByteLimit(proposal, maxProposalBytes, "KnowledgeUpdateProposal"); err != nil {
		return err
	}
	if err := requireKeys(proposal, proposalKeys...); err != nil {
		return fmt.Errorf("KnowledgeUpdateProposal: %w", err)
	}
	if err := validateProposalLiterals(proposal); err != nil {
		return err
	}
	parts, err := readProposalParts(proposal)
	if err != nil {
		return err
	}
	if err := validateProposalParts(parts); err != nil {
		return err
	}
	records, err := validateRecordSet(parts.records)
	if err != nil {
		return err
	}
	if err := validateProposalRecords(proposal, parts, records); err != nil {
		return err
	}
	if err := validateProposalIdentity(proposal); err != nil {
		return err
	}
	return nil
}

func validateProposalLiterals(proposal map[string]any) error {
	for key, expected := range map[string]string{
		"api_version": proposalAPI, "canonicalization": canonicalization, "kind": proposalKind,
	} {
		if err := requireStringLiteral(proposal, key, expected); err != nil {
			return err
		}
	}
	return nil
}

func readProposalParts(proposal map[string]any) (*proposalParts, error) {
	objects := make(map[string]map[string]any)
	for _, key := range []string{"bindings", "capability_grant_ref", "knowledge_scope", "proposer", "task_binding"} {
		value, err := objectValue(proposal, key)
		if err != nil {
			return nil, err
		}
		objects[key] = value
	}
	mutations, mutationErr := arrayValue(proposal, "mutations")
	records, recordErr := arrayValue(proposal, "records")
	submitted, timeErr := intValue(proposal, "submitted_at_unix_ms")
	if mutationErr != nil || recordErr != nil || timeErr != nil {
		return nil, fmt.Errorf("mutations/records/submitted_at_unix_ms have invalid types")
	}
	return &proposalParts{
		bindings: objects["bindings"], grantRef: objects["capability_grant_ref"],
		scope: objects["knowledge_scope"], proposer: objects["proposer"],
		task: objects["task_binding"], mutations: mutations, records: records, submitted: submitted,
	}, nil
}

func validateProposalParts(parts *proposalParts) error {
	if parts.submitted < 0 {
		return fmt.Errorf("submitted_at_unix_ms must be non-negative")
	}
	checks := []func() error{
		func() error { return validateBindings(parts.bindings, "bindings") },
		func() error { return validateGrantRef(parts.grantRef, "capability_grant_ref") },
		func() error { return validateKnowledgeScope(parts.scope, "knowledge_scope") },
		func() error { return validatePrincipal(parts.proposer, "proposer") },
		func() error { return validateTaskBinding(parts.task, "task_binding") },
		func() error { return validateMutationsShape(parts.mutations) },
	}
	for _, check := range checks {
		if err := check(); err != nil {
			return err
		}
	}
	return nil
}

func validateProposalIdentity(proposal map[string]any) error {
	claimed, err := stringValue(proposal, "proposal_sha256")
	if err != nil || validateHash(claimed, "proposal_sha256") != nil {
		return fmt.Errorf("proposal_sha256 is invalid")
	}
	identifier, err := stringValue(proposal, "proposal_id")
	if err != nil || identifier != "knowledge-update-proposal-"+claimed || len(identifier) != 90 {
		return fmt.Errorf("proposal_id must be the 90-byte identity derived from proposal_sha256")
	}
	computed, err := proposalDigest(proposal)
	if err != nil || computed != claimed {
		return fmt.Errorf("proposal_sha256 does not match canonical proposal preimage")
	}
	return nil
}

func proposalDigest(proposal map[string]any) (string, error) {
	preimage := cloneNode(proposal)
	preimage["proposal_id"] = ""
	preimage["proposal_sha256"] = ""
	if err := validateCanonicalByteLimit(preimage, maxProposalBytes, "KnowledgeUpdateProposal digest preimage"); err != nil {
		return "", err
	}
	return digestValue(proposalDomain, preimage)
}
