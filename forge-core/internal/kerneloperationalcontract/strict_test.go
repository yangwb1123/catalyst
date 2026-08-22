package kerneloperationalcontract

import (
	"fmt"
	"strings"
	"testing"
)

func mustStrict(t *testing.T, raw []byte) {
	t.Helper()
	if _, err := parseStrictJSON(raw, len(raw)); err != nil {
		t.Fatalf("strict boundary rejected: %v", err)
	}
}

func mustRejectStrict(t *testing.T, raw []byte) {
	t.Helper()
	if _, err := parseStrictJSON(raw, len(raw)); err == nil {
		t.Fatalf("strict boundary accepted %q", raw)
	}
}

func TestStrictNumberUTF8AndUnicodeCorpus(t *testing.T) {
	for _, raw := range [][]byte{[]byte("1.0"), []byte("1e0"), []byte("NaN"),
		[]byte("Infinity"), {'"', 0xff, '"'}} {
		mustRejectStrict(t, raw)
	}
	for _, raw := range [][]byte{[]byte("9223372036854775808"),
		[]byte("-9223372036854775809"), []byte("01"), []byte("-0")} {
		mustRejectStrict(t, raw)
	}
	mustStrict(t, []byte("9223372036854775807"))
	mustStrict(t, []byte("-9223372036854775808"))
	for _, scalar := range []rune{0, 0x1f, 0x7f, 0x80, 0x9f, 0x061c,
		0x200e, 0x200f, 0x2028, 0x2029, 0x202e, 0x2066, 0x2069} {
		mustRejectStrict(t, []byte(fmt.Sprintf("%q", string(scalar))))
	}
	mustRejectStrict(t, []byte(`"\ud800"`))
}

func TestStrictDepthFieldArrayAndStringBoundaries(t *testing.T) {
	depth16 := strings.Repeat("[", 15) + "0" + strings.Repeat("]", 15)
	depth17 := "[" + depth16 + "]"
	mustStrict(t, []byte(depth16))
	mustRejectStrict(t, []byte(depth17))
	mustStrict(t, []byte(objectWithFields(64)))
	mustRejectStrict(t, []byte(objectWithFields(65)))
	mustStrict(t, []byte(integerArray(256)))
	mustRejectStrict(t, []byte(integerArray(257)))
	mustStrict(t, []byte(`"`+strings.Repeat("é", maxStringBytes/2)+`"`))
	mustRejectStrict(t, []byte(`"`+strings.Repeat("é", maxStringBytes/2+1)+`"`))
}

func objectWithFields(count int) string {
	fields := make([]string, count)
	for index := range fields {
		fields[index] = fmt.Sprintf(`"a%02d":0`, index)
	}
	return "{" + strings.Join(fields, ",") + "}"
}

func integerArray(count int) string {
	items := make([]string, count)
	for index := range items {
		items[index] = fmt.Sprintf("%d", index)
	}
	return "[" + strings.Join(items, ",") + "]"
}

func TestEveryStandaloneDecoderHasAnExactByteCeiling(t *testing.T) {
	closure, _ := goldenClosure(t)
	tests := []struct {
		maximum int
		decode  func([]byte) error
	}{
		{maxArtifactRefBytes, decodeArtifactRefError},
		{maxArtifactReceiptBytes, decodeArtifactError},
		{maxInvocationBytes, decodeInvocationError},
		{maxEventBytes, decodeEventError},
		{maxExecutionReceiptBytes, decodeReceiptError},
		{maxClosureBytes, decodeClosureError},
	}
	valid := []any{closure.Artifacts[0], closure.ArtifactReceipts[0],
		closure.CapabilityInvocations[0], closure.InteractionEvents[0],
		closure.ExecutionReceipts[0], *closure}
	for index, test := range tests {
		raw, err := CanonicalJSON(valid[index])
		if err != nil || test.decode(raw) != nil {
			t.Fatalf("standalone valid %d: %v", index, err)
		}
		if test.decode(make([]byte, test.maximum+1)) == nil {
			t.Fatalf("standalone N+1 bytes %d accepted", index)
		}
	}
}

func TestTypedArtifactRefRejectsForbiddenScalarsAndShapeDrift(t *testing.T) {
	digest := strings.Repeat("a", 64)
	for _, reference := range []string{"\x7f", "\u0080", "\u2028", "\u2029", "\u202e"} {
		raw := []byte(fmt.Sprintf(
			`{"artifact_kind":"fixture","artifact_ref":"%s","artifact_sha256":"%s"}`,
			reference, digest))
		if _, err := DecodeArtifactRef(raw); err == nil {
			t.Fatalf("forbidden ArtifactRef scalar U+%04X accepted", []rune(reference)[0])
		}
	}
	for _, raw := range [][]byte{
		[]byte(fmt.Sprintf(`{"artifact_kind":"fixture","artifact_ref":"\ud800","artifact_sha256":"%s"}`, digest)),
		[]byte(fmt.Sprintf(`{"artifact_kind":"fixture","artifact_ref":"x","artifact_sha256":"%s","extra":0}`, digest)),
	} {
		if _, err := DecodeArtifactRef(raw); err == nil {
			t.Fatal("surrogate or extra field accepted")
		}
	}
}

