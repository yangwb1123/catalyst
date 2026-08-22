package kerneldecisioncontract

import (
	"encoding/json"
	"fmt"
	"math"
	"reflect"

	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

func atomIndex(values []CognitiveAtom) map[string]*CognitiveAtom {
	result := make(map[string]*CognitiveAtom, len(values))
	for index := range values {
		result[values[index].AtomID] = &values[index]
	}
	return result
}

func transactionRefs(value *DecisionTransaction) []AtomRef {
	result := []AtomRef{value.GoalAtomRef}
	result = append(result, value.TriggerAtomRefs...)
	return append(result, value.GuardAtomRefs...)
}

func validateRoleClosure(atoms []CognitiveAtom, transaction *DecisionTransaction) error {
	index := atomIndex(atoms)
	referenced := make(map[string]bool)
	for roleIndex, reference := range transactionRefs(transaction) {
		atom := index[reference.AtomID]
		if atom == nil || atom.AtomSHA256 != reference.AtomSHA256 {
			return fmt.Errorf("transaction atom reference does not resolve exact CognitiveAtom")
		}
		if !oneOf(atom.Source.SourceKind, "artifact", "cognitive_atom_v1", "evidence_record", "work_intent") {
			return fmt.Errorf("DecisionTransaction may reference only predecision atoms")
		}
		if (roleIndex == 0) != (atom.AtomType == "goal") {
			return fmt.Errorf("goal_atom_ref must resolve the only predecision goal CognitiveAtom")
		}
		referenced[reference.AtomID] = true
	}
	for index := range atoms {
		pre := atoms[index].Source.SourcePhase == "predecision"
		if pre != referenced[atoms[index].AtomID] {
			return fmt.Errorf("predecision atoms must equal exact transaction role union")
		}
	}
	return nil
}

func rawReference(raw json.RawMessage) (string, string, error) {
	var value map[string]any
	if err := json.Unmarshal(raw, &value); err != nil {
		return "", "", err
	}
	var id, digest string
	for field, member := range value {
		textValue, ok := member.(string)
		if !ok {
			return "", "", fmt.Errorf("operational source reference is not textual")
		}
		if len(field) >= 3 && field[len(field)-3:] == "_id" {
			id = textValue
		}
		if len(field) >= 7 && field[len(field)-7:] == "_sha256" {
			digest = textValue
		}
	}
	if id == "" || digest == "" {
		return "", "", fmt.Errorf("operational source reference fields are missing")
	}
	return id, digest, nil
}

func validatePostSources(atoms []CognitiveAtom, transaction *DecisionTransaction,
	operational *op.KernelOperationalReferenceClosure) error {
	for index := range atoms {
		atom := &atoms[index]
		if atom.Source.SourcePhase != "postdecision" {
			continue
		}
		id, digest, err := rawReference(atom.Source.SourceRef)
		if err != nil {
			return err
		}
		time, output, found := operationalSource(operational, atom.Source.SourceKind, id, digest)
		if !found {
			return fmt.Errorf("postdecision source_ref must resolve exact operational record")
		}
		if atom.Source.SourceKind == "artifact_receipt" && !output {
			return fmt.Errorf("postdecision ArtifactReceipt source must be declared_output")
		}
		if atom.Validity.ValidFromUnixMS < transaction.CreatedAtUnixMS ||
			atom.Validity.ValidFromUnixMS < time {
			return fmt.Errorf("postdecision Atom validity predates transaction or source")
		}
	}
	return nil
}

func operationalSource(value *op.KernelOperationalReferenceClosure, kind, id, digest string) (int64, bool, bool) {
	switch kind {
	case "artifact_receipt":
		for _, record := range value.ArtifactReceipts {
			if record.ArtifactReceiptID == id && record.ArtifactReceiptSHA256 == digest {
				return record.CreatedAtUnixMS, record.ReceiptRole == "declared_output", true
			}
		}
	case "capability_invocation":
		for _, record := range value.CapabilityInvocations {
			if record.InvocationID == id && record.InvocationSHA256 == digest {
				return record.RequestedAtUnixMS, false, true
			}
		}
	case "interaction_event":
		for _, record := range value.InteractionEvents {
			if record.EventID == id && record.EventSHA256 == digest {
				return record.OccurredAtUnixMS, false, true
			}
		}
	case "execution_receipt":
		for _, record := range value.ExecutionReceipts {
			if record.ExecutionReceiptID == id && record.ExecutionReceiptSHA256 == digest {
				return record.EndedAtUnixMS, false, true
			}
		}
	}
	return 0, false, false
}

func validateContext(atoms []CognitiveAtom, transaction *DecisionTransaction,
	operational *op.KernelOperationalReferenceClosure) error {
	for index := range atoms {
		if !reflect.DeepEqual(atoms[index].TaskBinding, transaction.TaskBinding) ||
			!reflect.DeepEqual(atoms[index].Bindings, transaction.Bindings) {
			return fmt.Errorf("every CognitiveAtom must share transaction task and bindings")
		}
	}
	for _, record := range operational.ArtifactReceipts {
		if !reflect.DeepEqual(record.TaskBinding, transaction.TaskBinding) || record.Bindings != transaction.Bindings {
			return fmt.Errorf("ArtifactReceipt task or bindings drift")
		}
	}
	for _, record := range operational.CapabilityInvocations {
		if !reflect.DeepEqual(record.TaskBinding, transaction.TaskBinding) || record.Bindings != transaction.Bindings {
			return fmt.Errorf("CapabilityInvocation task or bindings drift")
		}
	}
	return validateRemainingContext(transaction, operational)
}

func validateRemainingContext(transaction *DecisionTransaction,
	operational *op.KernelOperationalReferenceClosure) error {
	for _, record := range operational.InteractionEvents {
		if !reflect.DeepEqual(record.TaskBinding, transaction.TaskBinding) || record.Bindings != transaction.Bindings {
			return fmt.Errorf("InteractionEvent task or bindings drift")
		}
	}
	for _, record := range operational.ExecutionReceipts {
		if !reflect.DeepEqual(record.TaskBinding, transaction.TaskBinding) || record.Bindings != transaction.Bindings {
			return fmt.Errorf("ExecutionReceipt task or bindings drift")
		}
	}
	return nil
}

func selectedOption(value *DecisionTransaction) *DecisionOption {
	for index := range value.Options {
		if value.Options[index].OptionID == value.SelectedOptionID {
			return &value.Options[index]
		}
	}
	return nil
}

func validateSelectedOperation(transaction *DecisionTransaction,
	operational *op.KernelOperationalReferenceClosure) error {
	selected := selectedOption(transaction)
	for _, invocation := range operational.CapabilityInvocations {
		if invocation.CorrelationID != transaction.DecisionTransactionID ||
			invocation.Subject != transaction.Actor || invocation.Capability != selected.Capability ||
			invocation.RequestedActionSHA256 != selected.RequestedActionSHA256 ||
			invocation.IdempotencyKey != transaction.IdempotencyKey ||
			!reflect.DeepEqual(invocation.InputArtifactReceiptRefs, transaction.ReadArtifactReceiptRefs) ||
			!reflect.DeepEqual(invocation.DeclaredOutputSlots, transaction.WriteSlots) {
			return fmt.Errorf("Invocation differs from selected transaction declarations")
		}
	}
	for _, event := range operational.InteractionEvents {
		if event.CorrelationID != transaction.DecisionTransactionID {
			return fmt.Errorf("InteractionEvent correlation differs from transaction")
		}
	}
	for _, receipt := range operational.ExecutionReceipts {
		if receipt.CorrelationID != transaction.DecisionTransactionID {
			return fmt.Errorf("ExecutionReceipt correlation differs from transaction")
		}
	}
	return nil
}

func validateTimes(atoms []CognitiveAtom, transaction *DecisionTransaction,
	operational *op.KernelOperationalReferenceClosure) error {
	first := operational.CapabilityInvocations[0].RequestedAtUnixMS
	for _, invocation := range operational.CapabilityInvocations[1:] {
		if invocation.RequestedAtUnixMS < first {
			first = invocation.RequestedAtUnixMS
		}
	}
	if transaction.CreatedAtUnixMS > first {
		return fmt.Errorf("transaction creation follows first request")
	}
	if err := validateReadTimes(transaction, operational); err != nil {
		return err
	}
	for index := range atoms {
		if atoms[index].Source.SourcePhase != "predecision" {
			continue
		}
		validity := atoms[index].Validity
		if validity.ValidFromUnixMS > transaction.CreatedAtUnixMS ||
			validity.ValidUntilUnixMS != nil && transaction.CreatedAtUnixMS >= *validity.ValidUntilUnixMS {
			return fmt.Errorf("predecision Atom is future or expired at transaction creation")
		}
	}
	return nil
}

func validateReadTimes(transaction *DecisionTransaction,
	operational *op.KernelOperationalReferenceClosure) error {
	for _, reference := range transaction.ReadArtifactReceiptRefs {
		found := false
		for _, receipt := range operational.ArtifactReceipts {
			if receipt.ArtifactReceiptID == reference.ArtifactReceiptID &&
				receipt.ArtifactReceiptSHA256 == reference.ArtifactReceiptSHA256 &&
				receipt.ReceiptRole == "declared_input" &&
				receipt.CreatedAtUnixMS <= transaction.CreatedAtUnixMS {
				found = true
			}
		}
		if !found {
			return fmt.Errorf("transaction read must resolve nonfuture declared-input receipt")
		}
	}
	return nil
}

func checkedAdd(total, increment int64) (int64, error) {
	if increment < 0 || total > math.MaxInt64-increment {
		return 0, fmt.Errorf("caller-declared aggregate usage overflows signed int64")
	}
	return total + increment, nil
}

func validateBudgetClosure(transaction *DecisionTransaction,
	operational *op.KernelOperationalReferenceClosure) error {
	if int64(len(operational.CapabilityInvocations)) > transaction.Budget.MaxCalls {
		return fmt.Errorf("Invocation count exceeds transaction budget")
	}
	totals := [7]int64{}
	for _, receipt := range operational.ExecutionReceipts {
		usage := receipt.ObservedUsage
		values := [7]int64{usage.CallCount, usage.CostUSDMicros, usage.ElapsedMS,
			usage.InputTokens, usage.NetworkBytes, usage.OutputBytes, usage.OutputTokens}
		for index, increment := range values {
			var err error
			totals[index], err = checkedAdd(totals[index], increment)
			if err != nil {
				return err
			}
		}
	}
	limits := [7]int64{transaction.Budget.MaxCalls, transaction.Budget.MaxCostUSDMicros,
		transaction.Budget.TimeoutMS, transaction.Budget.MaxInputTokens,
		transaction.Budget.MaxNetworkBytes, transaction.Budget.MaxOutputBytes,
		transaction.Budget.MaxOutputTokens}
	for index := range totals {
		if totals[index] > limits[index] {
			return fmt.Errorf("caller-declared aggregate usage exceeds transaction budget")
		}
	}
	return nil
}

func validateReferenceGraph(atoms []CognitiveAtom, transaction *DecisionTransaction,
	operational *op.KernelOperationalReferenceClosure) error {
	if err := validateRoleClosure(atoms, transaction); err != nil {
		return err
	}
	if err := validatePostSources(atoms, transaction, operational); err != nil {
		return err
	}
	if err := validateContext(atoms, transaction, operational); err != nil {
		return err
	}
	if err := validateSelectedOperation(transaction, operational); err != nil {
		return err
	}
	if err := validateTimes(atoms, transaction, operational); err != nil {
		return err
	}
	return validateBudgetClosure(transaction, operational)
}
