// Package memory is forge-core's cross-session knowledge store: the durable
// notebook the autonomous loop writes to so a 24h run does not keep relearning
// what it already knew. Without it, the loop is amnesiac across iterations — by
// hour 8 it tears down the wall it built at hour 2, because it does not remember
// the gap it found, the decision it made, or the lesson a failed attempt taught.
// This package owns the storage for those knowledge entries so a later wave can
// have the evolve loop record findings once and consult them on every round,
// instead of rediscovering the same problem N iterations in a row. (Wiring it
// into the loop — append-on-discovery, query-before-acting — is that later wave;
// this wave delivers only the store: append, load, and a pure query filter.)
//
// The shape that drives the design: persist's Checkpoint is ONE recoverable
// snapshot you overwrite each round; memory is the opposite — an ACCUMULATING
// log of entries you only ever add to and never rewrite. That difference picks
// the on-disk format. The store is JSONL: one self-describing JSON object per
// line, appended with a single O_APPEND write. Appending one framed line is the
// natural primitive for a grow-only log — it touches none of the existing
// history, so it cannot truncate or corrupt prior knowledge the way a
// read-all/rewrite cycle could if it died mid-write. (A whole-file atomic
// rewrite would also work but would re-serialize and re-risk the entire corpus
// on every single append, paying O(n) bytes and a crash-window over all history
// to add one entry — wrong trade for a log that only grows.) Each Append issues
// one write(2) of one '\n'-terminated record under O_APPEND, so the line is the
// atomic unit and concurrent or successive appends cannot interleave into a
// half-written, unparseable line.
//
// Two properties matter most, and both are about not lying to the caller:
//
//   - Honest, fault-tolerant load. A missing store is the normal cold-start case
//     (the loop has simply learned nothing yet) and returns (nil, nil) — absence
//     is not an error. A present-but-malformed line is an explicit error, never
//     silently skipped: quietly dropping a garbled line would make Load report
//     LESS knowledge than the store actually holds, and the loop would act as if
//     a finding it once recorded had never happened (honesty-first). The caller
//     is forced to see the corruption and decide.
//
//   - Pure query, separate from IO. Query is a plain in-memory filter over an
//     already-loaded slice — it does no IO and never mutates its input, so the
//     selection logic is unit-testable without a disk and the same loaded corpus
//     can be filtered many ways. Append and Load are thin IO shells; encode,
//     decode, and Query are pure, so the serialization and filtering contracts
//     are testable without touching the filesystem. Pure Go standard library only.
package memory

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
)

// Entry kinds. A knowledge entry is one of three things the loop wants to carry
// forward across sessions, and the Kind field is constrained to these so queries
// and downstream tooling read a small, stable vocabulary rather than free text.
const (
	KindGap      = "gap"      // a shortfall the loop noticed and must not re-discover
	KindDecision = "decision" // a choice the loop committed to, with its rationale
	KindLesson   = "lesson"   // something a prior attempt (often a failure) taught
)

// Entry is a single piece of cross-session knowledge. It is intentionally flat
// and string-typed so it round-trips through JSON without bespoke decoders and
// stays greppable by tools that do not know forge-core's internal types. The
// json tags are the on-disk contract that downstream tooling reads, so they are
// stable and lower_snake.
//
// CreatedAtUnix is injected by the caller rather than read from time.Now inside
// this package, so the store stays a deterministic pure function of its inputs —
// tests can assert exact bytes, and the clock is the caller's concern (matching
// persist's UpdatedAtUnix).
type Entry struct {
	Kind          string `json:"kind"`            // one of KindGap | KindDecision | KindLesson
	Topic         string `json:"topic"`           // subject this entry is about (the query key)
	Detail        string `json:"detail"`          // the knowledge itself, in free text
	Iteration     int    `json:"iteration"`       // loop iteration that produced this entry
	CreatedAtUnix int64  `json:"created_at_unix"` // caller-supplied creation time (Unix seconds)
}

