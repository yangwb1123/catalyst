package knowledgeupdateproposalcontract

import (
	"fmt"

	"forgeos/forge-core/internal/governancecontract"
)

type recordSet struct {
	records []*governancecontract.Record
	byID    map[string]*governancecontract.Record
}

func validateRecordSet(values []any) (*recordSet, error) {
	if len(values) < 1 || len(values) > maxRecords {
		return nil, fmt.Errorf("records must contain 1..%d items", maxRecords)
	}
	if err := validateCanonicalByteLimit(values, maxRecordSetBytes, "embedded record set"); err != nil {
		return nil, err
	}
	canonical, err := canonicalJSON(values)
	if err != nil {
		return nil, err
	}
	records, err := governancecontract.DecodeRecordSet(canonical)
	if err != nil {
		return nil, fmt.Errorf("embedded governance record set: %w", err)
	}
	byID := make(map[string]*governancecontract.Record, len(records))
	for _, record := range records {
		byID[record.Header().RecordID] = record
	}
	return &recordSet{records: records, byID: byID}, nil
}

func validateProposalRecords(proposal map[string]any, parts *proposalParts, records *recordSet) error {
	claimed, err := stringValue(proposal, "record_set_sha256")
	computed, digestErr := digestValue(recordSetDomain, parts.records)
	if err != nil || validateHash(claimed, "record_set_sha256") != nil || digestErr != nil || claimed != computed {
		return fmt.Errorf("record_set_sha256 does not match exact embedded records")
	}
	if err := validateRecordScope(parts, records); err != nil {
		return err
	}
	if err := validateMutationRecords(parts, records); err != nil {
		return err
	}
	return validateExactClosure(parts.mutations, records)
}

func validateRecordScope(parts *proposalParts, records *recordSet) error {
	projectID, _ := stringValue(parts.task, "project_id")
	scope, _ := stringValue(parts.scope, "object_ref")
	for _, record := range records.records {
		header := record.Header()
		if header.ProjectID != projectID {
			return fmt.Errorf("record %q project_id does not match task_binding", header.RecordID)
		}
		if header.Scope != scope {
			return fmt.Errorf("record %q scope does not match knowledge_scope.object_ref", header.RecordID)
		}
	}
	return nil
}

func validateMutationRecords(parts *proposalParts, records *recordSet) error {
	for index, value := range parts.mutations {
		mutation := value.(map[string]any)
		afterRef := mutation["after_claim_ref"].(map[string]any)
		after, err := resolveClaimRef(afterRef, records)
		if err != nil {
			return fmt.Errorf("mutations[%d].after_claim_ref: %w", index, err)
		}
		if err := validateAfterBindings(parts, mutation, after); err != nil {
			return fmt.Errorf("mutations[%d]: %w", index, err)
		}
		operation, _ := stringValue(mutation, "operation")
		if operation == "create" {
			if err := validateCreate(after); err != nil {
				return fmt.Errorf("mutations[%d]: %w", index, err)
			}
			continue
		}
		beforeRef := mutation["before_claim_ref"].(map[string]any)
		before, err := resolveClaimRef(beforeRef, records)
		if err != nil {
			return fmt.Errorf("mutations[%d].before_claim_ref: %w", index, err)
		}
		if err := validateSupersede(mutation, before, after); err != nil {
			return fmt.Errorf("mutations[%d]: %w", index, err)
		}
	}
	return nil
}

func resolveClaimRef(reference map[string]any, records *recordSet) (*governancecontract.KnowledgeClaim, error) {
	recordID, _ := stringValue(reference, "record_id")
	digest, _ := stringValue(reference, "canonical_sha256")
	record, exists := records.byID[recordID]
	if !exists || record.Claim == nil {
		return nil, fmt.Errorf("must resolve to embedded KnowledgeClaim %q", recordID)
	}
	if record.Digest() != digest {
		return nil, fmt.Errorf("canonical_sha256 does not match record %q", recordID)
	}
	return record.Claim, nil
}

