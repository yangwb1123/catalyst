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

func TestCheckpointFormatAcceptsV2ForDiagnostics(t *testing.T) {
	got, err := decode([]byte(`{
		"_format":"forgeos.checkpoint.v2",
		"workflow":"evolve",
		"phase_index":2,
		"spent_usd_micros":125000
	}`))
	if err != nil {
		t.Fatalf("decode diagnostic v2 checkpoint: %v", err)
	}
	if got.FormatVersion != checkpointFormatV2 || got.Workflow != "evolve" ||
		got.PhaseIndex != 2 || got.SpentUsdMicros != 125000 {
		t.Fatalf("diagnostic v2 checkpoint changed: %+v", got)
	}
}

func TestCheckpointV3RequiresEveryExplicitNonNullField(t *testing.T) {
	data, err := encode(currentCheckpoint("evolve", 0))
	if err != nil {
		t.Fatal(err)
	}
	var complete map[string]json.RawMessage
	if err := json.Unmarshal(data, &complete); err != nil {
		t.Fatal(err)
	}
	for _, field := range checkpointV3RequiredFields {
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

func TestCheckpointV3AcceptsExplicitZeroAndFalse(t *testing.T) {
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
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		t.Fatal(err)
	}
	for _, field := range []string{
		"phase_index", "budget_cap_micros", "spent_usd_micros",
		"max_agent_calls", "agent_calls", "max_loop_backs", "loop_backs",
	} {
		if string(fields[field]) != "0" {
			t.Errorf("%s = %s, want explicit JSON zero", field, fields[field])
		}
	}
}

func TestCheckpointV3RejectsNullOptionalScalarsButAllowsOmission(t *testing.T) {
	data, err := encode(currentCheckpoint("evolve", 0))
	if err != nil {
		t.Fatal(err)
	}
	var complete map[string]json.RawMessage
	if err := json.Unmarshal(data, &complete); err != nil {
		t.Fatal(err)
	}
	for _, field := range checkpointV3OptionalScalarFields {
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

func TestCheckpointV3ResourceEnvelopeRoundTrips(t *testing.T) {
	want := currentCheckpoint("evolve", 3)
	want.PhaseIndex = 4
	want.BudgetCapMicros = 9_000_000
	want.SpentUsdMicros = 2_750_000
	want.MaxAgentCalls = 12
	want.AgentCalls = 5
	want.MaxLoopBacks = 3
	want.LoopBacks = 2
	want.EvolveScanReport = `EVOLVE_SCAN_V1: {"version":"evolve_scan_v1"}`

	data, err := encode(want)
	if err != nil {
		t.Fatal(err)
	}
	got, err := decode(data)
	if err != nil {
		t.Fatal(err)
	}
	if got != want {
		t.Fatalf("v3 resource envelope changed:\n got  %+v\n want %+v", got, want)
	}
}

func TestCheckpointV3ZeroAgentCallMaxIsUnbounded(t *testing.T) {
	cp := currentCheckpoint("evolve", 2)
	cp.PhaseIndex = 1
	cp.AgentCalls = 99
	data, err := encode(cp)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := decode(data); err != nil {
		t.Fatalf("zero max_agent_calls must remain unbounded: %v", err)
	}
}

func TestCheckpointV3RejectsInvalidResourceEnvelope(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*Checkpoint)
		want   string
	}{
		{"negative_phase", func(cp *Checkpoint) { cp.PhaseIndex = -1 }, "phase_index"},
		{"negative_budget_cap", func(cp *Checkpoint) { cp.BudgetCapMicros = -1 }, "budget_cap_micros"},
		{"negative_spend", func(cp *Checkpoint) { cp.SpentUsdMicros = -1 }, "spent_usd_micros"},
		{"negative_agent_max", func(cp *Checkpoint) { cp.MaxAgentCalls = -1 }, "max_agent_calls"},
		{"negative_agent_calls", func(cp *Checkpoint) { cp.AgentCalls = -1 }, "agent_calls"},
		{"agent_calls_over_positive_max", func(cp *Checkpoint) {
			cp.MaxAgentCalls, cp.AgentCalls = 2, 3
		}, "exceeds max_agent_calls"},
		{"negative_loop_max", func(cp *Checkpoint) { cp.MaxLoopBacks = -1 }, "max_loop_backs"},
		{"negative_loop_backs", func(cp *Checkpoint) { cp.LoopBacks = -1 }, "loop_backs"},
		{"loop_backs_over_positive_max", func(cp *Checkpoint) {
			cp.MaxLoopBacks, cp.LoopBacks = 1, 2
		}, "exceeds max_loop_backs"},
		{"loop_back_forbidden_at_zero_max", func(cp *Checkpoint) {
			cp.MaxLoopBacks, cp.LoopBacks = 0, 1
		}, "exceeds max_loop_backs"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cp := currentCheckpoint("evolve", 2)
			cp.PhaseIndex = 1
			tt.mutate(&cp)
			data, err := encode(cp)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := decode(data); err == nil || !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("decode invalid envelope error = %v, want %q", err, tt.want)
			}
		})
	}
}

func TestCheckpointV3AllowsConsumedProgressBeforeFirstPhaseCompletes(t *testing.T) {
	cp := currentCheckpoint("evolve", 0)
	cp.PhaseIndex = 0
	cp.MaxAgentCalls, cp.AgentCalls = 4, 1
	cp.MaxLoopBacks = 3
	cp.Reason = "agent spawn reserved (mid-iteration)"
	data, err := encode(cp)
	if err != nil {
		t.Fatal(err)
	}
	got, err := decode(data)
	if err != nil {
		t.Fatalf("phase-zero progress must be resumable: %v", err)
	}
	if got.PhaseIndex != 0 || got.AgentCalls != 1 {
		t.Fatalf("phase-zero progress changed: %+v", got)
	}
}

func TestSaveRejectsIncompleteV3BeforeWriting(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	cp := currentCheckpoint("evolve", 1)
	cp.WorkflowDigest = ""
	err := Save(path, cp, 0)
	if err == nil || !strings.Contains(err.Error(), "workflow_digest") {
		t.Fatalf("Save incomplete v3 error = %v", err)
	}
	if _, statErr := os.Stat(path); !os.IsNotExist(statErr) {
		t.Fatalf("incomplete v3 must not be written; stat err=%v", statErr)
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
		t.Fatalf("decode v3 error = %v, want %q", err, want)
	}
}
