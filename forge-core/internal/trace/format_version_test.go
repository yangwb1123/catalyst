package trace

import (
	"bytes"
	"strings"
	"testing"
)

func TestTraceFormatRejectsUnknownGeneration(t *testing.T) {
	var out bytes.Buffer
	tracer := NewTracer(&out)
	err := tracer.Emit(Event{Format: "forgeos.trace.v2", Kind: "agent", Name: "planner"})
	if err == nil || !strings.Contains(err.Error(), "unsupported trace format") {
		t.Fatalf("Emit unknown format error = %v", err)
	}
	if out.Len() != 0 {
		t.Fatalf("unsupported trace event was written: %q", out.String())
	}
}

func TestTraceFormatAcceptsLegacyEmptyAndCurrent(t *testing.T) {
	for _, format := range []string{"", FormatV1} {
		if err := ValidateFormat(format); err != nil {
			t.Errorf("ValidateFormat(%q) = %v", format, err)
		}
	}
}
