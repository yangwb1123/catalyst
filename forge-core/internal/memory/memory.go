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
	"sync"
	"time"
)

// loadCache is a per-path cache for memory.Load: it caches decoded entries
// keyed by (path, mtime), so repeated Load calls within the same iteration
// (one per phase, analysis §2.2) read the file only once until it changes.
// Uses sync.Map so concurrent forge processes on different projects do not
// invalidate each other's cache entries (analysis §方向3: global cache collision).
// The cache is invalidated on every Append call via invalidateLoadCache.
var (
	loadCaches sync.Map // key=path(string), value=*loadCacheEntry
)

type loadCacheEntry struct {
	entries []Entry
	modTime time.Time
	err     error
	valid   bool
}

// loadFromCache checks the per-path cache before reading the file. Returns
// cached entries when the path matches and the file's mtime is unchanged.
func loadFromCache(path string) ([]Entry, bool, error) {
	v, ok := loadCaches.Load(path)
	if !ok {
		return nil, false, nil
	}
	ce := v.(*loadCacheEntry)
	if !ce.valid {
		return nil, false, nil
	}
	st, err := os.Stat(path)
	if err != nil {
		return nil, false, nil
	}
	if !st.ModTime().Equal(ce.modTime) {
		return nil, false, nil
	}
	return ce.entries, true, ce.err
}

// storeToCache caches the result of a Load call. Called by Load after a miss.
func storeToCache(path string, entries []Entry, err error) {
	ce := &loadCacheEntry{entries: entries, err: err, valid: true}
	if st, stErr := os.Stat(path); stErr == nil {
		ce.modTime = st.ModTime()
	}
	loadCaches.Store(path, ce)
}

// invalidateLoadCache clears all per-path caches so the next Load re-reads
// the file. Called by Append to ensure newly written entries are immediately
// visible. Uses Delete so concurrent forge processes on different projects
// do not trample each other's cache entries.
func invalidateLoadCache() {
	loadCaches.Range(func(key, _ interface{}) bool {
		loadCaches.Delete(key)
		return true
	})
}

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
// Confidence is an OPTIONAL caller-supplied signal (default 1.0) that the
// knowledge-entry source can set to annotate how trustworthy the entry is.
// A low-confidence entry (e.g. < 0.3) should be treated as speculation or
// unverified — the prompt layer prefixes it with "[unverified]" to cue the
// agent to independently verify rather than take it as settled truth.
// A zero-value (omitted in JSON via omitempty) loads as 1.0, so old files
// without the field are treated as full-confidence and the contract is
// byte-for-byte backward compatible.
//
// Supersedes is an OPTIONAL reference to a prior entry ID (its Topic) that
// THIS entry overrides. When Load encounters an entry whose Topic matches a
// prior entry's Supersedes target, the superseded entry is filtered out of
// the returned slice — the caller sees only the active (non-superseded)
// knowledge set. This gives the evolve loop an explicit retraction mechanism
// (analysis §回路A): a new iteration can write a corrected Decision that
// supersedes a prior wrong one, and the prompt layer automatically stops
// seeing the old entry. A zero-value (empty string) means "does not supersede
// anything", so old files without the field are byte-for-byte unaffected.
//
// CreatedAtUnix is injected by the caller rather than read from time.Now inside
// this package, so the store stays a deterministic pure function of its inputs —
// tests can assert exact bytes, and the clock is the caller's concern (matching
// persist's UpdatedAtUnix).
//
// Source is an OPTIONAL caller-supplied provenance marker that records which
// phase/agent produced this entry (e.g. "planner", "implementer", "reviewer").
// It enables downstream filtering and trust attribution: entries from a
// reader/verifier agent may be weighted differently from those from a writer.
// An empty source (the default) means "unknown provenance" — backward-compatible.
type Entry struct {
	Format        string  `json:"_format,omitempty"`
	Kind          string  `json:"kind"`                 // one of KindGap | KindDecision | KindLesson
	Topic         string  `json:"topic"`                // subject this entry is about (the query key)
	Detail        string  `json:"detail"`               // the knowledge itself, in free text
	Iteration     int     `json:"iteration"`            // loop iteration that produced this entry
	Source        string  `json:"source,omitempty"`     // phase/agent that produced this entry (empty=unknown)
	Confidence    float64 `json:"confidence,omitempty"` // 0.0-1.0, default 1.0 (omitted=highest)
	Supersedes    string  `json:"supersedes,omitempty"` // Topic this entry replaces (empty=none)
	CreatedAtUnix int64   `json:"created_at_unix"`      // caller-supplied creation time (Unix seconds)
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
//
// The Format field is set to "forgeos.memory.v1" on append so every new line
// carries its format version; old lines without the field decode with default
// version "v1" (backward compatible).
func Append(path string, e Entry) error {
	if e.Format == "" {
		e.Format = "forgeos.memory.v1"
	}
	line, err := encode(e)
	if err != nil {
		return err
	}
	invalidateLoadCache() // invalidate cache so the next Load sees the new entry
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

// Load reads every entry from the JSONL store at path, in file order. It uses
// an mtime-based cache (§2.2): repeated calls with the same path and unchanged
// file mtime return the cached result — avoids re-reading and re-parsing the
// file for every phase in the same iteration.
//
// A missing file returns (nil, nil): cold start. A malformed line returns
// (nil, err): surfaced, never silently skipped.
//
// After loading, entries that have been superseded by a later entry with a
// matching Supersedes Topic are filtered out, so the caller sees only the
// active (non-superseded) knowledge set. This gives the evolve loop an
// explicit retraction mechanism (§回路A): a new iteration can write a
// corrected Decision that supersedes a prior wrong one, and the prompt layer
// automatically stops seeing the old entry.
func Load(path string) ([]Entry, error) {
	if entries, ok, err := loadFromCache(path); ok {
		return entries, err
	}
	data, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			storeToCache(path, nil, nil)
			return nil, nil
		}
		err = fmt.Errorf("memory: read store: %w", err)
		storeToCache(path, nil, err)
		return nil, err
	}
	entries, err := decode(data)
	if err != nil {
		storeToCache(path, nil, err)
		return nil, err
	}
	entries = filterSuperseded(entries)
	storeToCache(path, entries, nil)
	return entries, nil
}

