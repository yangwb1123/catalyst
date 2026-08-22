package kerneloperationalcontract

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"os"
	"reflect"
	"strings"
	"testing"
)

const goldenRelativePath = "../../../docs/contracts/fixtures/kernel-operational-reference-closure-v1.json"

func goldenClosure(t *testing.T) (*KernelOperationalReferenceClosure, []byte) {
	t.Helper()
	physical, err := os.ReadFile(goldenRelativePath)
	if err != nil {
		t.Fatal(err)
	}
	if len(physical) == 0 || physical[len(physical)-1] != '\n' {
		t.Fatal("golden must have one trailing LF")
	}
	closure, err := DecodeClosure(physical[:len(physical)-1])
	if err != nil {
		t.Fatal(err)
	}
	return closure, physical
}

func refInvocation(value CapabilityInvocation) CapabilityInvocationRef {
	return CapabilityInvocationRef{InvocationID: value.InvocationID,
		InvocationSHA256: value.InvocationSHA256}
}

func refReceipt(value ArtifactReceipt) ArtifactReceiptRef {
	return ArtifactReceiptRef{ArtifactReceiptID: value.ArtifactReceiptID,
		ArtifactReceiptSHA256: value.ArtifactReceiptSHA256}
}

func refEvent(value InteractionEvent) InteractionEventRef {
	return InteractionEventRef{EventID: value.EventID, EventSHA256: value.EventSHA256}
}

func mutateCanonical(t *testing.T, raw []byte, path []string, value any, remove bool) []byte {
	t.Helper()
	node, err := parseStrictJSON(raw, maxClosureBytes)
	if err != nil {
		t.Fatal(err)
	}
	current, ok := node.(map[string]any)
	if !ok {
		t.Fatal("mutation root is not an object")
	}
	for _, field := range path[:len(path)-1] {
		current, ok = current[field].(map[string]any)
		if !ok {
			t.Fatalf("mutation path %q is not an object", field)
		}
	}
	field := path[len(path)-1]
	if remove {
		delete(current, field)
	} else {
		current[field] = value
	}
	result, err := canonicalJSON(node, maxClosureBytes)
	if err != nil {
		t.Fatal(err)
	}
	return result
}

func TestGoldenPhysicalAndSemanticPins(t *testing.T) {
	closure, physical := goldenClosure(t)
	wantPhysical := "85f8d9887331fe95e52533c228e40b41750f04dfe10f3a7c77e5a4daff785f2f"
	if got := fmt.Sprintf("%x", sha256.Sum256(physical)); got != wantPhysical {
		t.Fatalf("physical SHA-256 = %s", got)
	}
	wantClosure := "1db702583b8dae850413b75b80d620a6031ad452071908e33ea551a4f5feae0e"
	if closure.ClosureSHA256 != wantClosure {
		t.Fatalf("closure SHA-256 = %s", closure.ClosureSHA256)
	}
	canonical, err := CanonicalJSON(closure)
	if err != nil || !bytes.Equal(canonical, physical[:len(physical)-1]) {
		t.Fatalf("canonical parity failed: %v", err)
	}
}

func TestCanonicalJSONRetainsSixteenMiBCeiling(t *testing.T) {
	leaf := strings.Repeat("x", maxStringBytes)
	row := make([]any, maxArrayItems)
	for index := range row {
		row[index] = leaf
	}
	value := []any{row, row, row, row}
	if _, err := CanonicalJSON(value); err == nil {
		t.Fatal("ADR-0088 CanonicalJSON accepted more than 16 MiB")
	}
}

func TestEveryGoldenRecordResealsExactly(t *testing.T) {
	closure, _ := goldenClosure(t)
	for index := range closure.ArtifactReceipts {
		want := closure.ArtifactReceipts[index]
		blank := want
		blank.ArtifactReceiptID, blank.ArtifactReceiptSHA256 = "", ""
		got, err := SealArtifactReceipt(&blank)
		if err != nil || !reflect.DeepEqual(*got, want) {
			t.Fatalf("ArtifactReceipt[%d] reseal: %v", index, err)
		}
	}
	for index := range closure.CapabilityInvocations {
		want := closure.CapabilityInvocations[index]
		blank := want
		blank.InvocationID, blank.InvocationSHA256 = "", ""
		got, err := SealCapabilityInvocation(&blank)
		if err != nil || !reflect.DeepEqual(*got, want) {
			t.Fatalf("CapabilityInvocation[%d] reseal: %v", index, err)
		}
	}
	assertEventAndReceiptReseals(t, closure)
}

