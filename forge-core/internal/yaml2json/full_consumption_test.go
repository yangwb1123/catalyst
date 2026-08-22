package yaml2json

import (
	"strings"
	"testing"
)

func TestDecodeRejectsUnconsumedLogicalLines(t *testing.T) {
	for _, input := range []string{
		"stage: build\nstray scalar\nphases: []\n",
		"stage: build\n---\nphases: []\n",
		"outer:\n  inner: value\n stray\n",
	} {
		if _, err := Decode(strings.NewReader(input)); err == nil || !strings.Contains(err.Error(), "unconsumed content") {
			t.Errorf("Decode unconsumed content error = %v for %q", err, input)
		}
	}
}

func TestDecodeRejectsTrailingInlineContent(t *testing.T) {
	for _, input := range []string{
		"gates: [test] ignored\n",
		"on_fail: {action: loop_back} ignored\n",
		"items:\n  - [one, two] ignored\n",
	} {
		if _, err := Decode(strings.NewReader(input)); err == nil || !strings.Contains(err.Error(), "trailing content") {
			t.Errorf("Decode trailing inline content error = %v for %q", err, input)
		}
	}
}
