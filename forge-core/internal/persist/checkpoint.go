// Package persist gives forge-core's autonomous loop a durable memory.
//
// The loop's recovery state lives in process memory, so a crash mid-run means
// replaying from scratch and discarding already-completed work. This package
// owns the storage layer for that state: a single Checkpoint snapshot written
// after each iteration and read back on startup to resume where the loop left
// off. (Wiring it into the loop — write-per-round, resume-on-start — is a later
// wave; this wave delivers only read/write with atomic persistence.)
//
// Two properties matter most here, and both are about not lying to the caller:
//
//   - Atomic writes. Save never overwrites a good checkpoint in place. It writes
//     a sibling temp file, fsyncs it, then renames it over the target. rename(2)
//     is atomic within a filesystem, so a crash mid-write leaves either the old
//     intact checkpoint or the new one — never a half-written, unparseable file
//     that would itself corrupt the recovery path.
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
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
)

// Checkpoint is the minimal state needed to resume the autonomous loop after a
// crash or clean stop. It is deliberately small: just enough to re-enter the
// workflow at the right place and re-explain why it last stopped.
//
// UpdatedAtUnix is injected by the caller rather than read from time.Now inside
// this package, so persistence stays a deterministic pure function of its
// inputs — tests can assert exact bytes, and the clock is the caller's concern.
type Checkpoint struct {
	Workflow          string  `json:"workflow"`           // workflow asset being run (e.g. "build")
	Mode              string  `json:"mode"`               // execution mode the loop was driving under
	Iteration         int     `json:"iteration"`          // count of iterations already completed
	RoadmapCompletion float64 `json:"roadmap_completion"` // last observed completion fraction in [0,1]
	GatesGreen        bool    `json:"gates_green"`        // whether all required gates were green at the snapshot
	Reason            string  `json:"reason"`             // why the loop last stopped (for resume context)
	UpdatedAtUnix     int64   `json:"updated_at_unix"`    // caller-supplied snapshot time (Unix seconds)
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
// It writes to a sibling temp file ("<path>.tmp"), fsyncs it to durable storage,
// then renames it over path. The temp file lives in the same directory as the
// target so the rename stays within one filesystem and is therefore atomic — a
// crash at any point leaves either the prior checkpoint or the new one whole,
// never a truncated file. Parent directories are created as needed.
func Save(path string, cp Checkpoint) error {
	data, err := encode(cp)
	if err != nil {
		return err
	}
	if dir := filepath.Dir(path); dir != "" {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return fmt.Errorf("persist: create checkpoint dir: %w", err)
		}
	}
	tmp := path + ".tmp"
	if err := writeSynced(tmp, data); err != nil {
		return err
	}
	// Rename is the atomic commit point. If it fails, drop the temp file so a
	// stale ".tmp" can't masquerade as state or block the next Save.
	if err := os.Rename(tmp, path); err != nil {
		os.Remove(tmp)
		return fmt.Errorf("persist: commit checkpoint: %w", err)
	}
	return nil
}

// writeSynced writes data to path and fsyncs before close, so the bytes are on
// durable storage before Save's rename publishes them. Without the Sync, a power
// loss could land the rename but not the file contents, defeating atomicity.
func writeSynced(path string, data []byte) error {
	f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o644)
	if err != nil {
		return fmt.Errorf("persist: open temp checkpoint: %w", err)
	}
	if _, err := f.Write(data); err != nil {
		f.Close()
		return fmt.Errorf("persist: write temp checkpoint: %w", err)
	}
	if err := f.Sync(); err != nil {
		f.Close()
		return fmt.Errorf("persist: sync temp checkpoint: %w", err)
	}
	if err := f.Close(); err != nil {
		return fmt.Errorf("persist: close temp checkpoint: %w", err)
	}
	return nil
}

// Load reads a checkpoint from path.
//
// The bool reports presence. A missing file is the expected first-run state and
// returns (zero, false, nil) — absence is not an error. A present file that
// fails to decode returns (zero, false, err): corruption is surfaced, never
// silently swallowed as "no checkpoint", so the caller can't unknowingly throw
// real progress away. A readable, valid file returns (cp, true, nil).
func Load(path string) (Checkpoint, bool, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return Checkpoint{}, false, nil
		}
		return Checkpoint{}, false, fmt.Errorf("persist: read checkpoint: %w", err)
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
	return cp, nil
}
