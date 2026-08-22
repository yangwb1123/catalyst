package asset

import (
	"strings"
	"testing"
)

func TestLoadWorkflowJSONRejectsDuplicateKeysRecursively(t *testing.T) {
	for _, data := range []string{
		`{"stage":"build","stage":"review"}`,
		`{"stage":"build","phases":[{"name":"p","agent":"reviewer","readonly":true,"readonly":false}]}`,
		`{"stage":"build","phases":[{"name":"p","agent":"reviewer","on_fail":{"action":"loop_back","action":"continue"}}]}`,
		`{"stage":"build","phases":[],"stop_condition":{"type":"external","type":"conjunction"}}`,
	} {
		if _, err := LoadWorkflowJSON([]byte(data)); err == nil || !strings.Contains(err.Error(), "duplicate JSON object key") {
			t.Errorf("LoadWorkflowJSON duplicate error = %v for %s", err, data)
		}
	}
}

func TestLoadWorkflowJSONAllowsSameKeyInSeparatePhaseObjects(t *testing.T) {
	data := `{"stage":"review","phases":[{"name":"one","agent":"reviewer"},{"name":"two","agent":"reviewer"}]}`
	if _, err := LoadWorkflowJSON([]byte(data)); err != nil {
		t.Fatalf("separate object keys rejected: %v", err)
	}
}
