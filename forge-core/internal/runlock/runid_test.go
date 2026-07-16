package runlock

import "testing"

func TestNewRunID_FormatAndUniqueness(t *testing.T) {
	id := NewRunID()
	if id == "" {
		t.Fatal("NewRunID() returned empty string")
	}

	sep := -1
	for i, r := range id {
		if r == '-' {
			sep = i
			break
		}
	}
	if sep <= 0 || sep == len(id)-1 {
		t.Fatalf("NewRunID() = %q, want a single '-' separating two non-empty hex halves", id)
	}
	timePart, randPart := id[:sep], id[sep+1:]
	if !isHex(timePart) {
		t.Errorf("NewRunID() time component %q is not hex", timePart)
	}
	if !isHex(randPart) {
		t.Errorf("NewRunID() rand component %q is not hex", randPart)
	}
	// hex(4 crypto/rand bytes) == exactly 8 hex chars.
	if len(randPart) != 8 {
		t.Errorf("NewRunID() rand component %q has length %d, want 8 (4 bytes hex-encoded)", randPart, len(randPart))
	}

	const n = 1000
	seen := make(map[string]bool, n)
	for i := 0; i < n; i++ {
		gen := NewRunID()
		if seen[gen] {
			t.Fatalf("duplicate id generated within a %d-iteration tight loop (same-nanosecond-tick case): %q", n, gen)
		}
		seen[gen] = true
	}
}

func isHex(s string) bool {
	if s == "" {
		return false
	}
	for _, r := range s {
		switch {
		case r >= '0' && r <= '9':
		case r >= 'a' && r <= 'f':
		default:
			return false
		}
	}
	return true
}
