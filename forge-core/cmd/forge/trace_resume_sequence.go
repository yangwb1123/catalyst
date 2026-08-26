package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"math"
	"os"
	"path/filepath"

	"forgeos/forge-core/internal/statefs"
	"forgeos/forge-core/internal/trace"
)

const resumeTraceLineMaxBytes = 64 << 20

type traceResume struct {
	runID    string
	sequence int
	bound    bool
}

// openTracerForRun inspects resume history before openTracer can rotate and
// replace trace.jsonl.1, then installs the recovered floor before any Emit.
func openTracerForRun(root, runID string) (*trace.Tracer, func(), error) {
	resume, err := inspectTraceResume(root, runID)
	if err != nil {
		return nil, nil, err
	}
	tracer, closeTrace, err := openTracer(root)
	if err != nil {
		return nil, nil, err
	}
	if err := resume.apply(tracer); err != nil {
		closeTrace()
		return nil, nil, err
	}
	return tracer, closeTrace, nil
}

func inspectTraceResume(root, runID string) (traceResume, error) {
	if runID == "" || runID == "run_id_not_bound" {
		return traceResume{}, nil
	}
	sequence, err := persistedTraceSequence(root, runID)
	if err != nil {
		return traceResume{}, err
	}
	return traceResume{runID: runID, sequence: sequence, bound: true}, nil
}

func (resume traceResume) apply(tracer *trace.Tracer) error {
	if !resume.bound {
		return nil
	}
	if err := tracer.ResumeAfter(resume.sequence); err != nil {
		return fmt.Errorf("resume trace sequence: %w", err)
	}
	tracer.RunID = resume.runID
	return nil
}

// resumeTraceSequence is the focused scanner entry point used by tests.
// Production calls openTracerForRun so scanning precedes rotate.
func resumeTraceSequence(root string, tracer *trace.Tracer) error {
	if tracer == nil {
		return fmt.Errorf("resume trace sequence: tracer is nil")
	}
	resume, err := inspectTraceResume(root, tracer.RunID)
	if err != nil {
		return err
	}
	if !resume.bound {
		return fmt.Errorf("resume trace sequence: run_id is empty")
	}
	return resume.apply(tracer)
}

func persistedTraceSequence(root, runID string) (int, error) {
	maximum := 0
	for _, name := range []string{"trace.jsonl", "trace.jsonl.1"} {
		path := filepath.Join(forgeDir(root), name)
		sequence, present, err := traceSequenceFile(path, runID)
		if err != nil {
			return 0, fmt.Errorf("resume trace sequence: read %s: %w", name, err)
		}
		if present {
			maximum = greaterSequence(maximum, sequence)
		}
	}
	return maximum, nil
}

func traceSequenceFile(path, runID string) (int, bool, error) {
	before, present, err := statefs.InspectRegular(path)
	if err != nil || !present {
		return 0, present, err
	}
	file, err := statefs.OpenRegularReadOnly(path)
	if err != nil {
		return 0, true, err
	}
	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 64*1024), resumeTraceLineMaxBytes)
	sequence, scanErr := traceSequenceForRun(scanner, runID)
	closeErr := file.Close()
	if scanErr != nil {
		return 0, true, scanErr
	}
	if closeErr != nil {
		return 0, true, closeErr
	}
	after, stillPresent, err := statefs.InspectRegular(path)
	if err != nil || !stillPresent || !os.SameFile(before, after) ||
		before.Size() != after.Size() || !before.ModTime().Equal(after.ModTime()) {
		return 0, true, fmt.Errorf("statefs: %s changed while reading", path)
	}
	return sequence, true, nil
}

func validateTraceAppendFraming(path string, expectedSize int64) error {
	if expectedSize == 0 {
		return nil
	}
	file, err := statefs.OpenRegularReadOnly(path)
	if err != nil {
		return err
	}
	before, statErr := file.Stat()
	if statErr != nil || before.Size() != expectedSize {
		_ = file.Close()
		return fmt.Errorf("trace file changed before framing check")
	}
	if _, err := file.Seek(-1, io.SeekEnd); err != nil {
		_ = file.Close()
		return err
	}
	var final [1]byte
	_, readErr := io.ReadFull(file, final[:])
	closeErr := file.Close()
	if readErr != nil {
		return readErr
	}
	if closeErr != nil {
		return closeErr
	}
	after, present, err := statefs.InspectRegular(path)
	if err != nil || !present || !os.SameFile(before, after) ||
		before.Size() != after.Size() || !before.ModTime().Equal(after.ModTime()) {
		return fmt.Errorf("trace file changed during framing check")
	}
	if final[0] != '\n' {
		return fmt.Errorf("trace file ends with an incomplete record")
	}
	return nil
}

func traceSequenceForRun(scanner *bufio.Scanner, runID string) (int, error) {
	maximum := 0
	for scanner.Scan() {
		var identity struct {
			RunID string `json:"run_id"`
		}
		if json.Unmarshal(scanner.Bytes(), &identity) != nil || identity.RunID != runID {
			continue
		}
		var record struct {
			Format string          `json:"_format"`
			Seq    json.RawMessage `json:"seq"`
		}
		if err := json.Unmarshal(scanner.Bytes(), &record); err != nil {
			return 0, fmt.Errorf("invalid trace record for run %q: %w", runID, err)
		}
		if err := trace.ValidateFormat(record.Format); err != nil {
			return 0, err
		}
		sequence, err := boundedTraceSequence(record.Seq)
		if err != nil {
			return 0, err
		}
		maximum = greaterSequence(maximum, sequence)
	}
	if err := scanner.Err(); err != nil {
		return 0, err
	}
	return maximum, nil
}

func boundedTraceSequence(raw json.RawMessage) (int, error) {
	var number json.Number
	if len(raw) == 0 || json.Unmarshal(raw, &number) != nil {
		return 0, fmt.Errorf("trace sequence is missing or not an integer")
	}
	value, err := number.Int64()
	if err != nil || value < int64(math.MinInt) || value > int64(math.MaxInt) {
		return 0, fmt.Errorf("trace sequence %q is outside the supported integer range", number)
	}
	if value == int64(math.MaxInt) {
		return 0, fmt.Errorf("sequence %d leaves no next sequence", value)
	}
	if value <= 0 {
		return 0, fmt.Errorf("trace sequence must be positive, got %d", value)
	}
	return int(value), nil
}

func greaterSequence(left, right int) int {
	if right > left {
		return right
	}
	return left
}
