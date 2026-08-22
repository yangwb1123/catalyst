package kerneldecisioncontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"sort"
	"strings"
	"testing"
)

const goldenPath = "../../../docs/contracts/fixtures/kernel-decision-reference-closure-v1.json"

func golden(t *testing.T) (*KernelDecisionReferenceClosure, []byte) {
	t.Helper()
	physical, err := os.ReadFile(goldenPath)
	if err != nil {
		t.Fatal(err)
	}
	if len(physical) == 0 || physical[len(physical)-1] != '\n' {
		t.Fatal("golden must have exactly one trailing LF")
	}
	value, err := DecodeClosure(physical[:len(physical)-1])
	if err != nil {
		t.Fatal(err)
	}
	return value, physical
}

func TestGoldenPhysicalSemanticAndCanonicalPins(t *testing.T) {
	value, physical := golden(t)
	if got := fmt.Sprintf("%x", sha256.Sum256(physical)); got != "93f6225b745eacf966796cb671d723440890ae3ab02699dd40d6a078f539af1c" {
		t.Fatalf("physical SHA-256 = %s", got)
	}
	if value.ClosureSHA256 != "cdadf0e5fddbbda429939be4e68dc77dd0b52c0bb7e4fe955f1d485183908e58" {
		t.Fatalf("closure SHA-256 = %s", value.ClosureSHA256)
	}
	canonical, err := CanonicalJSON(value)
	if err != nil || !bytes.Equal(canonical, physical[:len(physical)-1]) {
		t.Fatalf("canonical parity failed: %v", err)
	}
}

func TestAllCognitiveAtomsTransactionAndClosureReseal(t *testing.T) {
	value, _ := golden(t)
	for index := range value.CognitiveAtoms {
		expected := value.CognitiveAtoms[index]
		blank := expected
		blank.AtomID, blank.AtomSHA256 = "", ""
		got, err := SealCognitiveAtom(&blank)
		if err != nil || !reflect.DeepEqual(*got, expected) {
			t.Fatalf("atom[%d] reseal: %v", index, err)
		}
	}
	expectedTransaction := value.DecisionTransaction
	blankTransaction := expectedTransaction
	blankTransaction.DecisionTransactionID, blankTransaction.DecisionTransactionSHA256 = "", ""
	gotTransaction, err := SealDecisionTransaction(&blankTransaction)
	if err != nil || !reflect.DeepEqual(*gotTransaction, expectedTransaction) {
		t.Fatalf("transaction reseal: %v", err)
	}
	expected := *value
	blank := expected
	blank.ClosureID, blank.ClosureSHA256 = "", ""
	got, err := SealClosure(&blank)
	if err != nil || !reflect.DeepEqual(*got, expected) {
		t.Fatalf("closure reseal: %v", err)
	}
}

func TestZeroTriggerAcceptedAndSixtyFiveRejected(t *testing.T) {
	value, _ := golden(t)
	transaction := value.DecisionTransaction
	transaction.DecisionTransactionID, transaction.DecisionTransactionSHA256 = "", ""
	transaction.GuardAtomRefs = append(transaction.GuardAtomRefs, transaction.TriggerAtomRefs...)
	sort.Slice(transaction.GuardAtomRefs, func(i, j int) bool {
		return transaction.GuardAtomRefs[i].AtomID < transaction.GuardAtomRefs[j].AtomID
	})
	transaction.TriggerAtomRefs = []AtomRef{}
	if _, err := SealDecisionTransaction(&transaction); err != nil {
		t.Fatalf("zero-trigger transaction rejected: %v", err)
	}
	transaction.TriggerAtomRefs = make([]AtomRef, 65)
	if _, err := SealDecisionTransaction(&transaction); err == nil {
		t.Fatal("65 trigger refs accepted")
	}
}

func TestStrictDecodeRejectsWireAndBadTypes(t *testing.T) {
	_, physical := golden(t)
	raw := physical[:len(physical)-1]
	cases := [][]byte{physical, append([]byte(" "), raw...), append(append([]byte{}, raw...), ' '),
		bytes.Replace(raw, []byte(`"atom_type":"constraint"`), []byte(`"atom_type":[]`), 1),
		bytes.Replace(raw, []byte(`"source_kind":"work_intent"`), []byte(`"source_kind":{}`), 1),
		bytes.Replace(raw, []byte(`"authority_kind":"contract_artifact"`), []byte(`"authority_kind":[]`), 1),
		bytes.Replace(raw, []byte(`"object_type":"string"`), []byte(`"object_type":{}`), 1),
		bytes.Replace(raw, []byte(`"declared_hardness":"contract"`), []byte(`"declared_hardness":[]`), 1),
	}
	for index, changed := range cases {
		if _, err := DecodeClosure(changed); err == nil {
			t.Fatalf("bad wire/type %d accepted", index)
		}
	}
}

