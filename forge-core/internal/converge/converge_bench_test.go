// Package converge includes benchmarks for the stop-condition evaluation path
// (fifth-wave-operational.md §缺口2: 零性能基准测试).
package converge

import (
	"testing"

	"forgeos/forge-core/internal/asset"
)

// thresholdPtr returns a pointer to f for use in Criteria.Threshold.
func thresholdPtr(f float64) *float64 { return &f }

// BenchmarkEvaluate benchmarks the conjunction evaluator with a full set of
// criteria, matching what a real build.yml stop condition looks like.
func BenchmarkEvaluate(b *testing.B) {
	criteria := []asset.Criterion{
		{Metric: "roadmap_completion", Operator: "==", Threshold: thresholdPtr(100.0)},
		{Metric: "gates_status", Value: "green"},
	}
	sig := Signals{
		RoadmapCompletion: 0.85,
		GatesGreen:        false,
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		Evaluate(criteria, sig)
	}
}

// BenchmarkConverge benchmarks the full Converge dispatch including the
// human_gate branch.
func BenchmarkConverge(b *testing.B) {
	stop := asset.StopCondition{
		Type: "conjunction",
		AllOf: []asset.Criterion{
			{Metric: "roadmap_completion", Operator: "==", Threshold: thresholdPtr(100.0)},
			{Metric: "gates_status", Value: "green"},
		},
	}
	sig := Signals{
		RoadmapCompletion: 0.85,
		GatesGreen:        false,
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		Converge(stop, sig)
	}
}
