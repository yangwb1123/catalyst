// Package persist — replay tests from real fixture data
// (seventh-wave-data-realism.md §方向5: 真实数据回放).
//
// These tests load actual trace.jsonl, memory.jsonl, and checkpoint.json files
// produced by a real evolve dry-run and verify that the current code parses
// them correctly. This catches regressions that synthetic unit tests miss:
// real data has longer traces, mixed event kinds, various confidence values,
// and edge cases like superseded entries and sequential JSONL writes.
//
// Fixtures live in testdata/replay/<name>/ and are checked into the repo so
// they stay stable across commits. Adding a new fixture is as simple as:
//
//	mkdir -p testdata/replay/my-run/
//	cp .forge/trace.jsonl     testdata/replay/my-run/
//	cp .forge/memory.jsonl    testdata/replay/my-run/
//	cp .forge/checkpoint.json testdata/replay/my-run/
package persist

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

// replayDir is the fixture directory relative to this test file.
const replayDir = "testdata/replay"

// readFixture reads the named file from a replay fixture directory.
func readFixture(fixture, filename string) []byte {
	data, err := os.ReadFile(filepath.Join(replayDir, fixture, filename))
	if err != nil {
		return nil
	}
	return data
}

// ── Trace replay ───────────────────────────────────────────────────────────

// traceEvent is the minimal on-disk event shape for fixture parsing.
// It mirrors trace.Event but lives in the persist package so importing
// trace is unnecessary for fixture verification.
type traceEvent struct {
	Format string `json:"_format,omitempty"`
	Seq    int    `json:"seq"`
	Kind   string `json:"kind"`
	Name   string `json:"name"`
	Status string `json:"status"`
	Detail string `json:"detail,omitempty"`
}

// decodeTrace parses JSONL trace data into events. Mirrors the production
// decode path in internal/trace without importing that package.
func decodeTrace(data []byte) ([]traceEvent, error) {
	var events []traceEvent
	for len(data) > 0 {
		idx := indexByte(data, '\n')
		if idx < 0 {
			break
		}
		line := trimSpace(data[:idx])
		data = data[idx+1:]
		if len(line) == 0 {
			continue
		}
		var ev traceEvent
		if err := json.Unmarshal(line, &ev); err != nil {
			return nil, err
		}
		events = append(events, ev)
	}
	return events, nil
}

func TestReplay_TraceParsing(t *testing.T) {
	fixtures := []string{"evolve-dry-run"}
	for _, name := range fixtures {
		t.Run(name, func(t *testing.T) {
			data := readFixture(name, "trace.jsonl")
			if data == nil {
				t.Skipf("fixture %s/trace.jsonl not found", name)
			}
			events, err := decodeTrace(data)
			if err != nil {
				t.Fatalf("decode trace: %v", err)
			}
			if len(events) == 0 {
				t.Fatalf("got 0 events from %s/trace.jsonl", name)
			}
			seenKinds := make(map[string]int)
			var maxSeq int
			for _, ev := range events {
				seenKinds[ev.Kind]++
				if ev.Seq > maxSeq {
					maxSeq = ev.Seq
				}
			}
			t.Logf("%s: %d events, kinds=%v, seqs=1..%d", name, len(events), seenKinds, maxSeq)
			if len(seenKinds) < 3 {
				t.Errorf("only %d event kind(s) in replay data, want ≥3; kinds=%v", len(seenKinds), seenKinds)
			}
			for i, ev := range events {
				if ev.Kind == "" {
					t.Errorf("event %d (seq=%d) has empty Kind", i, ev.Seq)
				}
			}
		})
	}
}

func TestReplay_TraceSeqIsMonotonic(t *testing.T) {
	data := readFixture("evolve-dry-run", "trace.jsonl")
	if data == nil {
		t.Skip("fixture evolve-dry-run/trace.jsonl not found")
	}
	events, err := decodeTrace(data)
	if err != nil {
		t.Fatalf("decode trace: %v", err)
	}
	for i := 1; i < len(events); i++ {
		if events[i].Seq <= events[i-1].Seq {
			t.Errorf("seq non-monotonic at index %d: seq %d ≤ %d", i, events[i].Seq, events[i-1].Seq)
		}
	}
}

// ── Memory replay ──────────────────────────────────────────────────────────

// memoryEntry is the minimal on-disk entry shape for fixture parsing.
// Mirrors memory.Entry.
type memoryEntry struct {
	Format        string  `json:"_format,omitempty"`
	Kind          string  `json:"kind"`
	Topic         string  `json:"topic"`
	Detail        string  `json:"detail"`
	Iteration     int     `json:"iteration"`
	Confidence    float64 `json:"confidence,omitempty"`
	Supersedes    string  `json:"supersedes,omitempty"`
	CreatedAtUnix int64   `json:"created_at_unix"`
}

// decodeMemory parses JSONL memory data into entries. Mirrors the production
// decode path in internal/memory (including the confidence default and
// superseded filtering).
func decodeMemory(data []byte) ([]memoryEntry, error) {
	var entries []memoryEntry
	for len(data) > 0 {
		idx := indexByte(data, '\n')
		if idx < 0 {
			break
		}
		line := trimSpace(data[:idx])
		data = data[idx+1:]
		if len(line) == 0 {
			continue
		}
		var e memoryEntry
		if err := json.Unmarshal(line, &e); err != nil {
			return nil, err
		}
		if e.Confidence == 0 {
			e.Confidence = 1.0
		}
		entries = append(entries, e)
	}
	return filterSupersededMemory(entries), nil
}

