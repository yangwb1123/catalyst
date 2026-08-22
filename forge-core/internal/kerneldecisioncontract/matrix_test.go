package kerneldecisioncontract

import (
	"bytes"
	"encoding/json"
	"fmt"
	"reflect"
	"sort"
	"strings"
	"testing"

	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

func TestGoldenVocabularyCoverage(t *testing.T) {
	value, _ := golden(t)
	types, sources, hardness, authorities := map[string]bool{}, map[string]bool{}, map[string]bool{}, map[string]bool{}
	for _, atom := range value.CognitiveAtoms {
		types[atom.AtomType] = true
		sources[atom.Source.SourceKind] = true
		hardness[atom.DeclaredHardness] = true
		authorities[atom.DeclaredAuthority.AuthorityKind] = true
	}
	if len(types) != 16 || len(sources) != 8 || len(hardness) != 6 || len(authorities) != 4 {
		t.Fatalf("coverage types=%d sources=%d hardness=%d authority=%d",
			len(types), len(sources), len(hardness), len(authorities))
	}
	if len(attestationsMap(t, value.CognitiveAtoms[0])) != 22 {
		t.Fatal("decision attestations are not exact twenty-two")
	}
}

func atomMatching(t *testing.T, value *KernelDecisionReferenceClosure,
	match func(CognitiveAtom) bool) CognitiveAtom {
	t.Helper()
	for _, atom := range value.CognitiveAtoms {
		if match(atom) {
			return atom
		}
	}
	t.Fatal("golden lacks requested CognitiveAtom")
	return CognitiveAtom{}
}

func TestRestrictedSourceTypeMatrixRejectsCrossFeed(t *testing.T) {
	value, _ := golden(t)
	cases := map[string]string{
		"artifact_receipt": "goal", "capability_invocation": "goal",
		"cognitive_atom_v1": "goal", "evidence_record": "goal",
		"execution_receipt": "goal", "interaction_event": "goal", "work_intent": "actor",
	}
	for sourceKind, atomType := range cases {
		atom := atomMatching(t, value, func(candidate CognitiveAtom) bool {
			return candidate.Source.SourceKind == sourceKind
		})
		atom.AtomID, atom.AtomSHA256, atom.AtomType = "", "", atomType
		if _, err := SealCognitiveAtom(&atom); err == nil ||
			!strings.Contains(err.Error(), "source_kind does not admit atom_type") {
			t.Fatalf("source/type cross-feed %s/%s accepted: %v", sourceKind, atomType, err)
		}
	}
}

func TestHardnessAuthorityNegativeMatrix(t *testing.T) {
	value, _ := golden(t)
	noneAuthority := DeclaredAuthority{AuthorityKind: "none", AuthorityRef: json.RawMessage("null")}
	approval := atomMatching(t, value, func(atom CognitiveAtom) bool {
		return atom.DeclaredAuthority.AuthorityKind == "approval_record"
	}).DeclaredAuthority
	legacy := atomMatching(t, value, func(atom CognitiveAtom) bool {
		return atom.Source.SourceKind == "cognitive_atom_v1"
	})
	legacy.DeclaredHardness = "advisory"
	observation := atomMatching(t, value, func(atom CognitiveAtom) bool {
		return atom.AtomType == "observation"
	})
	inadmitted := observation
	inadmitted.DeclaredHardness = "advisory"
	observation.DeclaredAuthority = approval
	constraint := atomMatching(t, value, func(atom CognitiveAtom) bool {
		return atom.DeclaredHardness == "contract"
	})
	constraint.DeclaredAuthority = noneAuthority
	goal := atomMatching(t, value, func(atom CognitiveAtom) bool { return atom.AtomType == "goal" })
	goal.DeclaredAuthority = noneAuthority
	decision := atomMatching(t, value, func(atom CognitiveAtom) bool {
		return atom.Source.SourceKind == "artifact" && atom.DeclaredHardness == "invariant"
	})
	decision.AtomType, decision.DeclaredHardness = "decision", "required"
	cases := []CognitiveAtom{legacy, inadmitted, observation, constraint, goal, decision}
	for index := range cases {
		cases[index].AtomID, cases[index].AtomSHA256 = "", ""
		if _, err := SealCognitiveAtom(&cases[index]); err == nil {
			t.Fatalf("hardness/authority negative case %d accepted", index)
		}
	}
}

func TestEveryIneffectiveFieldPromotionRejected(t *testing.T) {
	value, _ := golden(t)
	atom := value.CognitiveAtoms[0]
	atom.AtomID, atom.AtomSHA256, atom.EffectiveHardness = "", "", "advisory"
	if _, err := SealCognitiveAtom(&atom); err == nil {
		t.Fatal("effective_hardness promotion accepted")
	}
	atom = value.CognitiveAtoms[0]
	atom.AtomID, atom.AtomSHA256, atom.InstructionAllowed = "", "", true
	if _, err := SealCognitiveAtom(&atom); err == nil {
		t.Fatal("instruction_allowed promotion accepted")
	}

	attestationType := reflect.TypeOf(DecisionAttestations{})
	for index := 0; index < attestationType.NumField(); index++ {
		setTrue := func(value *DecisionAttestations) {
			reflect.ValueOf(value).Elem().Field(index).SetBool(true)
		}
		atom := value.CognitiveAtoms[0]
		atom.AtomID, atom.AtomSHA256 = "", ""
		setTrue(&atom.Attestations)
		transaction := value.DecisionTransaction
		transaction.DecisionTransactionID, transaction.DecisionTransactionSHA256 = "", ""
		setTrue(&transaction.Attestations)
		closure := *value
		closure.ClosureID, closure.ClosureSHA256 = "", ""
		setTrue(&closure.Attestations)
		if _, err := SealCognitiveAtom(&atom); err == nil {
			t.Fatalf("atom attestation %s accepted", attestationType.Field(index).Name)
		}
		if _, err := SealDecisionTransaction(&transaction); err == nil {
			t.Fatalf("transaction attestation %s accepted", attestationType.Field(index).Name)
		}
		if _, err := SealClosure(&closure); err == nil {
			t.Fatalf("closure attestation %s accepted", attestationType.Field(index).Name)
		}
	}
}

func attestationsMap(t *testing.T, value CognitiveAtom) map[string]bool {
	t.Helper()
	raw, err := CanonicalJSON(value.Attestations)
	if err != nil {
		t.Fatal(err)
	}
	result := map[string]bool{}
	if err := json.Unmarshal(raw, &result); err != nil {
		t.Fatal(err)
	}
	return result
}

func TestLegacyAndEvidenceCrossContractPositiveVectors(t *testing.T) {
	value, _ := golden(t)
	for index := range value.CognitiveAtoms {
		atom := value.CognitiveAtoms[index]
		if atom.Source.SourceKind == "cognitive_atom_v1" {
			atom.AtomID, atom.AtomSHA256 = "", ""
			atom.Source.SourceRef = json.RawMessage(`{"atom_id":"atom-99045a525632c18aec6b1c783ba1925e4603b4378b389e5ce86621ab25b145ae","canonical_sha256":"3905ee9fd8293924644dd5d9a1da522ffe944dc58db51a26ee6c584e1335ce20"}`)
			if _, err := SealCognitiveAtom(&atom); err != nil {
				t.Fatalf("real ADR-0047 non-equal ID/digest rejected: %v", err)
			}
		}
		if atom.Source.SourceKind == "evidence_record" {
			atom.AtomID, atom.AtomSHA256 = "", ""
			atom.Source.SourceRef = json.RawMessage(`{"canonical_sha256":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee","record_id":"fixture evidence record / v1"}`)
			if _, err := SealCognitiveAtom(&atom); err != nil {
				t.Fatalf("ADR-0060 short-text record ID rejected: %v", err)
			}
		}
	}
}

func TestClonePreservesAllowedHTMLLikeRawReferenceBytes(t *testing.T) {
	value, _ := golden(t)
	source := artifactAtom(value)
	source.AtomID, source.AtomSHA256 = "", ""
	source.Source.SourceRef = json.RawMessage(`{"artifact_kind":"source-document","artifact_ref":"fixture/<>&","artifact_sha256":"` +
		strings.Repeat("a", 64) + `"}`)
	if _, err := SealCognitiveAtom(&source); err != nil {
		t.Fatalf("allowed source reference scalars changed during clone: %v", err)
	}
	authority := atomMatching(t, value, func(atom CognitiveAtom) bool {
		return atom.DeclaredAuthority.AuthorityKind == "contract_artifact"
	})
	authority.AtomID, authority.AtomSHA256 = "", ""
	authority.DeclaredAuthority.AuthorityRef = json.RawMessage(`{"artifact_kind":"governance-contract","artifact_ref":"fixture/<>&","artifact_sha256":"` +
		strings.Repeat("b", 64) + `"}`)
	if _, err := SealCognitiveAtom(&authority); err != nil {
		t.Fatalf("allowed authority reference scalars changed during clone: %v", err)
	}
}

func TestPropositionScopeRoleOrderingAndUniqueness(t *testing.T) {
	value, _ := golden(t)
	nullable := artifactAtom(value)
	nullable.AtomID, nullable.AtomSHA256 = "", ""
	nullable.Scope.Module, nullable.Scope.Object = nil, nil
	if _, err := SealCognitiveAtom(&nullable); err != nil {
		t.Fatalf("nullable scope rejected: %v", err)
	}
	atom := value.CognitiveAtoms[0]
	atom.AtomID, atom.AtomSHA256 = "", ""
	atom.Proposition.ObjectType = "artifact_ref"
	for _, invalid := range []string{"artifact with spaces", "artifact$illegal"} {
		atom.Proposition.ObjectValue = invalid
		if _, err := SealCognitiveAtom(&atom); err == nil {
			t.Fatalf("artifact_ref proposition %q accepted", invalid)
		}
	}
	transaction := value.DecisionTransaction
	transaction.DecisionTransactionID, transaction.DecisionTransactionSHA256 = "", ""
	transaction.TriggerAtomRefs = append([]AtomRef{}, transaction.GuardAtomRefs[:2]...)
	transaction.TriggerAtomRefs[0], transaction.TriggerAtomRefs[1] =
		transaction.TriggerAtomRefs[1], transaction.TriggerAtomRefs[0]
	if _, err := SealDecisionTransaction(&transaction); err == nil {
		t.Fatal("reordered role refs accepted")
	}
	transaction.TriggerAtomRefs[0] = transaction.TriggerAtomRefs[1]
	if _, err := SealDecisionTransaction(&transaction); err == nil {
		t.Fatal("duplicate role refs accepted")
	}
}

func TestEmptyIOZeroTriggerAndEachRoleBound(t *testing.T) {
	value, _ := golden(t)
	transaction := value.DecisionTransaction
	transaction.DecisionTransactionID, transaction.DecisionTransactionSHA256 = "", ""
	transaction.GuardAtomRefs = append(transaction.GuardAtomRefs, transaction.TriggerAtomRefs...)
	sort.Slice(transaction.GuardAtomRefs, func(i, j int) bool {
		return transaction.GuardAtomRefs[i].AtomID < transaction.GuardAtomRefs[j].AtomID
	})
	transaction.TriggerAtomRefs = []AtomRef{}
	transaction.ReadArtifactReceiptRefs = []op.ArtifactReceiptRef{}
	transaction.WritePreconditions = []WritePrecondition{}
	transaction.WriteSlots = []string{}
	if _, err := SealDecisionTransaction(&transaction); err != nil {
		t.Fatalf("empty-I/O zero-trigger rejected: %v", err)
	}
}

func TestEveryNPlusOneCardinalityAndSelectorBound(t *testing.T) {
	value, _ := golden(t)
	cases := []struct {
		message string
		mutate  func(*DecisionTransaction)
	}{
		{"trigger_atom_refs cardinality", func(value *DecisionTransaction) { value.TriggerAtomRefs = make([]AtomRef, 65) }},
		{"guard_atom_refs cardinality", func(value *DecisionTransaction) { value.GuardAtomRefs = make([]AtomRef, 65) }},
		{"options cardinality", func(value *DecisionTransaction) { value.Options = make([]DecisionOption, 17) }},
		{"proof_obligations cardinality", func(value *DecisionTransaction) { value.ProofObligations = make([]ProofObligation, 33) }},
		{"read_artifact_receipt_refs cardinality", func(value *DecisionTransaction) { value.ReadArtifactReceiptRefs = make([]op.ArtifactReceiptRef, 33) }},
		{"write_preconditions cardinality", func(value *DecisionTransaction) { value.WritePreconditions = make([]WritePrecondition, 33) }},
		{"write_slots cardinality", func(value *DecisionTransaction) { value.WriteSlots = make([]string, 33) }},
		{"required_evidence_kinds cardinality", func(value *DecisionTransaction) {
			value.ProofObligations[0].RequiredEvidenceKinds = make([]string, 17)
		}},
	}
	for index, item := range cases {
		transaction := value.DecisionTransaction
		transaction.DecisionTransactionID, transaction.DecisionTransactionSHA256 = "", ""
		item.mutate(&transaction)
		if _, err := SealDecisionTransaction(&transaction); err == nil || !strings.Contains(err.Error(), item.message) {
			t.Fatalf("transaction N+1 case %d accepted", index)
		}
	}
	atom := artifactAtom(value)
	atom.AtomID, atom.AtomSHA256 = "", ""
	selector := "/" + strings.Repeat("x", maxSelectorBytes)
	atom.Source.SourceSelector = &selector
	if _, err := SealCognitiveAtom(&atom); err == nil || !strings.Contains(err.Error(), "source_selector") {
		t.Fatalf("4097-byte source_selector did not hit selector bound: %v", err)
	}
}

func artifactAtom(value *KernelDecisionReferenceClosure) CognitiveAtom {
	for _, atom := range value.CognitiveAtoms {
		if atom.Source.SourceKind == "artifact" {
			return atom
		}
	}
	panic("golden has no artifact atom")
}

func TestAtomCountAndDocumentNPlusOneBounds(t *testing.T) {
	value, _ := golden(t)
	closure := *value
	closure.ClosureID, closure.ClosureSHA256 = "", ""
	closure.CognitiveAtoms = make([]CognitiveAtom, maxAtoms+1)
	if _, err := SealClosure(&closure); err == nil || !strings.Contains(err.Error(), "cognitive_atoms cardinality") {
		t.Fatalf("257 CognitiveAtoms did not hit cardinality bound: %v", err)
	}
	cases := []struct {
		maximum int
		decode  func([]byte) error
	}{
		{maxAtomBytes, func(raw []byte) error { _, err := DecodeCognitiveAtom(raw); return err }},
		{maxTransactionBytes, func(raw []byte) error { _, err := DecodeDecisionTransaction(raw); return err }},
		{maxClosureBytes, func(raw []byte) error { _, err := DecodeClosure(raw); return err }},
	}
	for _, item := range cases {
		if err := item.decode(bytes.Repeat([]byte(" "), item.maximum+1)); err == nil ||
			!strings.Contains(err.Error(), "JSON byte length") {
			t.Fatalf("%d-byte N+1 did not hit document ceiling: %v", item.maximum, err)
		}
	}
}

func oversizedAtomSet(t *testing.T, value *KernelDecisionReferenceClosure) []CognitiveAtom {
	t.Helper()
	template := artifactAtom(value)
	result := make([]CognitiveAtom, 0, maxAtoms)
	for index := 0; index < maxAtoms; index++ {
		atom := template
		atom.AtomID, atom.AtomSHA256 = "", ""
		subject := fmt.Sprintf("aggregate-boundary-%03d", index)
		atom.Proposition.Subject, atom.Scope.Object = subject, &subject
		selector := "/" + strings.Repeat("x", maxSelectorBytes-2)
		atom.Source.SourceSelector = &selector
		sealed, err := SealCognitiveAtom(&atom)
		if err != nil {
			t.Fatal(err)
		}
		result = append(result, *sealed)
	}
	sort.Slice(result, func(i, j int) bool { return result[i].AtomID < result[j].AtomID })
	return result
}

func TestAtomSetAggregateNPlusOneBound(t *testing.T) {
	value, _ := golden(t)
	atoms := oversizedAtomSet(t, value)
	raw, err := canonicalBytes(atoms, maxClosureBytes)
	if err != nil || len(raw) <= maxAtomSetBytes {
		t.Fatalf("fixture did not exceed aggregate bound: bytes=%d err=%v", len(raw), err)
	}
	if err := sortAtoms(atoms); err != nil {
		t.Fatalf("individually valid aggregate fixture rejected early: %v", err)
	}
	if _, err := canonicalBytes(atoms, maxAtomSetBytes); err == nil ||
		!strings.Contains(err.Error(), "canonical JSON byte length") {
		t.Fatalf("oversized set did not hit aggregate byte ceiling: %v", err)
	}
}