func validateAfterBindings(parts *proposalParts, mutation map[string]any, claim *governancecontract.KnowledgeClaim) error {
	target, _ := stringValue(mutation, "target_aggregate_id")
	if claim.Metadata.AggregateID != target {
		return fmt.Errorf("after Claim aggregate_id does not match target_aggregate_id")
	}
	expected := map[string]string{
		"context_sha256":     parts.bindings["context_sha256"].(string),
		"policy_sha256":      parts.bindings["policy_sha256"].(string),
		"source_revision":    parts.bindings["source_revision"].(string),
		"source_tree_sha256": parts.bindings["source_tree_sha256"].(string),
	}
	actual := map[string]string{
		"context_sha256":     claim.Metadata.ContextSHA256,
		"policy_sha256":      claim.Metadata.PolicySHA256,
		"source_revision":    claim.Metadata.SourceRevision,
		"source_tree_sha256": claim.Metadata.SourceTreeSHA256,
	}
	for key, value := range expected {
		if actual[key] != value {
			return fmt.Errorf("after Claim %s does not match proposal bindings", key)
		}
	}
	if claim.Metadata.CreatedAtUnixMS > parts.submitted {
		return fmt.Errorf("after Claim creation is later than proposal submission")
	}
	creator := claim.Metadata.CreatedBy
	if creator.AuthorityDomain != parts.proposer["authority_domain"] ||
		creator.PrincipalID != parts.proposer["principal_id"] ||
		creator.PrincipalType != parts.proposer["principal_type"] ||
		creator.Role != parts.task["role"] || creator.RunID != parts.task["run_id"] {
		return fmt.Errorf("after Claim creator does not match proposer/task declarations")
	}
	return nil
}

func validateCreate(after *governancecontract.KnowledgeClaim) error {
	if after.Metadata.Sequence != 1 || len(after.Metadata.SupersedesRecordIDs) != 0 {
		return fmt.Errorf("create requires sequence 1 and no supersedes_record_ids")
	}
	return nil
}

func validateSupersede(mutation map[string]any, before, after *governancecontract.KnowledgeClaim) error {
	target, _ := stringValue(mutation, "target_aggregate_id")
	if before.Metadata.AggregateID != target || after.Metadata.AggregateID != target {
		return fmt.Errorf("supersede Claims must share target_aggregate_id")
	}
	if after.Metadata.Sequence != before.Metadata.Sequence+1 {
		return fmt.Errorf("before Claim must be the exact immediate predecessor")
	}
	if !containsString(after.Metadata.SupersedesRecordIDs, before.Metadata.RecordID) {
		return fmt.Errorf("after Claim must supersede before Claim")
	}
	if err := validateStableClaimIdentity(before, after); err != nil {
		return err
	}
	return validateShadowSuccessor(before, after)
}

func validateStableClaimIdentity(before, after *governancecontract.KnowledgeClaim) error {
	stable := before.Spec.ClaimType == after.Spec.ClaimType &&
		before.Spec.Subject == after.Spec.Subject && before.Spec.Predicate == after.Spec.Predicate &&
		before.Spec.ObjectType == after.Spec.ObjectType && before.Spec.ObjectValue == after.Spec.ObjectValue &&
		before.Spec.Owner == after.Spec.Owner
	if !stable {
		return fmt.Errorf("supersede changes stable semantic Claim identity")
	}
	if before.Metadata.ProjectID != after.Metadata.ProjectID || before.Metadata.Scope != after.Metadata.Scope {
		return fmt.Errorf("supersede changes project_id or scope")
	}
	if after.Metadata.CreatedAtUnixMS < before.Metadata.CreatedAtUnixMS {
		return fmt.Errorf("supersede creation time moves backwards")
	}
	return nil
}

func validateShadowSuccessor(before, after *governancecontract.KnowledgeClaim) error {
	if before.Status.State == after.Status.State {
		return nil
	}
	transition := before.Status.State + "->" + after.Status.State
	allowed := map[string]map[string]bool{
		"fact":       {"candidate->contested": true, "contested->candidate": true},
		"assumption": {"open->testing": true}, "hypothesis": {"open->testing": true},
		"proposal": {"draft->submitted": true}, "unknown": {"open->investigating": true},
	}
	if !allowed[before.Spec.ClaimType][transition] {
		return fmt.Errorf("shadow lifecycle transition %q is not allowed for %s", transition, before.Spec.ClaimType)
	}
	return nil
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
