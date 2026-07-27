// Package persist gives forge-core's autonomous loop a durable memory.
//
// The loop's recovery state lives in process memory, so a crash mid-run means
// replaying from scratch and discarding already-completed work. This package
// owns the storage layer for that state: a Checkpoint snapshot written after each
// completed iteration AND after each completed agent phase (PhaseIndex), read back
// on startup so a --resume continues where the loop left off — re-entering the
// in-progress iteration at the next unstarted phase rather than replaying it. The
// loop wiring (write-per-iteration/phase via cmd/forge's OnIteration/OnPhase hooks,
// resume-on-start via resumeStart) lives in cmd/forge; this package is the pure
// read/write storage layer with atomic persistence.
//
// Two properties matter most here, and both are about not lying to the caller:
//
//   - Atomic writes. Save never overwrites a good checkpoint in place. It writes
//     a sibling temp file, fsyncs it, then renames it over the target. rename(2)
//     is atomic within a filesystem, so a process crash mid-write leaves either
//     the old intact checkpoint or the new one — never a half-written,
//     unparseable file that would itself corrupt the recovery path.
//
//   - Fault-tolerant load, but honest about corruption. A missing checkpoint is
//     the normal first-run case and is reported as "not found", not an error. A
//     present-but-malformed checkpoint is an explicit error: silently treating
//     corruption as "no checkpoint" would discard real progress without telling
//     anyone (honesty-first), so the caller is forced to see it and decide.
//
// The pure Checkpoint<->JSON conversion (encode/decode) is split from the IO so
// the serialization contract is unit-testable without touching the filesystem;
// Save and Load are thin IO wrappers around it. Pure Go standard library only.
package persist

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/statefs"
)

const (
	checkpointFormatV1 = "forgeos.checkpoint.v1"
	checkpointMaxBytes = 4 << 20
	// CheckpointFormatCurrent is the only generation safe for autonomous resume.
	// Older generations remain Load-readable so doctor/status can diagnose them.
	CheckpointFormatCurrent = "forgeos.checkpoint.v2"
)

type checkpointWriter func(string, []byte, os.FileMode) error

type checkpointSnapshot struct {
	data    []byte
	present bool
}

// Checkpoint is the minimal state needed to resume the autonomous loop after a
// crash or clean stop. It is deliberately small: just enough to re-enter the
// workflow at the right place and re-explain why it last stopped.
//
// FormatVersion is the on-disk format identifier (e.g. "forgeos.checkpoint.v2"),
// so the loader can distinguish between format generations when a future
// breaking change is needed. Empty/v1 state remains readable for diagnostics,
// while cmd/forge refuses to resume anything except the current generation.
//
// UpdatedAtUnix is injected by the caller rather than read from time.Now inside
// this package, so persistence stays a deterministic pure function of its
// inputs — tests can assert exact bytes, and the clock is the caller's concern.
type Checkpoint struct {
	FormatVersion     string  `json:"_format,omitempty"`
	Workflow          string  `json:"workflow"` // workflow asset being run (e.g. "build")
	WorkflowDigest    string  `json:"workflow_digest,omitempty"`
	Mode              string  `json:"mode"`                // execution mode the loop was driving under
	Lifecycle         string  `json:"lifecycle,omitempty"` // resolved lifecycle frozen for this run
	Iteration         int     `json:"iteration"`           // count of iterations already COMPLETED
	RoadmapCompletion float64 `json:"roadmap_completion"`  // last observed completion fraction in [0,1]
	// PhaseIndex is PHASE-granular resume progress WITHIN the in-progress iteration
	// (Iteration+1): the index of the next phase to run. 0 (the default, and what a
	// per-iteration checkpoint resets it to at a clean iteration boundary) means "start
	// the next iteration from phase 0" — byte-for-byte the pre-phase-granular behavior.
	// A value > 0 is written after each agent phase completes mid-iteration, so a crash
	// resumes at that phase instead of replaying every completed (billed) agent phase.
	// omitempty keeps a checkpoint written before this field existed (and any clean
	// iteration-boundary checkpoint) byte-identical on disk and decoding to 0.
	PhaseIndex    int    `json:"phase_index,omitempty"`
	GatesGreen    bool   `json:"gates_green"`     // whether all required gates were green at the snapshot
	Reason        string `json:"reason"`          // why the loop last stopped (for resume context)
	UpdatedAtUnix int64  `json:"updated_at_unix"` // caller-supplied snapshot time (Unix seconds)
	// SpentUsdMicros is the loop's cumulative billed cost so far, in integer
	// MICRO-dollars (USD x 1e6), OPAQUE to this package EXACTLY like
	// trace.Event.CostUsdMicros — persist has no notion of dollars or a "budget"; it
	// only stores and returns this int so a --resume can re-seed the run-level cost
	// cap instead of restarting the tally from zero (the gap: a crash + --resume built
	// a fresh budget at spent=0, so cost already billed before the crash escaped the
	// cap and the run overspent). The micro<->dollar conversion and all budget meaning
	// live in the caller (cmd/forge cost.go), never here. Integer microdollars match
	// CostUsdMicros (jitter-free; 0.054 round-trips exactly). omitempty keeps a run
	// WITHOUT a run budget — and any checkpoint written before this field existed —
	// byte-for-byte identical on disk and decoding to 0, so old checkpoints stay
	// loadable and an unbudgeted run is unchanged.
	SpentUsdMicros int64 `json:"spent_usd_micros,omitempty"`
}

