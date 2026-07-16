// Package trace includes benchmarks for the trace event encoding/decoding path
// (fifth-wave-operational.md §缺口2: 零性能基准测试).
package trace

import (
	"bytes"
	"encoding/json"
	"testing"
)

// BenchmarkEncode benchmarks encoding a single trace event.
func BenchmarkEncode(b *testing.B) {
	ev := Event{
		Kind:       "iteration",
		Name:       "42",
		Status:     "ok",
		DurationMs: 45000,
		Detail:     "roadmap=100% gates_green=true",
	}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, err := encode(ev)
		if err != nil {
			b.Fatalf("encode: %v", err)
		}
	}
}

// BenchmarkEmit benchmarks emitting events through the Tracer.
func BenchmarkEmit(b *testing.B) {
	var buf bytes.Buffer
	tr := NewTracer(&buf)

	ev := Event{Kind: "agent", Name: "implementer", Status: "ok", DurationMs: 12000, Detail: "implemented feature X"}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if err := tr.Emit(ev); err != nil {
			b.Fatalf("Emit: %v", err)
		}
	}
}

// BenchmarkJSONMarshal benchmarks the raw JSON marshal step of an event with
// all fields populated, which is the hot path inside encode.
func BenchmarkJSONMarshal(b *testing.B) {
	ev := Event{
		Format:        "forgeos.trace.v1",
		Seq:           100,
		Kind:          "agent",
		Name:          "security-review",
		Status:        "PASS",
		DurationMs:    28000,
		CostUsdMicros: 35000,
		Model:         "opus",
		Detail:        "STRIDE analysis complete: no critical findings, 2 medium recommendations",
	}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, err := json.Marshal(ev)
		if err != nil {
			b.Fatalf("json.Marshal: %v", err)
		}
	}
}
