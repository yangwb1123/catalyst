package transitionreceiptcontract

import (
	"strings"
	"testing"
)

func TestIntrinsicReceiptContradictionsFailClosed(t *testing.T) {
	fixture := loadGolden(t)
	mutations := []func(map[string]any){
		func(r map[string]any) {
			r["previous_receipt_sha256"] = strings.Repeat("a", 64)
			r["previous_receipt_id"] = "transition-receipt-" + strings.Repeat("a", 64)
		},
		func(r map[string]any) { r["sequence"] = int64(2) },
		func(r map[string]any) { r["transition"].(map[string]any)["from_state"] = "BASELINED" },
		func(r map[string]any) { r["applicability"].(map[string]any)["stage_id"] = "BASELINED" },
		func(r map[string]any) {
			r["applicability"].(map[string]any)["reason_codes"] = []any{"contradiction"}
		},
		func(r map[string]any) {
			transition := r["transition"].(map[string]any)
			transition["to_state"] = "CHANGES_REQUESTED"
			r["applicability"].(map[string]any)["stage_id"] = "CHANGES_REQUESTED"
		},
		func(r map[string]any) {
			transition := r["transition"].(map[string]any)
			transition["to_state"] = "NEEDS_INFO"
			r["applicability"].(map[string]any)["stage_id"] = "NEEDS_INFO"
		},
	}
	for index, mutate := range mutations {
		receipt := cloneNode(fixtureNode(t, fixture, "transition_receipt"))
		mutate(receipt)
		receipt["receipt_id"], receipt["receipt_sha256"] = "", ""
		if err := validateReceipt(receipt, true); err == nil {
			t.Fatalf("intrinsic contradiction %d unexpectedly passed", index)
		}
	}
}

func TestApplicabilityContradictionCannotBecomeAssessmentRelation(t *testing.T) {
	fixture := loadGolden(t)
	assessment := cloneNode(fixtureNode(t, fixture, "expected_assessment"))
	assessment["relations"].(map[string]any)["applicability"] =
		"applicability_declaration_mismatch"
	assessment["reason_codes"] = []any{"applicability_declaration_mismatch"}
	assessment["assessment_sha256"] = ""
	digest, err := assessmentDigest(assessment)
	if err != nil {
		t.Fatal(err)
	}
	assessment["assessment_sha256"] = digest
	if err := validateAssessment(assessment); err == nil {
		t.Fatal("intrinsic applicability contradiction became an assessment relation")
	}
}

func TestStrictDecoderRejectsJSONAmbiguity(t *testing.T) {
	tests := [][]byte{
		[]byte(`{"a":1,"a":2}`),
		[]byte(`{"a":1.0}`),
		[]byte(`{"a":01}`),
		[]byte(`{"A":1}`),
		[]byte(`{"a":"\u202e"}`),
		[]byte(`{"a":"\u000a"}`),
		[]byte(`{"a":9223372036854775808}`),
	}
	for index, raw := range tests {
		if _, err := parseStrictJSON(raw, maxReceiptBytes); err == nil {
			t.Fatalf("ambiguous JSON %d unexpectedly passed", index)
		}
	}
}

func TestCanonicalDecoderRejectsUnknownAliasAndFormatting(t *testing.T) {
	fixture := loadGolden(t)
	receipt := fixtureNode(t, fixture, "transition_receipt")
	encoded, err := CanonicalReceiptJSON(receipt)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeCanonicalReceipt(append([]byte{' '}, encoded...)); err == nil {
		t.Fatal("leading whitespace unexpectedly passed")
	}
	alias := cloneNode(receipt)
	alias["state"] = "DRAFT"
	aliasBytes, err := canonicalJSON(alias)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := DecodeCanonicalReceipt(aliasBytes); err == nil {
		t.Fatal("unknown state alias unexpectedly passed")
	}
}

func TestEveryDocumentCeilingAppliesToProgrammaticInput(t *testing.T) {
	limits := []int{maxVocabularyBytes, maxAssessmentBytes, maxReceiptBytes,
		maxTargetBytes, maxRequestBytes}
	for _, maximum := range limits {
		document := oversizedCanonicalDocument(maximum)
		if err := validateCanonicalByteLimit(document, maximum, "programmatic"); err == nil {
			t.Fatalf("programmatic document exceeded %d bytes without rejection", maximum)
		}
	}
}

func oversizedCanonicalDocument(maximum int) map[string]any {
	item := strings.Repeat("x", maxStringBytes)
	count := maximum/(maxStringBytes+3) + 1
	values := make([]any, count)
	for index := range values {
		values[index] = item
	}
	return map[string]any{"values": values}
}

func TestProgrammaticDepthAndTypeBoundsFailBeforeEncoding(t *testing.T) {
	var value any = "leaf"
	for index := 0; index < maxJSONDepth; index++ {
		value = map[string]any{"child": value}
	}
	if _, err := canonicalJSON(value); err == nil {
		t.Fatal("programmatic over-depth value unexpectedly encoded")
	}
	if _, err := canonicalJSON(map[string]any{"number": 1}); err == nil {
		t.Fatal("machine-sized int bypassed signed int64 contract")
	}
	cycle := map[string]any{}
	cycle["self"] = cycle
	if _, err := canonicalJSON(cycle); err == nil {
		t.Fatal("cyclic programmatic value unexpectedly encoded")
	}
}

func TestProgrammaticMalformedTargetFailsClosed(t *testing.T) {
	fixture := loadGolden(t)
	target := fixtureNode(t, fixtureNode(t, fixture, "assessment_request"), "expected_target")
	mutations := []func(map[string]any){
		func(node map[string]any) { node["sequence"] = "2" },
		func(node map[string]any) { node["work_id"] = []any{} },
		func(node map[string]any) { node["reason_codes"] = map[string]any{} },
	}
	for index, mutate := range mutations {
		candidate := cloneNode(target)
		mutate(candidate)
		if err := validateTarget(candidate); err == nil {
			t.Fatalf("malformed target %d unexpectedly passed", index)
		}
	}
}

func TestIdentityAndOrderingTamperingFailClosed(t *testing.T) {
	fixture := loadGolden(t)
	receipt := cloneNode(fixtureNode(t, fixture, "transition_receipt"))
	receipt["receipt_sha256"] = strings.Repeat("0", 64)
	receipt["receipt_id"] = "transition-receipt-" + strings.Repeat("0", 64)
	if err := validateReceipt(receipt, false); err == nil {
		t.Fatal("receipt digest tampering unexpectedly passed")
	}
	receipt = cloneNode(fixtureNode(t, fixture, "transition_receipt"))
	precondition := cloneNode(receipt["preconditions"].([]any)[0].(map[string]any))
	receipt["preconditions"] = []any{precondition, cloneNode(precondition)}
	receipt["receipt_id"], receipt["receipt_sha256"] = "", ""
	if err := validateReceipt(receipt, true); err == nil {
		t.Fatal("duplicate ordered precondition unexpectedly passed")
	}
}