// Save atomically persists cp to path as JSON.
//
// It writes to an unpredictable O_EXCL sibling temp file, fsyncs its bytes, then
// renames it over path. The temp file lives in the same directory as the target,
// so a process crash leaves either the prior checkpoint or the new one whole,
// never a truncated file. Parent directories are created as needed. The
// containing directory is not fsynced, so this is not a power-loss durability
// guarantee for filesystems that require a separate directory sync.
//
// When retain > 0, up to retain historical checkpoints are preserved by copying
// a validated snapshot of the prior checkpoint set before committing current.
// History publication never moves current. A failed history or current commit
// triggers an exact snapshot restore; if that restore also fails, Save reports
// both errors rather than claiming a clean rollback. retain=0 (the default)
// keeps the legacy single-file behavior.
//
// FormatVersion is set on save so all new checkpoints carry the current marker;
// old files without the field remain diagnostic-readable with an empty marker.
func Save(path string, cp Checkpoint, retain int) error {
	return saveWithWriter(path, cp, retain, statefs.AtomicWrite)
}

func saveWithWriter(path string, cp Checkpoint, retain int, write checkpointWriter) error {
	if cp.FormatVersion == "" {
		cp.FormatVersion = CheckpointFormatCurrent
	}
	if err := validateFormat(cp.FormatVersion); err != nil {
		return err
	}
	if cp.FormatVersion == CheckpointFormatCurrent {
		if err := validateCurrentCheckpoint(cp); err != nil {
			return err
		}
	}
	data, err := encode(cp)
	if err != nil {
		return err
	}
	if cp.FormatVersion == CheckpointFormatCurrent {
		if err := validateCurrentEncoding(data); err != nil {
			return err
		}
	}
	if dir := filepath.Dir(path); dir != "" {
		if err := statefs.EnsurePrivateDirTree(dir); err != nil {
			return fmt.Errorf("persist: secure checkpoint dir: %w", err)
		}
	}
	if err := statefs.RemoveRegular(path + ".tmp"); err != nil {
		return fmt.Errorf("persist: reject legacy checkpoint temp: %w", err)
	}
	var snapshots []checkpointSnapshot
	if retain > 0 {
		snapshots, err = snapshotCheckpointSet(path, retain)
		if err != nil {
			return err
		}
		if err := publishRetainedHistory(path, snapshots, write); err != nil {
			return err
		}
	}
	if err := write(path, data, 0o600); err != nil {
		commitErr := fmt.Errorf("persist: commit checkpoint: %w", err)
		if retain == 0 {
			return commitErr
		}
		return withRollbackError(commitErr, restoreCheckpointSet(path, snapshots, 0))
	}
	return nil
}