func TestCanonicalJSONRejectsInvalidUTF8BeforeMarshalNormalization(t *testing.T) {
	invalid := string([]byte{0xff})
	for _, value := range []any{invalid, map[string]any{"nested": []any{invalid}}} {
		if _, err := CanonicalJSON(value); err == nil {
			t.Fatal("CanonicalJSON normalized caller-supplied invalid UTF-8")
		}
	}
	closure, _ := goldenClosure(t)
	artifact := closure.Artifacts[0]
	artifact.ArtifactRef = invalid
	if _, err := CanonicalJSON(artifact); err == nil {
		t.Fatal("CanonicalJSON normalized invalid UTF-8 in a typed record field")
	}
}

type sealErrorProbe struct {
	name string
	seal func() error
}

func invalidUTF8ClosureProbe(t *testing.T, invalid string) *KernelOperationalReferenceClosure {
	t.Helper()
	normalized := emptyProfile(t)
	receipt := normalized.ExecutionReceipts[0]
	receipt.ExecutionReceiptID, receipt.ExecutionReceiptSHA256 = "", ""
	receipt.Executor.PrincipalID = "\ufffd"
	sealedReceipt, err := SealExecutionReceipt(&receipt)
	if err != nil {
		t.Fatal(err)
	}
	candidate := *normalized
	candidate.ClosureID, candidate.ClosureSHA256 = "", ""
	candidate.ExecutionReceipts = []ExecutionReceipt{*sealedReceipt}
	sealed, err := SealClosure(&candidate)
	if err != nil {
		t.Fatal(err)
	}
	probe := *sealed
	probe.ClosureID, probe.ClosureSHA256 = "", ""
	probe.ExecutionReceipts = []ExecutionReceipt{sealed.ExecutionReceipts[0]}
	probe.ExecutionReceipts[0].Executor.PrincipalID = invalid
	return &probe
}

func invalidUTF8SealProbes(t *testing.T) []sealErrorProbe {
	t.Helper()
	closure, _ := goldenClosure(t)
	invalid := string([]byte{0xff})
	artifact := closure.ArtifactReceipts[0]
	artifact.ArtifactReceiptID, artifact.ArtifactReceiptSHA256 = "", ""
	artifact.Artifact.ArtifactRef = invalid
	invocation := closure.CapabilityInvocations[0]
	invocation.InvocationID, invocation.InvocationSHA256 = "", ""
	invocation.Bindings.SourceRevision = invalid
	event := closure.InteractionEvents[0]
	event.EventID, event.EventSHA256 = "", ""
	event.ObjectRef = invalid
	receipt := closure.ExecutionReceipts[0]
	receipt.ExecutionReceiptID, receipt.ExecutionReceiptSHA256 = "", ""
	receipt.Executor.PrincipalID = invalid
	closureProbe := invalidUTF8ClosureProbe(t, invalid)
	return []sealErrorProbe{
		{"ArtifactReceipt", func() error { _, err := SealArtifactReceipt(&artifact); return err }},
		{"CapabilityInvocation", func() error { _, err := SealCapabilityInvocation(&invocation); return err }},
		{"InteractionEvent", func() error { _, err := SealInteractionEvent(&event); return err }},
		{"ExecutionReceipt", func() error { _, err := SealExecutionReceipt(&receipt); return err }},
		{"Closure", func() error { _, err := SealClosure(closureProbe); return err }},
	}
}

func TestCloneAndEverySealRejectInvalidUTF8BeforeNormalization(t *testing.T) {
	invalid := string([]byte{0xff})
	if _, err := cloneValue(&struct{ Value string }{Value: invalid}); err == nil {
		t.Fatal("cloneValue normalized caller-supplied invalid UTF-8")
	}
	for _, probe := range invalidUTF8SealProbes(t) {
		if err := probe.seal(); err == nil {
			t.Fatalf("Seal%s normalized caller-supplied invalid UTF-8", probe.name)
		}
	}
}

func decodeArtifactRefError(raw []byte) error {
	_, err := DecodeArtifactRef(raw)
	return err
}

func decodeEventError(raw []byte) error {
	_, err := DecodeInteractionEvent(raw)
	return err
}

func decodeClosureError(raw []byte) error {
	_, err := DecodeClosure(raw)
	return err
}