// Append adds e to the JSONL knowledge store at path as one new line.
//
// It opens the file with O_APPEND|O_CREATE and issues a single write of the
// encoded, '\n'-terminated record. O_APPEND makes the kernel place the bytes at
// the current end of file for each write, so the one-line record is the atomic
// unit: successive (or concurrent) appends each land as a whole line and can
// never interleave into a corrupt half-line. Existing history is never read or
// rewritten, so a crash can at most lose the in-flight line, never damage prior
// entries. Parent directories are created as needed.
func Append(path string, e Entry) error {
	line, err := encode(e)
	if err != nil {
		return err
	}
	if dir := filepath.Dir(path); dir != "" {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			return fmt.Errorf("memory: create store dir: %w", err)
		}
	}
	f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
	if err != nil {
		return fmt.Errorf("memory: open store: %w", err)
	}
	// Single Write of a whole '\n'-terminated line: under O_APPEND this is the
	// atomic record boundary, so no lock is needed to keep lines from interleaving.
	if _, err := f.Write(line); err != nil {
		f.Close()
		return fmt.Errorf("memory: append entry: %w", err)
	}
	if err := f.Close(); err != nil {
		return fmt.Errorf("memory: close store: %w", err)
	}
	return nil
}

// Load reads every entry from the JSONL store at path, in file order.
//
// A missing file is the expected cold-start state — the loop has recorded
// nothing yet — and returns (nil, nil): absence is not an error. A present file
// with a malformed line returns (nil, err): a garbled line is surfaced, never
// silently skipped, because dropping it would make Load under-report what the
// store holds and let the loop forget a finding it once recorded. A readable
// store returns its entries in the order they were appended.
func Load(path string) ([]Entry, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return nil, nil
		}
		return nil, fmt.Errorf("memory: read store: %w", err)
	}
	return decode(data)
}

// Query returns the entries matching kind and topic, preserving input order.
//
// Pure: it allocates a fresh slice and never mutates entries, so a single loaded
// corpus can be filtered many ways and the call is safe to unit-test without a
// disk. An empty kind or topic means "do not constrain on that field", so
// Query(es, "", "") returns a copy of all entries and Query(es, KindGap, "")
// returns every gap regardless of topic. Matching is exact (not substring): the
// fields are a controlled vocabulary, not free-text search.
func Query(entries []Entry, kind, topic string) []Entry {
	out := make([]Entry, 0, len(entries))
	for _, e := range entries {
		if kind != "" && e.Kind != kind {
			continue
		}
		if topic != "" && e.Topic != topic {
			continue
		}
		out = append(out, e)
	}
	return out
}

// encode is the pure Entry→JSONL-bytes step, factored out of Append so the wire
// format is unit-testable without a filesystem. It returns one compact JSON
// object followed by a single '\n' — the JSONL line framing — so the newline is
// part of the record and Append writes the bytes verbatim. Pure: no IO.
func encode(e Entry) ([]byte, error) {
	b, err := json.Marshal(e)
	if err != nil {
		return nil, fmt.Errorf("memory: encode entry: %w", err)
	}
	return append(b, '\n'), nil
}

// decode parses the whole JSONL store into entries, in order. Pure: no IO.
//
// It scans line by line; blank lines (e.g. a trailing newline) are skipped, but
// any non-blank line that fails to parse is a hard error naming the line number,
// because silently dropping a corrupt entry would hide lost knowledge from the
// caller. bufio.Scanner's default token size is raised so a long Detail does not
// trip the line-length limit and masquerade as corruption.
func decode(data []byte) ([]Entry, error) {
	var entries []Entry
	sc := bufio.NewScanner(bytes.NewReader(data))
	sc.Buffer(make([]byte, 0, 64*1024), 16*1024*1024)
	line := 0
	for sc.Scan() {
		line++
		raw := bytes.TrimSpace(sc.Bytes())
		if len(raw) == 0 {
			continue
		}
		var e Entry
		if err := json.Unmarshal(raw, &e); err != nil {
			return nil, fmt.Errorf("memory: decode entry on line %d: %w", line, err)
		}
		entries = append(entries, e)
	}
	if err := sc.Err(); err != nil {
		return nil, fmt.Errorf("memory: scan store: %w", err)
	}
	return entries, nil
}
