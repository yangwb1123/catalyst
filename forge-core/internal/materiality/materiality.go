// Package materiality owns the caller-declared change-impact vocabulary used
// to select runtime assurance floors. It classifies no work itself: an omitted
// declaration remains visibly unbound and can never be inferred from a mode.
package materiality

import "fmt"

const Unbound = "materiality_not_bound"

var levels = map[string]struct{}{
	"L0": {}, "L1": {}, "L2": {}, "L3": {}, "L4": {}, Unbound: {},
}

// Normalize maps an omitted declaration to the durable unbound sentinel and
// rejects every spelling outside the closed L0-L4 vocabulary.
func Normalize(value string) (string, error) {
	if value == "" {
		return Unbound, nil
	}
	if _, ok := levels[value]; !ok {
		return "", fmt.Errorf("materiality must be one of L0|L1|L2|L3|L4 (got %q)", value)
	}
	return value, nil
}

// FromCLI distinguishes omission from an explicitly empty/internal value.
func FromCLI(value string, explicit bool) (string, error) {
	if explicit && (value == "" || value == Unbound) {
		return "", fmt.Errorf("materiality flag requires an explicit L0|L1|L2|L3|L4 value")
	}
	return Normalize(value)
}

func Valid(value string) bool {
	_, ok := levels[value]
	return ok
}

func RequiresStrictReview(value string) bool { return value == "L3" || value == "L4" }