func TestStrictUnknownDuplicateUTF8ControlSurrogateDepthAndSize(t *testing.T) {
	_, physical := golden(t)
	raw := physical[:len(physical)-1]
	invalidUTF8 := append([]byte{}, raw...)
	invalidUTF8[20] = 0xff
	cases := [][]byte{
		invalidUTF8,
		bytes.Replace(raw, []byte(`{"api_version":`),
			[]byte(`{"unknown":0,"api_version":`), 1),
		bytes.Replace(raw, []byte(`{"api_version":`),
			[]byte(`{"api_version":"x","api_version":`), 1),
		bytes.Replace(raw, []byte(`"value-03"`), []byte(`"\ud800"`), 1),
		bytes.Replace(raw, []byte(`"value-03"`), []byte(`"\u202e"`), 1),
		bytes.Replace(raw, []byte(`"atom_type":"constraint"`),
			[]byte(`"atom_type":[[[[[[[[[[[[[[[[["x"]]]]]]]]]]]]]]]]]`), 1),
		append(append([]byte{}, raw...), bytes.Repeat([]byte(" "), maxClosureBytes)...),
	}
	for index, changed := range cases {
		if _, err := DecodeClosure(changed); err == nil {
			t.Fatalf("strict mutation %d accepted", index)
		}
	}
}

func TestEveryPublicSealRejectsPreMarshalInvalidUTF8(t *testing.T) {
	value, _ := golden(t)
	invalid := string([]byte{0xff})
	atom := value.CognitiveAtoms[0]
	atom.AtomID, atom.AtomSHA256 = "", ""
	atom.Proposition.ObjectValue = invalid
	if _, err := SealCognitiveAtom(&atom); err == nil {
		t.Fatal("CognitiveAtom seal accepted invalid UTF-8")
	}
	transaction := value.DecisionTransaction
	transaction.DecisionTransactionID, transaction.DecisionTransactionSHA256 = "", ""
	transaction.IdempotencyKey = invalid
	if _, err := SealDecisionTransaction(&transaction); err == nil {
		t.Fatal("DecisionTransaction seal accepted invalid UTF-8")
	}
	closure := *value
	closure.ClosureID, closure.ClosureSHA256 = "", ""
	closure.Result = invalid
	if _, err := SealClosure(&closure); err == nil {
		t.Fatal("closure seal accepted invalid UTF-8")
	}
}

func TestCanonicalJSONRejectsInvalidTypedAndRawStrings(t *testing.T) {
	invalidUTF8 := string([]byte{0xff})
	cases := []any{
		invalidUTF8,
		map[string]any{"nested": []any{"safe", "blocked\u202e"}},
		[]byte("must-not-be-base64-normalized"),
		json.RawMessage([]byte{'{', '"', 'v', '"', ':', '"', 0xff, '"', '}'}),
		json.RawMessage(`{"z":1,"a":2}`),
	}
	for index, value := range cases {
		if _, err := CanonicalJSON(value); err == nil {
			t.Fatalf("CanonicalJSON accepted invalid typed case %d", index)
		}
	}
}

func TestCanonicalJSONRejectsC1Controls(t *testing.T) {
	for _, scalar := range []string{"\u0080", "\u009f"} {
		if _, err := CanonicalJSON(map[string]any{"value": scalar}); err == nil {
			t.Fatalf("CanonicalJSON accepted C1 control U+%04X", []rune(scalar)[0])
		}
	}
}

func TestCanonicalJSONRejectsSharedWireLimitViolations(t *testing.T) {
	deep := any("leaf")
	for range 17 {
		deep = map[string]any{"nested": deep}
	}
	fields := make(map[string]any, maxTypedFields+1)
	for index := 0; index <= maxTypedFields; index++ {
		fields[fmt.Sprintf("field_%d", index)] = index
	}
	cases := []any{
		1.5,
		float64(1),
		deep,
		fields,
		make([]any, maxTypedArrayItems+1),
		strings.Repeat("x", maxTypedStringBytes+1),
		map[string]any{"BadKey": true},
		map[string]any{strings.Repeat("a", maxTypedStringBytes+1): true},
	}
	for index, value := range cases {
		if _, err := CanonicalJSON(value); err == nil {
			t.Fatalf("CanonicalJSON accepted wire-limit case %d", index)
		}
	}
}

