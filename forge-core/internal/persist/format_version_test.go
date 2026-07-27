package persist

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCheckpointFormatRejectsUnknownGeneration(t *testing.T) {
	if _, err := decode([]byte(`{"_format":"forgeos.checkpoint.v99","workflow":"build"}`)); err == nil ||
		!strings.Contains(err.Error(), "unsupported checkpoint format") {
		t.Fatalf("decode unknown format error = %v", err)
	}

	path := filepath.Join(t.TempDir(), "checkpoint.json")
	err := Save(path, Checkpoint{FormatVersion: "forgeos.checkpoint.v99"}, 0)
	if err == nil || !strings.Contains(err.Error(), "unsupported checkpoint format") {
		t.Fatalf("Save unknown format error = %v", err)
	}
	if _, statErr := os.Stat(path); !os.IsNotExist(statErr) {
		t.Fatalf("unsupported checkpoint must not be written; stat err=%v", statErr)
	}
}

func TestCheckpointFormatAcceptsLegacyEmptyMarker(t *testing.T) {
	got, err := decode([]byte(`{"workflow":"build","iteration":1}`))
	if err != nil || got.Workflow != "build" {
		t.Fatalf("legacy checkpoint = %+v, %v", got, err)
	}
}

func TestCheckpointV2RequiresEveryExplicitNonNullField(t *testing.T) {
	data, err := encode(currentCheckpoint("evolve", 0))
	if err != nil {
		t.Fatal(err)
	}
	var complete map[string]json.RawMessage
	if err := json.Unmarshal(data, &complete); err != nil {
		t.Fatal(err)
	}
	for _, field := range checkpointV2RequiredFields {
		t.Run("missing_"+field, func(t *testing.T) {
			invalid := cloneRawFields(complete)
			delete(invalid, field)
			assertCurrentCheckpointDecodeFails(t, invalid, "missing required field")
		})
		t.Run("null_"+field, func(t *testing.T) {
			invalid := cloneRawFields(complete)
			invalid[field] = json.RawMessage("null")
			assertCurrentCheckpointDecodeFails(t, invalid, "is null")
		})
	}
}

func TestCheckpointV2AcceptsExplicitZeroAndFalse(t *testing.T) {
	cp := currentCheckpoint("evolve", 0)
	cp.RoadmapCompletion = 0
	cp.GatesGreen = false
	data, err := encode(cp)
	if err != nil {
		t.Fatal(err)
	}
	got, err := decode(data)
	if err != nil {
		t.Fatal(err)
	}
	if got.Iteration != 0 || got.RoadmapCompletion != 0 || got.GatesGreen {
		t.Fatalf("explicit zero/false checkpoint changed: %+v", got)
	}
}

func TestCheckpointV2RejectsNullOptionalScalarsButAllowsOmission(t *testing.T) {
	data, err := encode(currentCheckpoint("evolve", 0))
	if err != nil {
		t.Fatal(err)
	}
	var complete map[string]json.RawMessage
	if err := json.Unmarshal(data, &complete); err != nil {
		t.Fatal(err)
	}
	for _, field := range checkpointV2OptionalScalarFields {
		t.Run(field, func(t *testing.T) {
			invalid := cloneRawFields(complete)
			invalid[field] = json.RawMessage("null")
			assertCurrentCheckpointDecodeFails(t, invalid, "optional scalar")
		})
	}
	if _, err := decode(data); err != nil {
		t.Fatalf("omitted optional scalars must remain valid: %v", err)
	}
}

func TestSaveRejectsIncompleteV2BeforeWriting(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	cp := currentCheckpoint("evolve", 1)
	cp.WorkflowDigest = ""
	err := Save(path, cp, 0)
	if err == nil || !strings.Contains(err.Error(), "workflow_digest") {
		t.Fatalf("Save incomplete v2 error = %v", err)
	}
	if _, statErr := os.Stat(path); !os.IsNotExist(statErr) {
		t.Fatalf("incomplete v2 must not be written; stat err=%v", statErr)
	}
}

func cloneRawFields(src map[string]json.RawMessage) map[string]json.RawMessage {
	dst := make(map[string]json.RawMessage, len(src))
	for key, value := range src {
		dst[key] = append(json.RawMessage(nil), value...)
	}
	return dst
}

func assertCurrentCheckpointDecodeFails(t *testing.T, fields map[string]json.RawMessage, want string) {
	t.Helper()
	data, err := json.Marshal(fields)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := decode(data); err == nil || !strings.Contains(err.Error(), want) {
		t.Fatalf("decode v2 error = %v, want %q", err, want)
	}
}
