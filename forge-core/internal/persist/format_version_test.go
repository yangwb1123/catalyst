package persist

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"forgeos/forge-core/internal/materiality"
)

func TestCheckpointFormatCurrentIsV5(t *testing.T) {
	if CheckpointFormatCurrent != "forgeos.checkpoint.v5" {
		t.Fatalf("CheckpointFormatCurrent = %q, want v5", CheckpointFormatCurrent)
	}
}

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

func TestCheckpointFormatAcceptsV3WithoutMaterialityForDiagnosticsOnly(t *testing.T) {
	got, err := decode([]byte(`{
		"_format":"forgeos.checkpoint.v3",
		"workflow":"evolve",
		"workflow_digest":"old-digest",
		"mode":"balanced",
		"lifecycle":"mvp",
		"iteration":2,
		"roadmap_completion":0.5,
		"gates_green":true,
		"reason":"old checkpoint",
		"updated_at_unix":1750000000,
		"phase_index":1,
		"budget_cap_micros":0,
		"spent_usd_micros":0,
		"max_agent_calls":0,
		"agent_calls":0,
		"max_loop_backs":0,
		"loop_backs":0
	}`))
	if err != nil {
		t.Fatalf("decode diagnostic v3 checkpoint: %v", err)
	}
	if got.FormatVersion != checkpointFormatV3 || got.Materiality != "" {
		t.Fatalf("diagnostic v3 checkpoint changed: %+v", got)
	}
	if got.FormatVersion == CheckpointFormatCurrent {
		t.Fatal("v3 checkpoint must not be considered resumable/current")
	}
}

