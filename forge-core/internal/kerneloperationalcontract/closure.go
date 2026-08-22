package kerneloperationalcontract

import (
	"fmt"
	"sort"
)

func validateClosureCounts(value *KernelOperationalReferenceClosure) error {
	if value.Artifacts == nil || len(value.Artifacts) > maxArtifacts {
		return fmt.Errorf("artifacts cardinality must be 0..%d", maxArtifacts)
	}
	if value.ArtifactReceipts == nil || len(value.ArtifactReceipts) > maxArtifactReceipts {
		return fmt.Errorf("artifact_receipts cardinality must be 0..%d", maxArtifactReceipts)
	}
	if value.CapabilityInvocations == nil || len(value.CapabilityInvocations) < 1 ||
		len(value.CapabilityInvocations) > maxInvocations {
		return fmt.Errorf("capability_invocations cardinality must be 1..%d", maxInvocations)
	}
	if value.InteractionEvents == nil || len(value.InteractionEvents) > maxEvents {
		return fmt.Errorf("interaction_events cardinality must be 0..%d", maxEvents)
	}
	if value.ExecutionReceipts == nil || len(value.ExecutionReceipts) < 1 ||
		len(value.ExecutionReceipts) > maxExecutionReceipts {
		return fmt.Errorf("execution_receipts cardinality must be 1..%d", maxExecutionReceipts)
	}
	return nil
}

func validateClosureRecords(value *KernelOperationalReferenceClosure) error {
	if err := validateSortedUnique(value.Artifacts, "artifacts", maxArtifacts,
		false, validateArtifact); err != nil {
		return err
	}
	for index := range value.ArtifactReceipts {
		if err := validateArtifactReceipt(&value.ArtifactReceipts[index]); err != nil {
			return fmt.Errorf("artifact_receipts[%d]: %w", index, err)
		}
	}
	for index := range value.CapabilityInvocations {
		if err := validateInvocation(&value.CapabilityInvocations[index]); err != nil {
			return fmt.Errorf("capability_invocations[%d]: %w", index, err)
		}
	}
	for index := range value.InteractionEvents {
		if err := validateEvent(&value.InteractionEvents[index]); err != nil {
			return fmt.Errorf("interaction_events[%d]: %w", index, err)
		}
	}
	for index := range value.ExecutionReceipts {
		if err := validateExecutionReceipt(&value.ExecutionReceipts[index]); err != nil {
			return fmt.Errorf("execution_receipts[%d]: %w", index, err)
		}
	}
	return nil
}

func validateArtifactReceiptOrder(values []ArtifactReceipt) error {
	identities := make([]string, len(values))
	for index, value := range values {
		identities[index] = value.ArtifactReceiptID
	}
	if !sort.StringsAreSorted(identities) || hasDuplicate(identities) {
		return fmt.Errorf("artifact_receipts must be strictly identity-sorted and unique")
	}
	return nil
}

func validateClosureFields(value *KernelOperationalReferenceClosure, blank bool) error {
	if value == nil {
		return fmt.Errorf("KernelOperationalReferenceClosure is nil")
	}
	if err := validateRecordHeader(value.APIVersion, value.Kind, closureAPI, closureKind); err != nil {
		return err
	}
	if value.Canonicalization != canonicalization || value.Result != successMarker {
		return fmt.Errorf("closure canonicalization or result marker differs")
	}
	if err := validateIdentity(value.ClosureID, value.ClosureSHA256,
		closurePrefix, "closure", blank); err != nil {
		return err
	}
	if err := validateAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateClosureCounts(value); err != nil {
		return err
	}
	if err := validateClosureRecords(value); err != nil {
		return err
	}
	if err := validateArtifactReceiptOrder(value.ArtifactReceipts); err != nil {
		return err
	}
	return validateReferenceGraph(value)
}

func closureDigest(value *KernelOperationalReferenceClosure) (string, error) {
	blank, err := cloneValue(value)
	if err != nil {
		return "", err
	}
	blank.ClosureID, blank.ClosureSHA256 = "", ""
	if err := validateClosureFields(blank, true); err != nil {
		return "", err
	}
	return typedDigest(blank, closureDomain, maxClosureBytes)
}

// ValidateClosure validates every record, digest, projection, and supplied DAG edge.
func ValidateClosure(value *KernelOperationalReferenceClosure) error {
	if err := validateClosureFields(value, false); err != nil {
		return err
	}
	digest, err := closureDigest(value)
	if err != nil {
		return err
	}
	if value.ClosureSHA256 != digest {
		return fmt.Errorf("closure_sha256 does not match canonical preimage")
	}
	_, err = canonicalTyped(value, maxClosureBytes)
	return err
}

// SealClosure seals one exact blank-identity nonsemantic reference closure copy.
func SealClosure(value *KernelOperationalReferenceClosure) (*KernelOperationalReferenceClosure, error) {
	if value == nil || value.ClosureID != "" || value.ClosureSHA256 != "" {
		return nil, fmt.Errorf("sealing closure requires blank identity fields")
	}
	sealed, err := cloneValue(value)
	if err != nil {
		return nil, err
	}
	digest, err := closureDigest(sealed)
	if err != nil {
		return nil, err
	}
	sealed.ClosureID, sealed.ClosureSHA256 = closurePrefix+digest, digest
	return sealed, ValidateClosure(sealed)
}

// DecodeClosure decodes one exact compact canonical closure with no trailing LF.
func DecodeClosure(data []byte) (*KernelOperationalReferenceClosure, error) {
	var value KernelOperationalReferenceClosure
	if err := decodeTypedExact(data, maxClosureBytes, &value); err != nil {
		return nil, err
	}
	return &value, ValidateClosure(&value)
}
