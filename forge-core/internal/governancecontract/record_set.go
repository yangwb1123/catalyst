package governancecontract

import (
	"bytes"
	"fmt"
	"strconv"
)

func DecodeRecordSet(data []byte) ([]*Record, error) {
	node, err := parseStrictJSONBounded(data, maxSetBytes)
	if err != nil {
		return nil, fmt.Errorf("governance record set JSON: %w", err)
	}
	array, ok := node.([]any)
	if !ok || len(array) == 0 {
		return nil, fmt.Errorf("governance record set must be a nonempty array")
	}
	canonical, err := canonicalJSON(array)
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("record set input is not exact compact canonical JSON")
	}
	records, err := decodeRecordNodes(array)
	if err != nil {
		return nil, err
	}
	if err := ValidateRecordSet(records); err != nil {
		return nil, err
	}
	return records, nil
}

func decodeRecordNodes(nodes []any) ([]*Record, error) {
	records := make([]*Record, 0, len(nodes))
	for index, node := range nodes {
		encoded, err := canonicalJSON(node)
		if err != nil {
			return nil, fmt.Errorf("record %d: %w", index, err)
		}
		if len(encoded) > maxRecordBytes {
			return nil, fmt.Errorf("record %d exceeds %d bytes", index, maxRecordBytes)
		}
		record, err := DecodeRecord(encoded)
		if err != nil {
			return nil, fmt.Errorf("record %d: %w", index, err)
		}
		records = append(records, record)
	}
	return records, nil
}

func ValidateRecordSet(records []*Record) error {
	if len(records) == 0 || len(records) > maxArrayItems {
		return fmt.Errorf("record set must contain 1..%d records", maxArrayItems)
	}
	byID, err := indexRecords(records)
	if err != nil {
		return err
	}
	if err := detectSupersessionCycles(records, byID); err != nil {
		return err
	}
	if err := detectClaimDerivationCycles(records, byID); err != nil {
		return err
	}
	for _, record := range records {
		if err := validateSupersession(record, byID); err != nil {
			return err
		}
		if record.Claim != nil {
			if err := validateClaimReferences(record.Claim, byID); err != nil {
				return err
			}
		}
	}
	return nil
}

func indexRecords(records []*Record) (map[string]*Record, error) {
	byID := make(map[string]*Record, len(records))
	tuples := make(map[string]bool, len(records))
	totalBytes, previousID := len(records)+1, ""
	for index, record := range records {
		recordBytes, err := validateSetRecord(record)
		if err != nil {
			return nil, fmt.Errorf("record %d: %w", index, err)
		}
		header := record.Header()
		if index > 0 && header.RecordID <= previousID {
			return nil, fmt.Errorf("record set must be sorted by unique record_id")
		}
		if _, exists := byID[header.RecordID]; exists {
			return nil, fmt.Errorf("duplicate record_id %q", header.RecordID)
		}
		tuple := record.Kind() + "\x00" + header.AggregateID + "\x00" + strconv.FormatInt(header.Sequence, 10)
		if tuples[tuple] {
			return nil, fmt.Errorf("duplicate kind/aggregate_id/sequence tuple")
		}
		byID[header.RecordID], tuples[tuple] = record, true
		totalBytes, previousID = totalBytes+recordBytes, header.RecordID
	}
	if totalBytes > maxSetBytes {
		return nil, fmt.Errorf("record set exceeds %d canonical bytes", maxSetBytes)
	}
	return byID, nil
}

func validateSetRecord(record *Record) (int, error) {
	state, err := verifiedCanonicalState(record)
	if err != nil {
		return 0, err
	}
	applyCanonicalState(record, state)
	return len(state.record), nil
}

