package knowledgeupdateproposalcontract

import (
	"fmt"

	"forgeos/forge-core/internal/governancecontract"
)

func validateExactClosure(mutations []any, records *recordSet) error {
	stack := make([]string, 0, len(mutations))
	for _, value := range mutations {
		mutation := value.(map[string]any)
		reference := mutation["after_claim_ref"].(map[string]any)
		stack = append(stack, reference["record_id"].(string))
	}
	visited := make(map[string]bool, len(records.records))
	for len(stack) > 0 {
		last := len(stack) - 1
		recordID := stack[last]
		stack = stack[:last]
		if visited[recordID] {
			continue
		}
		record, exists := records.byID[recordID]
		if !exists {
			return fmt.Errorf("reachable closure references missing record %q", recordID)
		}
		visited[recordID] = true
		stack = append(stack, recordReferences(record)...)
	}
	if len(visited) != len(records.records) {
		for _, record := range records.records {
			if !visited[record.Header().RecordID] {
				return fmt.Errorf("record %q is outside the exact mutation closure", record.Header().RecordID)
			}
		}
	}
	return nil
}

func recordReferences(record *governancecontract.Record) []string {
	references := append([]string(nil), record.Header().SupersedesRecordIDs...)
	if record.Claim == nil {
		return references
	}
	references = append(references, record.Claim.Spec.SupportingEvidenceRecordIDs...)
	references = append(references, record.Claim.Spec.ContradictingEvidenceRecordIDs...)
	references = append(references, record.Claim.Spec.DerivedFromClaimRecordIDs...)
	return references
}