// Prune rewrites the memory store at path, keeping only the last N entries
// (most recent). It is the compaction counterpart of the append-only Append:
// a long-running evolve loop that accumulated 1000+ entries can be trimmed
// to the most relevant ones without losing the on-disk format (JSONL).
// Returns the number of entries removed, or 0 and an error.
// After a successful prune, the in-memory cache is invalidated so the next
// Load re-reads the new file.
//
// See memory_compact.go for Prune's on-disk sibling rewriteStore and for the
// age/kind-aware Compact, which trades exact truncation for a summarized tail.
func Prune(path string, keepLast int) (int, error) {
	entries, err := Load(path)
	if err != nil {
		return 0, err
	}
	if len(entries) == 0 {
		return 0, nil
	}
	if keepLast <= 0 {
		keepLast = 500 // safe default
	}
	if len(entries) <= keepLast {
		return 0, nil // nothing to prune
	}
	keep := entries[len(entries)-keepLast:]
	if err := rewriteStore(path, keep); err != nil {
		return 0, err
	}
	invalidateLoadCache()
	return len(entries) - len(keep), nil
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
		// Zero-value Confidence reads as 1.0 (backward compat: old files without
		// the field were implicitly full-confidence).
		if e.Confidence == 0 {
			e.Confidence = 1.0
		}
		entries = append(entries, e)
	}
	if err := sc.Err(); err != nil {
		return nil, fmt.Errorf("memory: scan store: %w", err)
	}
	return entries, nil
}

// filterSuperseded removes entries that have been superseded by a later entry
// with a matching Supersedes field. Two-pass pure filter:
//
// Pass 1 (right-to-left): walk entries from newest to oldest, tracking which
// Topics are "active" (have been superseded by a later entry that we kept).
// The first entry with a given Supersedes target becomes the keeper; any
// earlier entry with that Topic is filtered out.
//
// Pass 2 (left-to-right): select entries whose Topic is NOT in the superseded
// set, plus the superseding entries themselves. Preserves input order.
func filterSuperseded(entries []Entry) []Entry {
	if len(entries) == 0 {
		return entries
	}
	// Pass 1: right-to-left — find the active (keeper) entry for each
	// superseded topic. A later entry with Supersedes=target marks target as
	// superseded and itself as the active replacement. If a yet-later entry
	// also supersedes the same target, the latest one wins.
	active := make(map[string]int) // topic → index of the active superseding entry
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
	// Pass 2: left-to-right — keep entries that are not superseded, or are
	// the active superseding entry for their topic.
	out := make([]Entry, 0, len(entries))
	for i, e := range entries {
		if superseded[e.Topic] {
			// This entry's topic is superseded. Keep it only if it is the
			// active superseding entry (the one doing the superseding).
			if keeperIdx, ok := active[e.Topic]; ok && keeperIdx == i {
				out = append(out, e)
			}
			continue
		}
		out = append(out, e)
	}
	return out
}