func TestCanonicalJSONSortsGenericStructKeys(t *testing.T) {
	type reverseDeclaration struct {
		Zed   string `json:"zed"`
		Alpha string `json:"alpha"`
	}
	raw, err := CanonicalJSON(reverseDeclaration{Zed: "last", Alpha: "first"})
	if err != nil {
		t.Fatal(err)
	}
	if string(raw) != `{"alpha":"first","zed":"last"}` {
		t.Fatalf("generic struct keys are not canonical: %s", raw)
	}
}

func TestCanonicalJSONHonorsGenericStructOmission(t *testing.T) {
	type omittedFields struct {
		Visible int     `json:"visible"`
		Ignored float64 `json:"-"`
		Omitted []byte  `json:"omitted,omitempty"`
	}
	raw, err := CanonicalJSON(omittedFields{Visible: 1, Ignored: 1})
	if err != nil {
		t.Fatalf("wire-omitted unsupported fields were traversed: %v", err)
	}
	if string(raw) != `{"visible":1}` {
		t.Fatalf("generic struct omission changed canonical JSON: %s", raw)
	}
}

func TestCanonicalJSONHonorsTwentyMiBOuterCeiling(t *testing.T) {
	leaf := strings.Repeat("<", maxTypedStringBytes)
	row := make([]any, maxTypedArrayItems)
	for index := range row {
		row[index] = leaf
	}
	value := []any{row, row, row, row}
	raw, err := CanonicalJSON(value)
	if err != nil {
		t.Fatalf("ADR-0090 rejected canonical JSON between 16 and 20 MiB: %v", err)
	}
	if len(raw) <= 16*1024*1024 || len(raw) > maxClosureBytes {
		t.Fatalf("programmatic boundary fixture has %d bytes", len(raw))
	}
	roundTrip, err := CanonicalJSON(json.RawMessage(raw))
	if err != nil || !bytes.Equal(roundTrip, raw) {
		t.Fatalf("ADR-0090 RawMessage inherited the ADR-0088 ceiling: %v", err)
	}
}

func assertSealedUnchanged(t *testing.T, value any, before []byte, validate func() error) {
	t.Helper()
	after, err := CanonicalJSON(value)
	if err != nil || !bytes.Equal(after, before) {
		t.Fatalf("caller mutation changed sealed value: %v", err)
	}
	if err := validate(); err != nil {
		t.Fatalf("caller mutation invalidated sealed value: %v", err)
	}
}

func TestSealCognitiveAtomDeepCloneIsolation(t *testing.T) {
	value, _ := golden(t)
	input := value.CognitiveAtoms[0]
	input.AtomID, input.AtomSHA256 = "", ""
	sealed, err := SealCognitiveAtom(&input)
	if err != nil {
		t.Fatal(err)
	}
	before, _ := CanonicalJSON(sealed)
	input.Source.SourceRef[0] = 'x'
	input.DeclaredAuthority.AuthorityRef[0] = 'x'
	if input.Scope.Module != nil {
		*input.Scope.Module = "caller-mutated-module"
	}
	assertSealedUnchanged(t, sealed, before, func() error { return ValidateCognitiveAtom(sealed) })
}

func TestSealDecisionTransactionDeepCloneIsolation(t *testing.T) {
	value, _ := golden(t)
	input := value.DecisionTransaction
	input.DecisionTransactionID, input.DecisionTransactionSHA256 = "", ""
	sealed, err := SealDecisionTransaction(&input)
	if err != nil {
		t.Fatal(err)
	}
	before, _ := CanonicalJSON(sealed)
	input.TriggerAtomRefs[0].AtomID = "caller-mutated-atom"
	input.Options[0].Capability.CapabilityID = "caller-mutated-capability"
	if input.TaskBinding.AttemptID != nil {
		*input.TaskBinding.AttemptID = "caller-mutated-attempt"
	}
	assertSealedUnchanged(t, sealed, before, func() error { return ValidateDecisionTransaction(sealed) })
}

func TestSealClosureDeepCloneIsolation(t *testing.T) {
	value, _ := golden(t)
	input := *value
	input.ClosureID, input.ClosureSHA256 = "", ""
	sealed, err := SealClosure(&input)
	if err != nil {
		t.Fatal(err)
	}
	before, _ := CanonicalJSON(sealed)
	input.CognitiveAtoms[0].AtomType = "caller-mutated-type"
	input.OperationalClosure.CapabilityInvocations[0].DeclaredOutputSlots[0] = "caller-mutated-slot"
	assertSealedUnchanged(t, sealed, before, func() error { return ValidateClosure(sealed) })
}
