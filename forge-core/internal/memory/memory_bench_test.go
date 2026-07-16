// Package memory_test includes benchmarks for memory subsystem key paths.
package memory

import (
	"path/filepath"
	"testing"
)

// BenchmarkLoad benchmarks memory.Load with N entries, simulating a store
// that has accumulated real-world knowledge (seventh-wave-data-realism.md).
func BenchmarkLoad(b *testing.B) {
	dir := b.TempDir()
	path := filepath.Join(dir, "bench.jsonl")

	// Pre-populate with N entries like a real evolve loop would accumulate.
	for i := 0; i < 500; i++ {
		e := Entry{
			Kind:       KindLesson,
			Topic:      "auth",
			Detail:     "iter: some detailed lesson learned from the evolve loop about authentication patterns",
			Iteration:  i + 1,
			Confidence: 1.0,
			Format:     "forgeos.memory.v1",
		}
		if i%3 == 0 {
			e.Kind = KindGap
			e.Topic = "cache"
		} else if i%5 == 0 {
			e.Kind = KindDecision
			e.Topic = "db"
		}
		if err := Append(path, e); err != nil {
			b.Fatalf("seed append #%d: %v", i, err)
		}
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, err := Load(path)
		if err != nil {
			b.Fatalf("Load: %v", err)
		}
	}
}
