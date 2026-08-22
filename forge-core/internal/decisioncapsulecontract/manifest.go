package decisioncapsulecontract

import (
	"fmt"
	"reflect"

	kd "forgeos/forge-core/internal/kerneldecisioncontract"
	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

func validateManifestLocal(value *StructuralReplayManifest, allowBlank bool) error {
	if value == nil {
		return fmt.Errorf("StructuralReplayManifest is nil")
	}
	checks := []func() error{
		func() error { return validateManifestHeader(value, allowBlank) },
		func() error { return validateManifestAtoms(value) },
		func() error { return validateManifestArtifacts(value) },
		func() error { return validateManifestAttemptRefs(value) },
		func() error { return validateManifestEventReceiptRefs(value) },
	}
	for _, check := range checks {
		if err := check(); err != nil {
			return err
		}
	}
	return nil
}

func validateManifestShape(value *StructuralReplayManifest, allowBlank bool) error {
	if err := validateManifestLocal(value, allowBlank); err != nil {
		return err
	}
	_, err := measureTypedJSON(value, maxManifestBytes)
	return err
}

func validateManifestHeader(value *StructuralReplayManifest, allowBlank bool) error {
	if value.APIVersion != manifestAPI || value.Canonicalization != canonicalization ||
		value.Kind != manifestKind || value.ReplayMode != manifestMode {
		return fmt.Errorf("StructuralReplayManifest constants differ")
	}
	if value.EffectReplayAllowed || value.HistoryRewriteAllowed {
		return fmt.Errorf("manifest effect replay and history rewrite controls must be false")
	}
	if err := validateIdentity(value.ManifestID, value.ManifestSHA256,
		manifestPrefix, "manifest", allowBlank); err != nil {
		return err
	}
	if err := validateReplayAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateReference(value.DecisionClosureRef.ClosureID,
		value.DecisionClosureRef.ClosureSHA256, decisionClosurePrefix,
		"decision_closure_ref"); err != nil {
		return err
	}
	if err := validateReference(value.DecisionTransactionRef.DecisionTransactionID,
		value.DecisionTransactionRef.DecisionTransactionSHA256,
		decisionTransactionPrefix, "decision_transaction_ref"); err != nil {
		return err
	}
	if err := validateReference(value.OperationalClosureRef.ClosureID,
		value.OperationalClosureRef.ClosureSHA256, operationalClosurePrefix,
		"operational_closure_ref"); err != nil {
		return err
	}
	return nil
}

func validateManifestAtoms(value *StructuralReplayManifest) error {
	if value.PredecisionAtomRefs == nil || len(value.PredecisionAtomRefs) < 1 ||
		len(value.PredecisionAtomRefs) > maxAtoms || value.PostdecisionAtomRefs == nil ||
		len(value.PostdecisionAtomRefs) > maxAtoms ||
		len(value.PredecisionAtomRefs)+len(value.PostdecisionAtomRefs) > maxAtoms {
		return fmt.Errorf("pre/post atom reference cardinality or combined ceiling differs")
	}
	atomKeys := make([]string, 0,
		len(value.PredecisionAtomRefs)+len(value.PostdecisionAtomRefs))
	for _, group := range []struct {
		label string
		refs  []kd.AtomRef
	}{{"predecision_atom_refs", value.PredecisionAtomRefs},
		{"postdecision_atom_refs", value.PostdecisionAtomRefs}} {
		keys := make([]string, len(group.refs))
		for index, reference := range group.refs {
			if err := validateReference(reference.AtomID, reference.AtomSHA256,
				atomPrefix, group.label); err != nil {
				return err
			}
			keys[index] = reference.AtomID
		}
		if err := validateUniqueKeys(keys, group.label, true); err != nil {
			return err
		}
		atomKeys = append(atomKeys, keys...)
	}
	return validateUniqueKeys(atomKeys, "combined atom references", false)
}

func validateManifestArtifacts(value *StructuralReplayManifest) error {
	if value.ArtifactRefs == nil || len(value.ArtifactRefs) > maxArtifacts {
		return fmt.Errorf("artifact_refs cardinality must be 0..%d", maxArtifacts)
	}
	artifactKeys := make([]string, len(value.ArtifactRefs))
	for index := range value.ArtifactRefs {
		if err := validateArtifactRef(&value.ArtifactRefs[index]); err != nil {
			return fmt.Errorf("artifact_refs[%d]: %w", index, err)
		}
		raw, _ := op.CanonicalJSON(&value.ArtifactRefs[index])
		artifactKeys[index] = string(raw)
	}
	return validateUniqueKeys(artifactKeys, "artifact_refs", true)
}

func validateManifestAttemptRefs(value *StructuralReplayManifest) error {
	if value.ArtifactReceiptRefs == nil || len(value.ArtifactReceiptRefs) > maxArtifactReceipts {
		return fmt.Errorf("artifact_receipt_refs cardinality must be 0..%d", maxArtifactReceipts)
	}
	receiptKeys := make([]string, len(value.ArtifactReceiptRefs))
	for index, reference := range value.ArtifactReceiptRefs {
		if err := validateReference(reference.ArtifactReceiptID,
			reference.ArtifactReceiptSHA256, artifactReceiptPrefix,
			"artifact_receipt_ref"); err != nil {
			return err
		}
		receiptKeys[index] = reference.ArtifactReceiptID
	}
	if err := validateUniqueKeys(receiptKeys, "artifact_receipt_refs", true); err != nil {
		return err
	}
	if value.CapabilityInvocationRefs == nil || len(value.CapabilityInvocationRefs) < 1 ||
		len(value.CapabilityInvocationRefs) > maxInvocations {
		return fmt.Errorf("capability_invocation_refs cardinality must be 1..%d", maxInvocations)
	}
	invocationKeys := make([]string, len(value.CapabilityInvocationRefs))
	for index, reference := range value.CapabilityInvocationRefs {
		if err := validateReference(reference.InvocationID, reference.InvocationSHA256,
			invocationPrefix, "capability_invocation_ref"); err != nil {
			return err
		}
		invocationKeys[index] = reference.InvocationID
	}
	if err := validateUniqueKeys(invocationKeys, "capability_invocation_refs", false); err != nil {
		return err
	}
	return nil
}

func validateManifestEventReceiptRefs(value *StructuralReplayManifest) error {
	if value.InteractionEventRefs == nil || len(value.InteractionEventRefs) > maxEvents {
		return fmt.Errorf("interaction_event_refs cardinality must be 0..%d", maxEvents)
	}
	eventKeys := make([]string, len(value.InteractionEventRefs))
	for index, reference := range value.InteractionEventRefs {
		if err := validateReference(reference.EventID, reference.EventSHA256,
			eventPrefix, "interaction_event_ref"); err != nil {
			return err
		}
		eventKeys[index] = reference.EventID
	}
	if err := validateUniqueKeys(eventKeys, "interaction_event_refs", false); err != nil {
		return err
	}
	if value.ExecutionReceiptRefs == nil || len(value.ExecutionReceiptRefs) < 1 ||
		len(value.ExecutionReceiptRefs) > maxExecutionReceipts {
		return fmt.Errorf("execution_receipt_refs cardinality must be 1..%d", maxExecutionReceipts)
	}
	executionKeys := make([]string, len(value.ExecutionReceiptRefs))
	for index, reference := range value.ExecutionReceiptRefs {
		if err := validateReference(reference.ExecutionReceiptID,
			reference.ExecutionReceiptSHA256, executionReceiptPrefix,
			"execution_receipt_ref"); err != nil {
			return err
		}
		executionKeys[index] = reference.ExecutionReceiptID
	}
	return validateUniqueKeys(executionKeys, "execution_receipt_refs", false)
}

func manifestHeaderProjectionMatches(value *StructuralReplayManifest,
	closure *kd.KernelDecisionReferenceClosure) bool {
	if closure == nil {
		return false
	}
	operational, transaction := &closure.OperationalClosure, &closure.DecisionTransaction
	return value.DecisionClosureRef == (ClosureRef{
		ClosureID: closure.ClosureID, ClosureSHA256: closure.ClosureSHA256}) &&
		value.DecisionTransactionRef == (DecisionTransactionRef{
			DecisionTransactionID:     transaction.DecisionTransactionID,
			DecisionTransactionSHA256: transaction.DecisionTransactionSHA256}) &&
		value.OperationalClosureRef == (ClosureRef{
			ClosureID: operational.ClosureID, ClosureSHA256: operational.ClosureSHA256})
}

func manifestAttemptProjectionMatches(value *StructuralReplayManifest,
	operational *op.KernelOperationalReferenceClosure) bool {
	if len(value.ArtifactReceiptRefs) != len(operational.ArtifactReceipts) ||
		len(value.CapabilityInvocationRefs) != len(operational.CapabilityInvocations) {
		return false
	}
	for index, record := range operational.ArtifactReceipts {
		if value.ArtifactReceiptRefs[index] != (op.ArtifactReceiptRef{
			ArtifactReceiptID:     record.ArtifactReceiptID,
			ArtifactReceiptSHA256: record.ArtifactReceiptSHA256}) {
			return false
		}
	}
	for index, record := range operational.CapabilityInvocations {
		if value.CapabilityInvocationRefs[index] != (op.CapabilityInvocationRef{
			InvocationID: record.InvocationID, InvocationSHA256: record.InvocationSHA256}) {
			return false
		}
	}
	return true
}

func manifestEventProjectionMatches(value *StructuralReplayManifest,
	operational *op.KernelOperationalReferenceClosure) bool {
	if len(value.InteractionEventRefs) != len(operational.InteractionEvents) ||
		len(value.ExecutionReceiptRefs) != len(operational.ExecutionReceipts) {
		return false
	}
	for index, record := range operational.InteractionEvents {
		if value.InteractionEventRefs[index] != (op.InteractionEventRef{
			EventID: record.EventID, EventSHA256: record.EventSHA256}) {
			return false
		}
	}
	for index, record := range operational.ExecutionReceipts {
		if value.ExecutionReceiptRefs[index] != (op.ExecutionReceiptRef{
			ExecutionReceiptID:     record.ExecutionReceiptID,
			ExecutionReceiptSHA256: record.ExecutionReceiptSHA256}) {
			return false
		}
	}
	return true
}

func manifestAtomProjectionMatches(actual []kd.AtomRef,
	closure *kd.KernelDecisionReferenceClosure, phase string) bool {
	index := 0
	for _, atom := range closure.CognitiveAtoms {
		if atom.Source.SourcePhase != phase {
			continue
		}
		if index >= len(actual) || actual[index] != (kd.AtomRef{
			AtomID: atom.AtomID, AtomSHA256: atom.AtomSHA256}) {
			return false
		}
		index++
	}
	return index == len(actual)
}

func requireManifestProjection(value *StructuralReplayManifest,
	closure *kd.KernelDecisionReferenceClosure) error {
	if !manifestHeaderProjectionMatches(value, closure) {
		return fmt.Errorf("manifest must be the exact ordered projection of its closure")
	}
	operational := &closure.OperationalClosure
	if !reflect.DeepEqual(value.ArtifactRefs, operational.Artifacts) ||
		!manifestAttemptProjectionMatches(value, operational) ||
		!manifestEventProjectionMatches(value, operational) ||
		!manifestAtomProjectionMatches(value.PredecisionAtomRefs, closure, "predecision") ||
		!manifestAtomProjectionMatches(value.PostdecisionAtomRefs, closure, "postdecision") {
		return fmt.Errorf("manifest must be the exact ordered projection of its closure")
	}
	return nil
}

func newManifest(closure *kd.KernelDecisionReferenceClosure) *StructuralReplayManifest {
	operational := &closure.OperationalClosure
	return &StructuralReplayManifest{
		APIVersion:               manifestAPI,
		ArtifactReceiptRefs:      make([]op.ArtifactReceiptRef, 0, len(operational.ArtifactReceipts)),
		ArtifactRefs:             append([]op.ArtifactRef{}, operational.Artifacts...),
		Canonicalization:         canonicalization,
		CapabilityInvocationRefs: make([]op.CapabilityInvocationRef, 0, len(operational.CapabilityInvocations)),
		DecisionClosureRef: ClosureRef{
			ClosureID: closure.ClosureID, ClosureSHA256: closure.ClosureSHA256,
		},
		DecisionTransactionRef: DecisionTransactionRef{
			DecisionTransactionID:     closure.DecisionTransaction.DecisionTransactionID,
			DecisionTransactionSHA256: closure.DecisionTransaction.DecisionTransactionSHA256,
		},
		ExecutionReceiptRefs:  make([]op.ExecutionReceiptRef, 0, len(operational.ExecutionReceipts)),
		InteractionEventRefs:  make([]op.InteractionEventRef, 0, len(operational.InteractionEvents)),
		Kind:                  manifestKind,
		OperationalClosureRef: ClosureRef{ClosureID: operational.ClosureID, ClosureSHA256: operational.ClosureSHA256},
		PostdecisionAtomRefs:  make([]kd.AtomRef, 0),
		PredecisionAtomRefs:   make([]kd.AtomRef, 0),
		ReplayMode:            manifestMode,
	}
}

func populateManifestRefs(manifest *StructuralReplayManifest,
	closure *kd.KernelDecisionReferenceClosure) {
	operational := &closure.OperationalClosure
	for _, record := range operational.ArtifactReceipts {
		manifest.ArtifactReceiptRefs = append(manifest.ArtifactReceiptRefs, op.ArtifactReceiptRef{
			ArtifactReceiptID:     record.ArtifactReceiptID,
			ArtifactReceiptSHA256: record.ArtifactReceiptSHA256,
		})
	}
	for _, record := range operational.CapabilityInvocations {
		manifest.CapabilityInvocationRefs = append(manifest.CapabilityInvocationRefs,
			op.CapabilityInvocationRef{InvocationID: record.InvocationID,
				InvocationSHA256: record.InvocationSHA256})
	}
	for _, record := range operational.InteractionEvents {
		manifest.InteractionEventRefs = append(manifest.InteractionEventRefs,
			op.InteractionEventRef{EventID: record.EventID, EventSHA256: record.EventSHA256})
	}
	for _, record := range operational.ExecutionReceipts {
		manifest.ExecutionReceiptRefs = append(manifest.ExecutionReceiptRefs,
			op.ExecutionReceiptRef{ExecutionReceiptID: record.ExecutionReceiptID,
				ExecutionReceiptSHA256: record.ExecutionReceiptSHA256})
	}
	for _, atom := range closure.CognitiveAtoms {
		reference := kd.AtomRef{AtomID: atom.AtomID, AtomSHA256: atom.AtomSHA256}
		if atom.Source.SourcePhase == "predecision" {
			manifest.PredecisionAtomRefs = append(manifest.PredecisionAtomRefs, reference)
		} else {
			manifest.PostdecisionAtomRefs = append(manifest.PostdecisionAtomRefs, reference)
		}
	}
}

func deriveManifestPrepared(closure *kd.KernelDecisionReferenceClosure) (
	*StructuralReplayManifest, error,
) {
	manifest := newManifest(closure)
	populateManifestRefs(manifest, closure)
	if err := validateManifestShape(manifest, true); err != nil {
		return nil, err
	}
	digest, err := digestValue(manifest, manifestDomain, maxManifestBytes)
	if err != nil {
		return nil, fmt.Errorf("manifest blank preimage: %w", err)
	}
	manifest.ManifestID, manifest.ManifestSHA256 = manifestPrefix+digest, digest
	if _, err := canonicalBytes(manifest, maxManifestBytes); err != nil {
		return nil, fmt.Errorf("sealed manifest: %w", err)
	}
	return manifest, nil
}

func validateManifestPrepared(value *StructuralReplayManifest,
	closure *kd.KernelDecisionReferenceClosure, allowBlank bool) error {
	if err := validateManifestShape(value, allowBlank); err != nil {
		return err
	}
	blank := *value
	blank.ManifestID, blank.ManifestSHA256 = "", ""
	expected, err := deriveManifestPrepared(closure)
	if err != nil {
		return err
	}
	expected.ManifestID, expected.ManifestSHA256 = "", ""
	if !reflect.DeepEqual(&blank, expected) {
		return fmt.Errorf("manifest must be the exact ordered projection of its closure")
	}
	digest, err := digestValue(&blank, manifestDomain, maxManifestBytes)
	if err != nil {
		return fmt.Errorf("manifest blank preimage: %w", err)
	}
	if !allowBlank && value.ManifestSHA256 != digest {
		return fmt.Errorf("manifest_sha256 does not match canonical preimage")
	}
	if _, err := canonicalBytes(value, maxManifestBytes); err != nil {
		return fmt.Errorf("sealed manifest: %w", err)
	}
	return nil
}

func StructuralReplayManifestDigest(value *StructuralReplayManifest,
	decisionClosure *kd.KernelDecisionReferenceClosure) (string, error) {
	if value == nil {
		return "", fmt.Errorf("StructuralReplayManifest is nil")
	}
	blank := *value
	blank.ManifestID, blank.ManifestSHA256 = "", ""
	if err := validateManifestLocal(&blank, true); err != nil {
		return "", err
	}
	if _, err := measureTypedJSON(value, maxManifestBytes); err != nil {
		return "", err
	}
	if err := validateManifestShape(&blank, true); err != nil {
		return "", err
	}
	if err := requireManifestProjection(&blank, decisionClosure); err != nil {
		return "", err
	}
	closure, err := validateDecisionClosure(decisionClosure)
	if err != nil {
		return "", err
	}
	if err := validateManifestPrepared(&blank, closure, true); err != nil {
		return "", err
	}
	return digestValue(&blank, manifestDomain, maxManifestBytes)
}

func ValidateStructuralReplayManifest(value *StructuralReplayManifest,
	decisionClosure *kd.KernelDecisionReferenceClosure) error {
	if err := validateManifestShape(value, false); err != nil {
		return err
	}
	if err := requireManifestProjection(value, decisionClosure); err != nil {
		return err
	}
	closure, err := validateDecisionClosure(decisionClosure)
	if err != nil {
		return err
	}
	return validateManifestPrepared(value, closure, false)
}

func SealStructuralReplayManifest(value *StructuralReplayManifest,
	decisionClosure *kd.KernelDecisionReferenceClosure) (*StructuralReplayManifest, error) {
	if value == nil || value.ManifestID != "" || value.ManifestSHA256 != "" {
		return nil, fmt.Errorf("sealing manifest requires blank own identity")
	}
	if err := validateManifestShape(value, true); err != nil {
		return nil, err
	}
	if err := requireManifestProjection(value, decisionClosure); err != nil {
		return nil, err
	}
	closure, err := validateDecisionClosure(decisionClosure)
	if err != nil {
		return nil, err
	}
	if err := validateManifestPrepared(value, closure, true); err != nil {
		return nil, err
	}
	return deriveManifestPrepared(closure)
}

func DeriveStructuralReplayManifest(decisionClosure *kd.KernelDecisionReferenceClosure) (
	*StructuralReplayManifest, error,
) {
	closure, err := validateDecisionClosure(decisionClosure)
	if err != nil {
		return nil, err
	}
	return deriveManifestPrepared(closure)
}

func DecodeStructuralReplayManifest(raw []byte,
	decisionClosure *kd.KernelDecisionReferenceClosure) (*StructuralReplayManifest, error) {
	value, err := decodeExact[StructuralReplayManifest](raw, maxManifestBytes)
	if err != nil {
		return nil, err
	}
	return value, ValidateStructuralReplayManifest(value, decisionClosure)
}
