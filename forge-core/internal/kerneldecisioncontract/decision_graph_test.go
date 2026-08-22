package kerneldecisioncontract

import (
	"encoding/json"
	"math"
	"sort"
	"strings"
	"testing"

	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

func resealedAtomCandidate(t *testing.T, source *KernelDecisionReferenceClosure,
	index int, mutate func(*CognitiveAtom)) *KernelDecisionReferenceClosure {
	t.Helper()
	candidate := *source
	candidate.ClosureID, candidate.ClosureSHA256 = "", ""
	candidate.CognitiveAtoms = append([]CognitiveAtom{}, source.CognitiveAtoms...)
	atom := candidate.CognitiveAtoms[index]
	atom.AtomID, atom.AtomSHA256 = "", ""
	mutate(&atom)
	sealed, err := SealCognitiveAtom(&atom)
	if err != nil {
		t.Fatal(err)
	}
	candidate.CognitiveAtoms[index] = *sealed
	sort.Slice(candidate.CognitiveAtoms, func(i, j int) bool {
		return candidate.CognitiveAtoms[i].AtomID < candidate.CognitiveAtoms[j].AtomID
	})
	return &candidate
}

func postAtomIndex(value *KernelDecisionReferenceClosure, kind string) int {
	for index := range value.CognitiveAtoms {
		if value.CognitiveAtoms[index].Source.SourceKind == kind {
			return index
		}
	}
	return -1
}

func TestFullyResealedAtomTaskBindingsAndPostSourceDrift(t *testing.T) {
	value, _ := golden(t)
	index := postAtomIndex(value, "capability_invocation")
	cases := []func(*CognitiveAtom){
		func(atom *CognitiveAtom) { atom.TaskBinding.TaskID = "drifted-task" },
		func(atom *CognitiveAtom) { atom.Bindings.ContextSHA256 = "f" + atom.Bindings.ContextSHA256[1:] },
		func(atom *CognitiveAtom) {
			atom.Validity.ValidFromUnixMS = value.DecisionTransaction.CreatedAtUnixMS - 1
		},
	}
	for caseIndex, mutate := range cases {
		candidate := resealedAtomCandidate(t, value, index, mutate)
		if _, err := SealClosure(candidate); err == nil {
			t.Fatalf("resealed atom drift %d accepted", caseIndex)
		}
	}
}

func TestDeclaredInputReceiptCannotSourcePostdecisionAtom(t *testing.T) {
	value, _ := golden(t)
	input := value.OperationalClosure.ArtifactReceipts[0]
	for _, receipt := range value.OperationalClosure.ArtifactReceipts {
		if receipt.ReceiptRole == "declared_input" {
			input = receipt
		}
	}
	index := postAtomIndex(value, "artifact_receipt")
	candidate := resealedAtomCandidate(t, value, index, func(atom *CognitiveAtom) {
		atom.Source.SourceRef = json.RawMessage(`{"artifact_receipt_id":"` +
			input.ArtifactReceiptID + `","artifact_receipt_sha256":"` + input.ArtifactReceiptSHA256 + `"}`)
		atom.Validity.ValidFromUnixMS = value.DecisionTransaction.CreatedAtUnixMS
	})
	if _, err := SealClosure(candidate); err == nil {
		t.Fatal("declared-input ArtifactReceipt accepted as postdecision source")
	}
}

func TestPostAtomCannotFallBetweenTransactionAndSource(t *testing.T) {
	value, _ := golden(t)
	index := postAtomIndex(value, "capability_invocation")
	created := value.DecisionTransaction.CreatedAtUnixMS
	requested := value.OperationalClosure.CapabilityInvocations[0].RequestedAtUnixMS
	candidate := resealedAtomCandidate(t, value, index, func(atom *CognitiveAtom) {
		atom.Validity.ValidFromUnixMS = created + (requested-created)/2
	})
	if _, err := SealClosure(candidate); err == nil || !strings.Contains(err.Error(), "predates") {
		t.Fatalf("postdecision validity between transaction and source accepted: %v", err)
	}
}

func TestTransactionAndReadTemporalEdges(t *testing.T) {
	value, _ := golden(t)
	cases := []struct {
		name   string
		mutate func(*KernelDecisionReferenceClosure)
	}{
		{"transaction-after-request", func(candidate *KernelDecisionReferenceClosure) {
			first := candidate.OperationalClosure.CapabilityInvocations[0].RequestedAtUnixMS
			for _, invocation := range candidate.OperationalClosure.CapabilityInvocations[1:] {
				if invocation.RequestedAtUnixMS < first {
					first = invocation.RequestedAtUnixMS
				}
			}
			candidate.DecisionTransaction.CreatedAtUnixMS = first + 1
		}},
		{"future-read", func(candidate *KernelDecisionReferenceClosure) {
			readID := candidate.DecisionTransaction.ReadArtifactReceiptRefs[0].ArtifactReceiptID
			for index := range candidate.OperationalClosure.ArtifactReceipts {
				if candidate.OperationalClosure.ArtifactReceipts[index].ArtifactReceiptID == readID {
					candidate.OperationalClosure.ArtifactReceipts[index].CreatedAtUnixMS =
						candidate.DecisionTransaction.CreatedAtUnixMS + 1
				}
			}
		}},
	}
	for _, item := range cases {
		candidate, err := cloneValue(value)
		if err != nil {
			t.Fatal(err)
		}
		item.mutate(candidate)
		if err := validateTimes(candidate.CognitiveAtoms, &candidate.DecisionTransaction,
			&candidate.OperationalClosure); err == nil {
			t.Fatalf("temporal case %s accepted", item.name)
		}
	}
}

func TestPredecisionAtomTemporalEdges(t *testing.T) {
	value, _ := golden(t)
	for _, future := range []bool{true, false} {
		candidate, err := cloneValue(value)
		if err != nil {
			t.Fatal(err)
		}
		for index := range candidate.CognitiveAtoms {
			if candidate.CognitiveAtoms[index].Source.SourcePhase != "predecision" {
				continue
			}
			created := candidate.DecisionTransaction.CreatedAtUnixMS
			if future {
				candidate.CognitiveAtoms[index].Validity.ValidFromUnixMS = created + 1
			} else {
				candidate.CognitiveAtoms[index].Validity.ValidUntilUnixMS = &created
			}
			break
		}
		if err := validateTimes(candidate.CognitiveAtoms, &candidate.DecisionTransaction,
			&candidate.OperationalClosure); err == nil {
			t.Fatalf("predecision temporal case future=%t accepted", future)
		}
	}
}

func resealedTransactionCandidate(t *testing.T, source *KernelDecisionReferenceClosure,
	mutate func(*DecisionTransaction)) *KernelDecisionReferenceClosure {
	t.Helper()
	candidate := *source
	candidate.ClosureID, candidate.ClosureSHA256 = "", ""
	transaction := source.DecisionTransaction
	transaction.DecisionTransactionID, transaction.DecisionTransactionSHA256 = "", ""
	mutate(&transaction)
	sealed, err := SealDecisionTransaction(&transaction)
	if err != nil {
		t.Fatal(err)
	}
	candidate.DecisionTransaction = *sealed
	return &candidate
}

func resealedRoleTransaction(t *testing.T, source *KernelDecisionReferenceClosure,
	mutate func(*DecisionTransaction)) *DecisionTransaction {
	t.Helper()
	transaction, err := cloneValue(&source.DecisionTransaction)
	if err != nil {
		t.Fatal(err)
	}
	transaction.DecisionTransactionID, transaction.DecisionTransactionSHA256 = "", ""
	mutate(transaction)
	sealed, err := SealDecisionTransaction(transaction)
	if err != nil {
		t.Fatal(err)
	}
	return sealed
}

func TestGoalAtomRefRejectsNonGoalAtom(t *testing.T) {
	value, _ := golden(t)
	wrongGoal := resealedRoleTransaction(t, value, func(transaction *DecisionTransaction) {
		transaction.GoalAtomRef, transaction.TriggerAtomRefs[0] =
			transaction.TriggerAtomRefs[0], transaction.GoalAtomRef
	})
	if err := validateRoleClosure(value.CognitiveAtoms, wrongGoal); err == nil ||
		!strings.Contains(err.Error(), "only predecision goal") {
		t.Fatalf("non-goal goal_atom_ref accepted: %v", err)
	}
}

func TestTriggerAndGuardRejectAdditionalGoalAtom(t *testing.T) {
	value, _ := golden(t)
	atoms := append([]CognitiveAtom(nil), value.CognitiveAtoms...)
	index := -1
	for candidate := range atoms {
		if atoms[candidate].Source.SourcePhase == "predecision" && atoms[candidate].AtomType == "preference" {
			index = candidate
			break
		}
	}
	if index < 0 {
		t.Fatal("golden has no predecision preference")
	}
	oldID := atoms[index].AtomID
	changed := atoms[index]
	changed.AtomID, changed.AtomSHA256 = "", ""
	changed.AtomType = "goal"
	sealedAtom, err := SealCognitiveAtom(&changed)
	if err != nil {
		t.Fatal(err)
	}
	atoms[index] = *sealedAtom
	sort.Slice(atoms, func(i, j int) bool { return atoms[i].AtomID < atoms[j].AtomID })
	replacement := AtomRef{AtomID: sealedAtom.AtomID, AtomSHA256: sealedAtom.AtomSHA256}
	extraGoal := resealedRoleTransaction(t, value, func(transaction *DecisionTransaction) {
		for index := range transaction.TriggerAtomRefs {
			if transaction.TriggerAtomRefs[index].AtomID == oldID {
				transaction.TriggerAtomRefs[index] = replacement
			}
		}
		for index := range transaction.GuardAtomRefs {
			if transaction.GuardAtomRefs[index].AtomID == oldID {
				transaction.GuardAtomRefs[index] = replacement
			}
		}
		sort.Slice(transaction.TriggerAtomRefs, func(i, j int) bool {
			return transaction.TriggerAtomRefs[i].AtomID < transaction.TriggerAtomRefs[j].AtomID
		})
		sort.Slice(transaction.GuardAtomRefs, func(i, j int) bool {
			return transaction.GuardAtomRefs[i].AtomID < transaction.GuardAtomRefs[j].AtomID
		})
	})
	if err := validateRoleClosure(atoms, extraGoal); err == nil ||
		!strings.Contains(err.Error(), "only predecision goal") {
		t.Fatalf("additional role goal accepted: %v", err)
	}
}

func TestNonGoalTriggerAndGuardRolesRemainUntyped(t *testing.T) {
	value, _ := golden(t)
	transaction := resealedRoleTransaction(t, value, func(transaction *DecisionTransaction) {
		transaction.TriggerAtomRefs[0], transaction.GuardAtomRefs[0] =
			transaction.GuardAtomRefs[0], transaction.TriggerAtomRefs[0]
		sort.Slice(transaction.GuardAtomRefs, func(i, j int) bool {
			return transaction.GuardAtomRefs[i].AtomID < transaction.GuardAtomRefs[j].AtomID
		})
	})
	if err := validateRoleClosure(value.CognitiveAtoms, transaction); err != nil {
		t.Fatalf("non-goal trigger/guard swap rejected: %v", err)
	}
}

func TestSelectedOperationProjectionDriftRejected(t *testing.T) {
	value, _ := golden(t)
	cases := []func(*DecisionTransaction){
		func(transaction *DecisionTransaction) {
			selectedOption(transaction).Capability.CapabilityID = "drifted.capability"
		},
		func(transaction *DecisionTransaction) {
			selectedOption(transaction).RequestedActionSHA256 = "0" +
				selectedOption(transaction).RequestedActionSHA256[1:]
		},
		func(transaction *DecisionTransaction) { transaction.IdempotencyKey = "drifted-idempotency" },
		func(transaction *DecisionTransaction) {
			transaction.ReadArtifactReceiptRefs = transaction.ReadArtifactReceiptRefs[:0]
		},
		func(transaction *DecisionTransaction) { transaction.WriteSlots = transaction.WriteSlots[:0] },
	}
	for index, mutate := range cases {
		candidate := resealedTransactionCandidate(t, value, mutate)
		if _, err := SealClosure(candidate); err == nil {
			t.Fatalf("selected projection drift %d accepted", index)
		}
	}
}

func TestEveryOperationalRecordClassBindingDriftRejected(t *testing.T) {
	value, _ := golden(t)
	cases := []func(*KernelDecisionReferenceClosure){
		func(candidate *KernelDecisionReferenceClosure) {
			candidate.OperationalClosure.ArtifactReceipts[0].Bindings.ContextSHA256 = "f" +
				candidate.OperationalClosure.ArtifactReceipts[0].Bindings.ContextSHA256[1:]
		},
		func(candidate *KernelDecisionReferenceClosure) {
			candidate.OperationalClosure.CapabilityInvocations[0].Bindings.ContextSHA256 = "f" +
				candidate.OperationalClosure.CapabilityInvocations[0].Bindings.ContextSHA256[1:]
		},
		func(candidate *KernelDecisionReferenceClosure) {
			candidate.OperationalClosure.InteractionEvents[0].Bindings.ContextSHA256 = "f" +
				candidate.OperationalClosure.InteractionEvents[0].Bindings.ContextSHA256[1:]
		},
		func(candidate *KernelDecisionReferenceClosure) {
			candidate.OperationalClosure.ExecutionReceipts[0].Bindings.ContextSHA256 = "f" +
				candidate.OperationalClosure.ExecutionReceipts[0].Bindings.ContextSHA256[1:]
		},
	}
	for index, mutate := range cases {
		candidate := *value
		candidate.ClosureID, candidate.ClosureSHA256 = "", ""
		mutate(&candidate)
		if _, err := SealClosure(&candidate); err == nil {
			t.Fatalf("operational binding drift %d accepted", index)
		}
	}
}

func TestMissingAndOrphanPredecisionAtomsRejected(t *testing.T) {
	value, _ := golden(t)
	candidate := resealedTransactionCandidate(t, value, func(transaction *DecisionTransaction) {
		transaction.GuardAtomRefs = transaction.GuardAtomRefs[1:]
	})
	if _, err := SealClosure(candidate); err == nil {
		t.Fatal("orphan predecision atom accepted")
	}
	candidate = resealedTransactionCandidate(t, value, func(transaction *DecisionTransaction) {
		zeros := strings.Repeat("0", 64)
		transaction.GoalAtomRef = AtomRef{AtomID: atomPrefix + zeros, AtomSHA256: zeros}
	})
	if _, err := SealClosure(candidate); err == nil {
		t.Fatal("unresolved role atom accepted")
	}
}

func TestCheckedBudgetAggregationBoundaries(t *testing.T) {
	if _, err := checkedAdd(math.MaxInt64, 1); err == nil {
		t.Fatal("signed int64 aggregate overflow accepted")
	}
	value, _ := golden(t)
	transaction := value.DecisionTransaction
	transaction.Budget.MaxCalls = 2
	transaction.Budget.MaxCostUSDMicros = 20
	transaction.Budget.TimeoutMS = 700
	transaction.Budget.MaxInputTokens = 14
	transaction.Budget.MaxNetworkBytes = 18
	transaction.Budget.MaxOutputBytes = 34
	transaction.Budget.MaxOutputTokens = 6
	if err := validateBudgetClosure(&transaction, &value.OperationalClosure); err != nil {
		t.Fatalf("exact aggregate rejected: %v", err)
	}
	cases := []func(*op.ObservedUsage){
		func(value *op.ObservedUsage) { value.CallCount++ },
		func(value *op.ObservedUsage) { value.CostUSDMicros++ },
		func(value *op.ObservedUsage) { value.ElapsedMS++ },
		func(value *op.ObservedUsage) { value.InputTokens++ },
		func(value *op.ObservedUsage) { value.NetworkBytes++ },
		func(value *op.ObservedUsage) { value.OutputBytes++ },
		func(value *op.ObservedUsage) { value.OutputTokens++ },
	}
	for index, mutate := range cases {
		operational, err := cloneValue(&value.OperationalClosure)
		if err != nil {
			t.Fatal(err)
		}
		mutate(&operational.ExecutionReceipts[0].ObservedUsage)
		if err := validateBudgetClosure(&transaction, operational); err == nil ||
			!strings.Contains(err.Error(), "aggregate usage exceeds") {
			t.Fatalf("aggregate N+1 dimension %d did not hit usage bound: %v", index, err)
		}
	}
	operational, err := cloneValue(&value.OperationalClosure)
	if err != nil {
		t.Fatal(err)
	}
	operational.CapabilityInvocations = append(operational.CapabilityInvocations,
		operational.CapabilityInvocations[len(operational.CapabilityInvocations)-1])
	if err := validateBudgetClosure(&transaction, operational); err == nil ||
		!strings.Contains(err.Error(), "Invocation count") {
		t.Fatalf("invocation-count N+1 did not hit independent bound: %v", err)
	}
}

func TestSelectedProjectionAndCorrelationEdgesIndependent(t *testing.T) {
	value, _ := golden(t)
	cases := []func(*op.KernelOperationalReferenceClosure){
		func(value *op.KernelOperationalReferenceClosure) {
			value.CapabilityInvocations[0].Capability.CapabilityID = "drifted.capability"
		},
		func(value *op.KernelOperationalReferenceClosure) {
			value.CapabilityInvocations[0].RequestedActionSHA256 = strings.Repeat("0", 64)
		},
		func(value *op.KernelOperationalReferenceClosure) {
			value.CapabilityInvocations[0].IdempotencyKey = "drifted"
		},
		func(value *op.KernelOperationalReferenceClosure) {
			value.CapabilityInvocations[0].InputArtifactReceiptRefs = nil
		},
		func(value *op.KernelOperationalReferenceClosure) {
			value.CapabilityInvocations[0].DeclaredOutputSlots = nil
		},
		func(value *op.KernelOperationalReferenceClosure) {
			value.CapabilityInvocations[0].CorrelationID = "drifted"
		},
		func(value *op.KernelOperationalReferenceClosure) {
			value.InteractionEvents[0].CorrelationID = "drifted"
		},
		func(value *op.KernelOperationalReferenceClosure) {
			value.ExecutionReceipts[0].CorrelationID = "drifted"
		},
	}
	for index, mutate := range cases {
		operational, err := cloneValue(&value.OperationalClosure)
		if err != nil {
			t.Fatal(err)
		}
		mutate(operational)
		if err := validateSelectedOperation(&value.DecisionTransaction, operational); err == nil {
			t.Fatalf("selected/correlation edge %d accepted", index)
		}
	}
}

func TestEveryUsageDimensionRejectsCheckedOverflow(t *testing.T) {
	value, _ := golden(t)
	cases := []func(*op.ObservedUsage, int64){
		func(value *op.ObservedUsage, amount int64) { value.CallCount = amount },
		func(value *op.ObservedUsage, amount int64) { value.CostUSDMicros = amount },
		func(value *op.ObservedUsage, amount int64) { value.ElapsedMS = amount },
		func(value *op.ObservedUsage, amount int64) { value.InputTokens = amount },
		func(value *op.ObservedUsage, amount int64) { value.NetworkBytes = amount },
		func(value *op.ObservedUsage, amount int64) { value.OutputBytes = amount },
		func(value *op.ObservedUsage, amount int64) { value.OutputTokens = amount },
	}
	zero := op.ObservedUsage{}
	for index, set := range cases {
		operational, err := cloneValue(&value.OperationalClosure)
		if err != nil {
			t.Fatal(err)
		}
		operational.ExecutionReceipts[0].ObservedUsage = zero
		operational.ExecutionReceipts[1].ObservedUsage = zero
		set(&operational.ExecutionReceipts[0].ObservedUsage, math.MaxInt64)
		set(&operational.ExecutionReceipts[1].ObservedUsage, 1)
		err = validateBudgetClosure(&value.DecisionTransaction, operational)
		if err == nil || !strings.Contains(err.Error(), "overflows signed int64") {
			t.Fatalf("usage dimension %d did not hit checked-add overflow: %v", index, err)
		}
	}
}
