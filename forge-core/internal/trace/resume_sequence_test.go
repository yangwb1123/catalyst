package trace

import (
	"bytes"
	"encoding/json"
	"math"
	"sync"
	"testing"
)

func TestResumeAfterContinuesWithoutRegression(t *testing.T) {
	var output bytes.Buffer
	tracer := NewTracer(&output)
	if err := tracer.ResumeAfter(8); err != nil {
		t.Fatal(err)
	}
	mustEmitSequenceEvent(t, tracer)
	if err := tracer.ResumeAfter(3); err == nil {
		t.Fatal("late resume must fail")
	}
	mustEmitSequenceEvent(t, tracer)
	if got := decodedSequences(t, output.Bytes()); !equalSequences(got, []int{9, 10}) {
		t.Fatalf("sequences = %v, want [9 10]", got)
	}
}

func TestResumeAfterRejectsNegativeWithoutMutation(t *testing.T) {
	var output bytes.Buffer
	tracer := NewTracer(&output)
	if err := tracer.ResumeAfter(-1); err == nil {
		t.Fatal("negative sequence must fail")
	}
	mustEmitSequenceEvent(t, tracer)
	if got := decodedSequences(t, output.Bytes()); !equalSequences(got, []int{1}) {
		t.Fatalf("sequences = %v, want [1]", got)
	}
}

func TestResumeAfterRejectsExhaustedSequence(t *testing.T) {
	var output bytes.Buffer
	tracer := NewTracer(&output)
	if err := tracer.ResumeAfter(math.MaxInt); err == nil {
		t.Fatal("max int floor must fail before emit")
	}
	mustEmitSequenceEvent(t, tracer)
	if got := decodedSequences(t, output.Bytes()); !equalSequences(got, []int{1}) {
		t.Fatalf("sequences = %v, want [1]", got)
	}
	exhausted := NewTracer(&bytes.Buffer{})
	exhausted.seq = math.MaxInt
	if err := exhausted.Emit(Event{Kind: "iteration"}); err == nil {
		t.Fatal("emit at max int must fail instead of overflowing")
	}
	if exhausted.seq != math.MaxInt {
		t.Fatalf("exhausted sequence mutated to %d", exhausted.seq)
	}
}

func TestResumeAfterSerializesConcurrentFloorsBeforeEmit(t *testing.T) {
	var output bytes.Buffer
	tracer := NewTracer(&output)
	var wait sync.WaitGroup
	for i := 0; i < 32; i++ {
		wait.Add(1)
		go func(floor int) {
			defer wait.Done()
			if err := tracer.ResumeAfter(floor); err != nil {
				t.Errorf("ResumeAfter(%d): %v", floor, err)
			}
		}(i)
	}
	wait.Wait()
	mustEmitSequenceEvent(t, tracer)
	if got := decodedSequences(t, output.Bytes()); !equalSequences(got, []int{32}) {
		t.Fatalf("sequences = %v, want [32]", got)
	}
}

func mustEmitSequenceEvent(t *testing.T, tracer *Tracer) {
	t.Helper()
	if err := tracer.Emit(Event{Kind: "iteration", Status: "ok"}); err != nil {
		t.Fatal(err)
	}
}

func decodedSequences(t *testing.T, data []byte) []int {
	t.Helper()
	lines := bytes.Split(bytes.TrimSpace(data), []byte("\n"))
	sequences := make([]int, 0, len(lines))
	for _, line := range lines {
		var event Event
		if err := json.Unmarshal(line, &event); err != nil {
			t.Fatalf("decode trace event: %v", err)
		}
		sequences = append(sequences, event.Seq)
	}
	return sequences
}

func equalSequences(left, right []int) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