func snapshotCheckpointSet(path string, retain int) ([]checkpointSnapshot, error) {
	snapshots := make([]checkpointSnapshot, retain+1)
	for i := 0; i <= retain; i++ {
		candidate := retainedCheckpointPath(path, i)
		data, present, err := statefs.ReadRegular(candidate, checkpointMaxBytes)
		if err != nil {
			return nil, fmt.Errorf("persist: snapshot checkpoint %s: %w", candidate, err)
		}
		snapshots[i] = checkpointSnapshot{data: data, present: present}
	}
	return snapshots, nil
}

func publishRetainedHistory(
	path string, snapshots []checkpointSnapshot, write checkpointWriter,
) error {
	if len(snapshots) < 2 || !snapshots[0].present {
		return nil
	}
	for target := len(snapshots) - 1; target >= 1; target-- {
		targetPath := retainedCheckpointPath(path, target)
		if err := applyCheckpointSnapshot(targetPath, snapshots[target-1], write); err != nil {
			historyErr := fmt.Errorf("persist: commit checkpoint history %s: %w", targetPath, err)
			return withRollbackError(historyErr, restoreCheckpointSet(path, snapshots, 1))
		}
	}
	return nil
}

func restoreCheckpointSet(path string, snapshots []checkpointSnapshot, first int) error {
	var restoreErr error
	for i := first; i < len(snapshots); i++ {
		target := retainedCheckpointPath(path, i)
		if err := applyCheckpointSnapshot(target, snapshots[i], statefs.AtomicWrite); err != nil {
			restoreErr = errors.Join(restoreErr, fmt.Errorf("restore %s: %w", target, err))
		}
	}
	return restoreErr
}

func applyCheckpointSnapshot(
	path string, snapshot checkpointSnapshot, write checkpointWriter,
) error {
	if snapshot.present {
		return write(path, snapshot.data, 0o600)
	}
	return statefs.RemoveRegular(path)
}

func retainedCheckpointPath(path string, index int) string {
	if index == 0 {
		return path
	}
	return fmt.Sprintf("%s.%d", path, index)
}

func withRollbackError(operationErr, rollbackErr error) error {
	if rollbackErr == nil {
		return operationErr
	}
	return fmt.Errorf("%w; checkpoint rollback failed: %v", operationErr, rollbackErr)
}

// Load reads a checkpoint from path.
//
// The bool reports presence. A missing file is the expected first-run state and
// returns (zero, false, nil) — absence is not an error. A present file that
// fails to decode returns (zero, false, err): corruption is surfaced, never
// silently swallowed as "no checkpoint", so the caller can't unknowingly throw
// real progress away. A readable, valid file returns (cp, true, nil).
func Load(path string) (Checkpoint, bool, error) {
	data, found, err := statefs.ReadRegular(path, checkpointMaxBytes)
	if err != nil {
		return Checkpoint{}, false, fmt.Errorf("persist: read checkpoint: %w", err)
	}
	if !found {
		return Checkpoint{}, false, nil
	}
	cp, err := decode(data)
	if err != nil {
		return Checkpoint{}, false, err
	}
	return cp, true, nil
}

// encode serializes a Checkpoint to its on-disk JSON form. Pure: no IO. Indented
// so a committed checkpoint stays human-readable when someone inspects a run.
func encode(cp Checkpoint) ([]byte, error) {
	data, err := json.MarshalIndent(cp, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("persist: encode checkpoint: %w", err)
	}
	return append(data, '\n'), nil
}