func TestCheckpointV4RequiresEveryExplicitNonNullField(t *testing.T) {
	data, err := encode(currentCheckpoint("evolve", 0))
	if err != nil {
		t.Fatal(err)
	}
	var complete map[string]json.RawMessage
	if err := json.Unmarshal(data, &complete); err != nil {
		t.Fatal(err)
	}
	for _, field := range checkpointV5RequiredFields {
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

func TestCheckpointV4AcceptsEveryMaterialityValue(t *testing.T) {
	for _, value := range []string{materiality.Unbound, "L0", "L1", "L2", "L3", "L4"} {
		t.Run(value, func(t *testing.T) {
			cp := currentCheckpoint("evolve", 0)
			cp.Materiality = value
			data, err := encode(cp)
			if err != nil {
				t.Fatal(err)
			}
			got, err := decode(data)
			if err != nil {
				t.Fatalf("decode materiality %q: %v", value, err)
			}
			if got.Materiality != value {
				t.Fatalf("Materiality = %q, want %q", got.Materiality, value)
			}
		})
	}
}

func TestCheckpointV4RejectsInvalidMateriality(t *testing.T) {
	for _, value := range []string{"", "l3", " L3", "L5", "high"} {
		t.Run(value, func(t *testing.T) {
			cp := currentCheckpoint("evolve", 0)
			cp.Materiality = value
			data, err := encode(cp)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := decode(data); err == nil || !strings.Contains(err.Error(), "materiality") {
				t.Fatalf("decode invalid materiality %q error = %v", value, err)
			}
		})
	}
}

func TestCheckpointV4RejectsDuplicateRecoveryFields(t *testing.T) {
	data, err := encode(currentCheckpoint("evolve", 2))
	if err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct{ name, prefix string }{
		{"materiality", `{"materiality":"L4",`},
		{"mode", `{"mode":"engineering",`},
		{"iteration", `{"iteration":9,`},
		{"phase_cursor", `{"phase_index":3,`},
		{"workflow_digest", `{"workflow_digest":"other",`},
	} {
		t.Run(tc.name, func(t *testing.T) {
			mutated := append([]byte(tc.prefix), data[1:]...)
			if _, err := decode(mutated); err == nil || !strings.Contains(err.Error(), "duplicate field") {
				t.Fatalf("decode duplicate %s error = %v", tc.name, err)
			}
		})
	}
}

func TestCheckpointV5RejectsNestedDuplicateAndUnsortedRecoveryKeys(t *testing.T) {
	data, err := encode(currentCheckpoint("evolve", 0))
	if err != nil {
		t.Fatal(err)
	}
	for _, replacement := range []string{
		`"phase_output_receipts":{"evolve/a":"` + strings.Repeat("a", 64) +
			`","evolve/a":"` + strings.Repeat("b", 64) + `"}`,
		`"phase_semantic_outputs":{"evolve/z":"z","evolve/a":"a"}`,
	} {
		mutated := strings.Replace(string(data), `"phase_output_receipts": {}`, replacement, 1)
		if strings.Contains(replacement, "phase_semantic_outputs") {
			mutated = strings.Replace(string(data), `"phase_semantic_outputs": {}`, replacement, 1)
		}
		if _, err := decode([]byte(mutated)); err == nil ||
			!strings.Contains(err.Error(), "sorted and unique") {
			t.Fatalf("nested recovery map mutation error = %v", err)
		}
	}
}

func TestCheckpointV4AcceptsExplicitZeroAndFalse(t *testing.T) {
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

func TestCheckpointV4RejectsNullOptionalScalarsButAllowsOmission(t *testing.T) {
	data, err := encode(currentCheckpoint("evolve", 0))
	if err != nil {
		t.Fatal(err)
	}
	var complete map[string]json.RawMessage
	if err := json.Unmarshal(data, &complete); err != nil {
		t.Fatal(err)
	}
	for _, field := range checkpointV5OptionalScalarFields {
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

func TestCheckpointV4ResourceEnvelopeRoundTrips(t *testing.T) {
	want := currentCheckpoint("evolve", 3)
	want.PhaseIndex = 4
	want.BudgetCapMicros = 9_000_000
	want.SpentUsdMicros = 2_750_000
	want.MaxAgentCalls = 12
	want.AgentCalls = 5
	want.MaxLoopBacks = 3
	want.LoopBacks = 2
	want.EvolveScanReport = `EVOLVE_SCAN_V1: {"version":"evolve_scan_v1"}`
	want.EvolveScanSemanticOutput = want.EvolveScanReport
	want.PhaseSemanticOutputs["evolve/scan"] = want.EvolveScanReport

	data, err := encode(want)
	if err != nil {
		t.Fatal(err)
	}
	got, err := decode(data)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("v4 resource envelope changed:\n got  %+v\n want %+v", got, want)
	}
}

func TestCheckpointV4ZeroAgentCallMaxIsUnbounded(t *testing.T) {
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

func TestCheckpointV4RejectsInvalidResourceEnvelope(t *testing.T) {
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

func TestCheckpointV4AllowsConsumedProgressBeforeFirstPhaseCompletes(t *testing.T) {
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

func TestSaveRejectsIncompleteV4BeforeWriting(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	cp := currentCheckpoint("evolve", 1)
	cp.WorkflowDigest = ""
	err := Save(path, cp, 0)
	if err == nil || !strings.Contains(err.Error(), "workflow_digest") {
		t.Fatalf("Save incomplete v4 error = %v", err)
	}
	if _, statErr := os.Stat(path); !os.IsNotExist(statErr) {
		t.Fatalf("incomplete v4 must not be written; stat err=%v", statErr)
	}
}

func TestSaveRejectsInvalidV4MaterialityBeforeWriting(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	cp := currentCheckpoint("evolve", 1)
	cp.Materiality = "l3"
	err := Save(path, cp, 0)
	if err == nil || !strings.Contains(err.Error(), "materiality") {
		t.Fatalf("Save invalid materiality error = %v", err)
	}
	if _, statErr := os.Stat(path); !os.IsNotExist(statErr) {
		t.Fatalf("invalid materiality checkpoint must not be written; stat err=%v", statErr)
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
		t.Fatalf("decode current checkpoint error = %v, want %q", err, want)
	}
}