func filterSupersededMemory(entries []memoryEntry) []memoryEntry {
	if len(entries) == 0 {
		return entries
	}
	active := make(map[string]int)
	superseded := make(map[string]bool)
	for i := len(entries) - 1; i >= 0; i-- {
		e := entries[i]
		if e.Supersedes != "" {
			if _, exists := active[e.Supersedes]; !exists {
				active[e.Supersedes] = i
				superseded[e.Supersedes] = true
			}
		}
	}
	var out []memoryEntry
	for i, e := range entries {
		if superseded[e.Topic] {
			if keeperIdx, ok := active[e.Topic]; ok && keeperIdx == i {
				out = append(out, e)
			}
			continue
		}
		out = append(out, e)
	}
	return out
}

func TestReplay_MemoryParsing(t *testing.T) {
	fixtures := []string{"evolve-dry-run"}
	for _, name := range fixtures {
		t.Run(name, func(t *testing.T) {
			data := readFixture(name, "memory.jsonl")
			if data == nil {
				t.Skipf("fixture %s/memory.jsonl not found", name)
			}
			entries, err := decodeMemory(data)
			if err != nil {
				t.Fatalf("decode memory: %v", err)
			}
			if len(entries) == 0 {
				t.Fatalf("got 0 entries from %s/memory.jsonl", name)
			}
			seenKinds := make(map[string]int)
			var maxIter int
			for _, e := range entries {
				seenKinds[e.Kind]++
				if e.Iteration > maxIter {
					maxIter = e.Iteration
				}
			}
			t.Logf("%s: %d entries, kinds=%v, max_iter=%d", name, len(entries), seenKinds, maxIter)
			if len(seenKinds) < 3 {
				t.Errorf("only %d entry kind(s) in replay data, want 3; kinds=%v", len(seenKinds), seenKinds)
			}
			for _, e := range entries {
				if e.Confidence <= 0 || e.Confidence > 1.0 {
					t.Errorf("entry kind=%s topic=%s has confidence=%f, want in (0, 1.0]", e.Kind, e.Topic, e.Confidence)
				}
				if e.Topic == "" {
					t.Errorf("entry kind=%s has empty Topic", e.Kind)
				}
			}
		})
	}
}

func TestReplay_MemorySuperseded(t *testing.T) {
	data := readFixture("evolve-dry-run", "memory.jsonl")
	if data == nil {
		t.Skip("fixture evolve-dry-run/memory.jsonl not found")
	}
	entries, err := decodeMemory(data)
	if err != nil {
		t.Fatalf("decode memory: %v", err)
	}
	// The fixture has one superseding entry (PostgreSQL supersedes db-choice).
	// Ensure the superseded entry (Topic="db-choice") has been filtered out.
	for _, e := range entries {
		if e.Topic == "db-choice" {
			t.Errorf("superseded entry with topic %q should have been filtered out: %+v", e.Topic, e)
		}
	}
}

// ── Checkpoint replay ──────────────────────────────────────────────────────

func TestReplay_CheckpointLoads(t *testing.T) {
	fixtures := []string{"evolve-dry-run"}
	for _, name := range fixtures {
		t.Run(name, func(t *testing.T) {
			path := filepath.Join(replayDir, name, "checkpoint.json")
			cp, found, err := Load(path)
			if err != nil {
				t.Fatalf("Load checkpoint from %s: %v", path, err)
			}
			if !found {
				t.Fatalf("checkpoint not found at %s", path)
			}
			if cp.Workflow == "" || cp.Iteration == 0 {
				t.Errorf("checkpoint missing required fields: %+v", cp)
			}
			t.Logf("%s: workflow=%q iteration=%d roadmap=%.0f%% gates=%v mode=%q",
				name, cp.Workflow, cp.Iteration, cp.RoadmapCompletion*100, cp.GatesGreen, cp.Mode)
		})
	}
}

// ── Multi-fixture walk ─────────────────────────────────────────────────────

func TestReplay_MultipleFixtures(t *testing.T) {
	entries, err := os.ReadDir(replayDir)
	if err != nil {
		t.Skipf("replay directory %s not found: %v", replayDir, err)
	}
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		name := entry.Name()
		t.Run(name, func(t *testing.T) {
			if traceData := readFixture(name, "trace.jsonl"); traceData != nil {
				events, err := decodeTrace(traceData)
				if err != nil {
					t.Errorf("%s/trace.jsonl: %v", name, err)
				} else if len(events) == 0 {
					t.Errorf("%s/trace.jsonl: 0 events", name)
				}
			}
			if memData := readFixture(name, "memory.jsonl"); memData != nil {
				entries, err := decodeMemory(memData)
				if err != nil {
					t.Errorf("%s/memory.jsonl: %v", name, err)
				} else if len(entries) == 0 {
					t.Errorf("%s/memory.jsonl: 0 entries", name)
				}
			}
			cpPath := filepath.Join(replayDir, name, "checkpoint.json")
			if _, err := os.Stat(cpPath); err == nil {
				cp, found, err := Load(cpPath)
				if err != nil {
					t.Errorf("%s/checkpoint.json: %v", name, err)
				} else if !found {
					t.Errorf("%s/checkpoint.json: not found", name)
				} else if cp.Workflow == "" {
					t.Errorf("%s/checkpoint.json: missing workflow field", name)
				}
			}
		})
	}
}

// ── Low-level helpers ──────────────────────────────────────────────────────

func indexByte(data []byte, b byte) int {
	for i, c := range data {
		if c == b {
			return i
		}
	}
	return -1
}

func trimSpace(data []byte) []byte {
	start, end := 0, len(data)
	for start < end && (data[start] == ' ' || data[start] == '\t' || data[start] == '\r') {
		start++
	}
	for end > start && (data[end-1] == ' ' || data[end-1] == '\t' || data[end-1] == '\r') {
		end--
	}
	return data[start:end]
}