func assertEventAndReceiptReseals(t *testing.T, closure *KernelOperationalReferenceClosure) {
	t.Helper()
	for index := range closure.InteractionEvents {
		want := closure.InteractionEvents[index]
		blank := want
		blank.EventID, blank.EventSHA256 = "", ""
		got, err := SealInteractionEvent(&blank)
		if err != nil || !reflect.DeepEqual(*got, want) {
			t.Fatalf("InteractionEvent[%d] reseal: %v", index, err)
		}
	}
	for index := range closure.ExecutionReceipts {
		want := closure.ExecutionReceipts[index]
		blank := want
		blank.ExecutionReceiptID, blank.ExecutionReceiptSHA256 = "", ""
		got, err := SealExecutionReceipt(&blank)
		if err != nil || !reflect.DeepEqual(*got, want) {
			t.Fatalf("ExecutionReceipt[%d] reseal: %v", index, err)
		}
	}
	blank := *closure
	blank.ClosureID, blank.ClosureSHA256 = "", ""
	got, err := SealClosure(&blank)
	if err != nil || !reflect.DeepEqual(*got, *closure) {
		t.Fatalf("closure reseal: %v", err)
	}
}

func TestStrictCanonicalDecodeRejectsWireDrift(t *testing.T) {
	_, physical := goldenClosure(t)
	raw := physical[:len(physical)-1]
	cases := [][]byte{physical, append([]byte(" "), raw...), append(append([]byte{}, raw...), ' ')}
	cases = append(cases, bytes.Replace(raw, []byte(`{"api_version":`),
		[]byte(`{"api_version":"x","api_version":`), 1))
	for index, changed := range cases {
		if _, err := DecodeClosure(changed); err == nil {
			t.Fatalf("wire drift %d accepted", index)
		}
	}
	unknown := mutateCanonical(t, raw, []string{"authority"}, false, false)
	if _, err := DecodeClosure(unknown); err == nil {
		t.Fatal("unknown closure field accepted")
	}
}

func TestReusedNestedRuntimeVectors(t *testing.T) {
	closure, _ := goldenClosure(t)
	invocation, _ := CanonicalJSON(closure.CapabilityInvocations[0])
	artifact, _ := CanonicalJSON(closure.ArtifactReceipts[0])
	receipt, _ := CanonicalJSON(closure.ExecutionReceipts[0])
	tests := []struct {
		raw    []byte
		path   []string
		value  any
		remove bool
		decode func([]byte) error
	}{
		{invocation, []string{"subject", "authority_domain"}, nil, true, decodeInvocationError},
		{invocation, []string{"task_binding", "extra"}, "x", false, decodeInvocationError},
		{invocation, []string{"capability", "capability_contract_sha256"}, "bad", false, decodeInvocationError},
		{artifact, []string{"artifact", "artifact_kind"}, int64(1), false, decodeArtifactError},
		{invocation, []string{"capability_grant_ref", "authority_domain"}, nil, true, decodeInvocationError},
		{invocation, []string{"capability_grant_ref", "extra"}, "x", false, decodeInvocationError},
		{invocation, []string{"capability_grant_ref", "authority_domain"}, "", false, decodeInvocationError},
		{invocation, []string{"capability_grant_ref", "grant_sha256"}, "bad", false, decodeInvocationError},
		{invocation, []string{"capability_grant_ref", "grant_id"}, int64(1), false, decodeInvocationError},
		{receipt, []string{"observed_usage", "call_count"}, nil, true, decodeReceiptError},
		{receipt, []string{"observed_usage", "extra"}, int64(0), false, decodeReceiptError},
		{receipt, []string{"observed_usage", "call_count"}, true, false, decodeReceiptError},
		{receipt, []string{"observed_usage", "call_count"}, int64(-1), false, decodeReceiptError},
	}
	for index, test := range tests {
		changed := mutateCanonical(t, test.raw, test.path, test.value, test.remove)
		if err := test.decode(changed); err == nil {
			t.Fatalf("nested runtime vector %d accepted", index)
		}
	}
}

func decodeInvocationError(raw []byte) error {
	_, err := DecodeCapabilityInvocation(raw)
	return err
}

func decodeArtifactError(raw []byte) error {
	_, err := DecodeArtifactReceipt(raw)
	return err
}

func decodeReceiptError(raw []byte) error {
	_, err := DecodeExecutionReceipt(raw)
	return err
}