// decode parses a Checkpoint from its JSON encoding. Pure: no IO. Unlike the
// asset loader, this is strict — a malformed checkpoint is a hard error, because
// resuming from partial recovery state is more dangerous than refusing to.
func decode(data []byte) (Checkpoint, error) {
	var cp Checkpoint
	if err := json.Unmarshal(data, &cp); err != nil {
		return Checkpoint{}, fmt.Errorf("persist: decode checkpoint: %w", err)
	}
	if err := validateFormat(cp.FormatVersion); err != nil {
		return Checkpoint{}, err
	}
	if cp.FormatVersion == CheckpointFormatCurrent {
		if err := validateCurrentEncoding(data); err != nil {
			return Checkpoint{}, err
		}
		if err := validateCurrentCheckpoint(cp); err != nil {
			return Checkpoint{}, err
		}
	}
	return cp, nil
}

func validateFormat(format string) error {
	if format == "" || format == checkpointFormatV1 || format == CheckpointFormatCurrent {
		return nil
	}
	return fmt.Errorf("persist: unsupported checkpoint format %q", format)
}

var checkpointV2RequiredFields = []string{
	"workflow",
	"workflow_digest",
	"mode",
	"lifecycle",
	"iteration",
	"roadmap_completion",
	"gates_green",
	"reason",
	"updated_at_unix",
}

var checkpointV2OptionalScalarFields = []string{
	"phase_index",
	"spent_usd_micros",
}

// validateCurrentEncoding distinguishes a deliberately persisted zero/false
// value from a missing or explicit-null field. encoding/json maps null into the
// Go zero value for scalar fields, so validating only the decoded struct would
// let a truncated v2 checkpoint masquerade as a legitimate iteration-zero
// snapshot.
func validateCurrentEncoding(data []byte) error {
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		return fmt.Errorf("persist: decode checkpoint fields: %w", err)
	}
	for _, name := range checkpointV2RequiredFields {
		raw, ok := fields[name]
		if !ok {
			return fmt.Errorf("persist: checkpoint %s missing required field %q",
				CheckpointFormatCurrent, name)
		}
		raw = bytes.TrimSpace(raw)
		if len(raw) == 0 || bytes.Equal(raw, []byte("null")) {
			return fmt.Errorf("persist: checkpoint %s required field %q is null",
				CheckpointFormatCurrent, name)
		}
	}
	for _, name := range checkpointV2OptionalScalarFields {
		raw, ok := fields[name]
		if ok && bytes.Equal(bytes.TrimSpace(raw), []byte("null")) {
			return fmt.Errorf("persist: checkpoint %s optional scalar field %q is null",
				CheckpointFormatCurrent, name)
		}
	}
	return nil
}

func validateCurrentCheckpoint(cp Checkpoint) error {
	requiredStrings := []struct {
		name  string
		value string
	}{
		{"workflow", cp.Workflow},
		{"workflow_digest", cp.WorkflowDigest},
		{"mode", cp.Mode},
		{"lifecycle", cp.Lifecycle},
		{"reason", cp.Reason},
	}
	for _, field := range requiredStrings {
		if strings.TrimSpace(field.value) == "" {
			return fmt.Errorf("persist: checkpoint %s required field %q must be non-empty",
				CheckpointFormatCurrent, field.name)
		}
	}
	switch {
	case cp.Iteration < 0:
		return fmt.Errorf("persist: checkpoint iteration %d must be non-negative", cp.Iteration)
	case cp.RoadmapCompletion < 0 || cp.RoadmapCompletion > 1:
		return fmt.Errorf("persist: checkpoint roadmap_completion %v must be within [0,1]",
			cp.RoadmapCompletion)
	case cp.PhaseIndex < 0:
		return fmt.Errorf("persist: checkpoint phase_index %d must be non-negative", cp.PhaseIndex)
	case cp.SpentUsdMicros < 0:
		return fmt.Errorf("persist: checkpoint spent_usd_micros %d must be non-negative",
			cp.SpentUsdMicros)
	case cp.UpdatedAtUnix <= 0:
		return fmt.Errorf("persist: checkpoint updated_at_unix %d must be positive",
			cp.UpdatedAtUnix)
	}
	return nil
}