func validateSupersession(record *Record, byID map[string]*Record) error {
	header := record.Header()
	if header.Sequence == 1 && len(header.SupersedesRecordIDs) != 0 {
		return fmt.Errorf("sequence 1 record %q cannot supersede records", header.RecordID)
	}
	if header.Sequence > 1 && len(header.SupersedesRecordIDs) == 0 {
		return fmt.Errorf("sequence %d record %q must supersede records", header.Sequence, header.RecordID)
	}
	hasPriorSequence := false
	for _, priorID := range header.SupersedesRecordIDs {
		prior, exists := byID[priorID]
		if !exists {
			return fmt.Errorf("record %q supersedes missing record %q", header.RecordID, priorID)
		}
		priorHeader := prior.Header()
		if prior.Kind() != record.Kind() || priorHeader.AggregateID != header.AggregateID || priorHeader.Sequence >= header.Sequence {
			return fmt.Errorf("record %q has invalid supersession target %q", header.RecordID, priorID)
		}
		hasPriorSequence = hasPriorSequence || priorHeader.Sequence == header.Sequence-1
	}
	if header.Sequence > 1 && !hasPriorSequence {
		return fmt.Errorf("record %q must supersede a sequence %d record", header.RecordID, header.Sequence-1)
	}
	return nil
}

func detectSupersessionCycles(records []*Record, byID map[string]*Record) error {
	visiting, visited := make(map[string]bool), make(map[string]bool)
	var visit func(string) error
	visit = func(recordID string) error {
		if visiting[recordID] {
			return fmt.Errorf("supersession cycle includes record %q", recordID)
		}
		if visited[recordID] {
			return nil
		}
		visiting[recordID] = true
		for _, priorID := range byID[recordID].Header().SupersedesRecordIDs {
			if _, exists := byID[priorID]; exists {
				if err := visit(priorID); err != nil {
					return err
				}
			}
		}
		delete(visiting, recordID)
		visited[recordID] = true
		return nil
	}
	for _, record := range records {
		if err := visit(record.Header().RecordID); err != nil {
			return err
		}
	}
	return nil
}

func validateClaimReferences(claim *KnowledgeClaim, byID map[string]*Record) error {
	for _, recordID := range append(append([]string{}, claim.Spec.SupportingEvidenceRecordIDs...), claim.Spec.ContradictingEvidenceRecordIDs...) {
		referenced, exists := byID[recordID]
		if !exists || referenced.Evidence == nil {
			return fmt.Errorf("claim %q references missing EvidenceRecord %q", claim.Metadata.RecordID, recordID)
		}
		if !containsString(referenced.Evidence.Spec.Subjects, claim.Spec.Subject) {
			return fmt.Errorf("EvidenceRecord %q does not contain claim subject %q", recordID, claim.Spec.Subject)
		}
	}
	for _, recordID := range claim.Spec.DerivedFromClaimRecordIDs {
		referenced, exists := byID[recordID]
		if !exists || referenced.Claim == nil {
			return fmt.Errorf("claim %q references missing KnowledgeClaim %q", claim.Metadata.RecordID, recordID)
		}
	}
	return nil
}

func detectClaimDerivationCycles(records []*Record, byID map[string]*Record) error {
	visiting, visited := make(map[string]bool), make(map[string]bool)
	var visit func(string) error
	visit = func(recordID string) error {
		if visiting[recordID] {
			return fmt.Errorf("claim derivation cycle includes record %q", recordID)
		}
		if visited[recordID] {
			return nil
		}
		visiting[recordID] = true
		for _, derivedID := range byID[recordID].Claim.Spec.DerivedFromClaimRecordIDs {
			if derivedID == recordID {
				return fmt.Errorf("claim %q cannot derive from itself", recordID)
			}
			if target, exists := byID[derivedID]; exists && target.Claim != nil {
				if err := visit(derivedID); err != nil {
					return err
				}
			}
		}
		delete(visiting, recordID)
		visited[recordID] = true
		return nil
	}
	for _, record := range records {
		if record.Claim != nil {
			if err := visit(record.Header().RecordID); err != nil {
				return err
			}
		}
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
