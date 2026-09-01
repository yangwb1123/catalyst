package application

import (
	"sort"

	core "forgeos/forge-core/internal/platformcorecontract"
)

const maxAssessmentEvidenceRefs = 16

func validateEvidence(values []core.RecordRef) error {
	if len(values) < 1 || len(values) > maxAssessmentEvidenceRefs {
		return invalidSnapshot("assessment evidence count is outside its bound", nil)
	}
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if err := core.ValidateReference(value); err != nil {
			return invalidSnapshot("assessment evidence reference is invalid", err)
		}
		identity := value.RecordType + "\x00" + value.RecordID
		if _, exists := seen[identity]; exists {
			return invalidSnapshot("assessment evidence contains a duplicate identity", nil)
		}
		seen[identity] = struct{}{}
	}
	return nil
}

func validateEvidenceConsistency(
	values []core.RecordRef, known map[string]string,
) error {
	for _, value := range values {
		identity := value.RecordType + "\x00" + value.RecordID
		digest, exists := known[identity]
		if exists && digest != value.RecordSHA256 {
			return invalidSnapshot("assessment evidence identity has conflicting digests", nil)
		}
		known[identity] = value.RecordSHA256
	}
	return nil
}

func sortedEvidence(values []core.RecordRef) []core.RecordRef {
	result := append([]core.RecordRef(nil), values...)
	sort.Slice(result, func(left, right int) bool {
		if result[left].RecordType != result[right].RecordType {
			return result[left].RecordType < result[right].RecordType
		}
		if result[left].RecordID != result[right].RecordID {
			return result[left].RecordID < result[right].RecordID
		}
		return result[left].RecordSHA256 < result[right].RecordSHA256
	})
	return result
}

func sameStringSet(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	leftCopy, rightCopy := append([]string(nil), left...), append([]string(nil), right...)
	sort.Strings(leftCopy)
	sort.Strings(rightCopy)
	for index := range leftCopy {
		if leftCopy[index] != rightCopy[index] {
			return false
		}
	}
	return true
}
