package yaml2json

import (
	"strings"
	"testing"
)

func TestDecodeRejectsDuplicateMappingKeysAtEveryWorkflowShape(t *testing.T) {
	for _, input := range []string{
		"stage: build\nstage: review\n",
		"outer:\n  fresh_context: true\n  fresh_context: false\n",
		"phases:\n  - name: reviewer\n    readonly: true\n    readonly: false\n",
		"phases:\n  - name: reviewer\n    on_fail: {action: loop_back, action: continue}\n",
	} {
		if _, err := Decode(strings.NewReader(input)); err == nil || !strings.Contains(err.Error(), "duplicate") {
			t.Errorf("Decode duplicate keys error = %v for %q", err, input)
		}
	}
}

func TestDecodeStillAllowsSameKeyInDistinctMappings(t *testing.T) {
	input := "phases:\n  - name: one\n    readonly: true\n  - name: two\n    readonly: false\n"
	if _, err := Decode(strings.NewReader(input)); err != nil {
		t.Fatalf("same keys in separate phase mappings rejected: %v", err)
	}
}
