// Package asset includes benchmarks for the workflow asset loading critical path
// (fifth-wave-operational.md §缺口2: 零性能基准测试).
package asset

import (
	"os"
	"path/filepath"
	"testing"
)

// BenchmarkLoadWorkflowJSON benchmarks parsing a workflow JSON document, which
// is the startup path for every `forge run` and `forge evolve`.
func BenchmarkLoadWorkflowJSON(b *testing.B) {
	// Use the real build.yml workflow data for realistic benchmarking.
	// The JSON was produced by the yaml2json shim from the actual build.yml.
	data := []byte(`{
  "stage": "build",
  "phases": [
    {"name": "planner", "agent": "planner", "required_gates": [], "feeds_forward": true},
    {"name": "implementer", "agent": "implementer", "required_gates": [], "model_tier": "sonnet", "feeds_forward": true, "emits": ["task-plan.md", "proposal.md"], "fresh_context": false},
    {"name": "harness-gates", "agent": "implementer", "required_gates": ["lint", "test", "build", "complexity"], "on_fail": {"action": "loop_back", "target_phase": "implementer"}},
    {"name": "reviewer", "agent": "reviewer", "required_gates": [], "fresh_context": true, "required_when": "../policies/modes.yml#workflow_depth.reviewer"},
    {"name": "qa", "agent": "qa", "verdict_contract": "qa_v1", "required_gates": ["test", "build"], "on_fail": {"action": "loop_back", "target_phase": "implementer"}}
  ],
  "stop_condition": {
    "type": "conjunction",
    "all_of": [
      {"metric": "roadmap_completion", "operator": "==", "threshold": 100},
      {"metric": "gates_status", "value": "green"}
    ],
    "anti_pattern": "round_count",
    "on_unmet": {"action": "loop_to_next_roadmap_item", "target_phase": "planner"}
  }
}`)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, err := LoadWorkflowJSON(data)
		if err != nil {
			b.Fatalf("LoadWorkflowJSON: %v", err)
		}
	}
}

// BenchmarkLoadWorkflowRealData uses a real on-disk workflow file if available.
func BenchmarkLoadWorkflowRealData(b *testing.B) {
	// Try to find a real workflow JSON file in the repo.
	candidates := []string{
		"../cmd/forge/testdata/build.json",
		"../../cmd/forge/testdata/build.json",
	}
	var data []byte
	for _, path := range candidates {
		if d, err := os.ReadFile(filepath.Clean(path)); err == nil {
			data = d
			break
		}
	}
	if data == nil {
		// Fall back to the embedded benchmark data.
		b.Skip("no real workflow JSON available")
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, err := LoadWorkflowJSON(data)
		if err != nil {
			b.Fatalf("LoadWorkflowJSON: %v", err)
		}
	}
}
