package materiality

import "testing"

func TestNormalizeUsesClosedVocabularyAndExplicitUnboundSentinel(t *testing.T) {
	for _, value := range []string{"L0", "L1", "L2", "L3", "L4", Unbound} {
		if got, err := Normalize(value); err != nil || got != value || !Valid(got) {
			t.Fatalf("Normalize(%q) = %q, %v", value, got, err)
		}
	}
	if got, err := Normalize(""); err != nil || got != Unbound {
		t.Fatalf("Normalize(empty) = %q, %v", got, err)
	}
	for _, invalid := range []string{"l3", " L3", "L5", "high"} {
		if _, err := Normalize(invalid); err == nil {
			t.Errorf("Normalize(%q) accepted", invalid)
		}
	}
}

func TestStrictReviewFloorIsOnlyL3AndL4(t *testing.T) {
	for _, value := range []string{Unbound, "L0", "L1", "L2"} {
		if RequiresStrictReview(value) {
			t.Errorf("%s unexpectedly requires strict review", value)
		}
	}
	for _, value := range []string{"L3", "L4"} {
		if !RequiresStrictReview(value) {
			t.Errorf("%s must require strict review", value)
		}
	}
}

func TestFromCLIRejectsExplicitEmptyAndInternalSentinel(t *testing.T) {
	if got, err := FromCLI("", false); err != nil || got != Unbound {
		t.Fatalf("omitted materiality = %q, %v", got, err)
	}
	for _, value := range []string{"", Unbound} {
		if _, err := FromCLI(value, true); err == nil {
			t.Errorf("explicit %q accepted", value)
		}
	}
}
