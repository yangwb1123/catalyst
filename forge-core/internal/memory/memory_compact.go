// Compaction and retention for the memory package (seventh-wave-data-realism.md
// §方向2). memory.go owns the append/load/query primitives over the JSONL
// store; this file owns the two ways a long-running evolve loop keeps that
// store from growing without bound: Prune's sibling rewriteStore (the shared
// atomic rewrite primitive) and the age/kind-aware Compact, which — unlike
// Prune's blunt "keep the last N" truncation — retains a summary of what it
// discards instead of losing it outright.
package memory

import (
	"bytes"
	"fmt"
	"sort"
	"strings"
	"time"

	"forgeos/forge-core/internal/statefs"
)

// rewriteStore atomically replaces the memory store with a new set of entries.
// It writes to a temp file and renames over the target, so a crash never leaves
// a truncated file. Shared by Prune (memory.go) and Compact.
func rewriteStore(path string, entries []Entry) error {
	if err := statefs.RemoveRegular(path + ".tmp"); err != nil {
		return fmt.Errorf("memory: reject legacy temp: %w", err)
	}
	var data bytes.Buffer
	for _, e := range entries {
		line, err := encode(e)
		if err != nil {
			return fmt.Errorf("memory: encode entry: %w", err)
		}
		if _, err := data.Write(line); err != nil {
			return fmt.Errorf("memory: write entry: %w", err)
		}
	}
	if err := statefs.AtomicWrite(path, data.Bytes(), 0o600); err != nil {
		return fmt.Errorf("memory: rewrite store: %w", err)
	}
	return nil
}

// DefaultCompactThreshold is the entry count above which Compact triggers
// summarization. Based on the estimate that a real 24h evolve run produces
// ~500 entries/day.
const DefaultCompactThreshold = 500

// DefaultCompactKeepPerKind is the number of entries retained per kind after
// compaction. 20 per kind × 3 kinds = 60 entries + up to 3 summary entries.
const DefaultCompactKeepPerKind = 20

// CompactAgeSeconds defines the age boundary: only entries older than this
// duration (default 24 hours) are eligible for compaction. Recent entries are
// preserved verbatim so current-prompt context is not lost.
const CompactAgeSeconds = 86400 // 24 hours

// Compact reads the memory store at path, groups old entries by kind, and
// compacts them: for each kind it retains at most keepPerKind of the most
// recent entries, and the rest are replaced by a summary entry. Recent entries
// (created within the last ageSeconds seconds) are always preserved verbatim.
//
// A compaction is triggered only when len(entries) > threshold. If the store
// has fewer entries, Compact is a no-op (returns nil, false, nil).
//
// Returns the number of entries removed, whether compaction happened, and any
// error. After a successful compaction, the in-memory cache is invalidated so
// the next Load re-reads the new file.
func Compact(path string, threshold, keepPerKind, ageSeconds int) (removed int, compacted bool, err error) {
	if keepPerKind < 0 {
		keepPerKind = 0 // safe default, mirrors Prune's keepLast<=0 clamp
	}
	entries, err := Load(path)
	if err != nil {
		return 0, false, fmt.Errorf("memory: compact load: %w", err)
	}
	if len(entries) <= threshold {
		return 0, false, nil // below threshold, no-op
	}

	recent, old := splitByAge(entries, ageSeconds)
	if len(old) == 0 {
		return 0, false, nil // nothing eligible for compaction
	}

	compactedEntries := compactByKind(old, keepPerKind)

	// Rebuild: recent entries (preserved verbatim) + compacted old entries.
	all := append(recent, compactedEntries...)
	if err := rewriteStore(path, all); err != nil {
		return 0, false, fmt.Errorf("memory: compact rewrite: %w", err)
	}
	invalidateLoadCache()
	return len(entries) - len(all), true, nil
}

// splitByAge partitions entries into recent (created within ageSeconds of now)
// and old (eligible for compaction). An entry without a timestamp (0) is
// treated as old for safety — better to compact an unmarked entry than to let
// it grow unbounded.
func splitByAge(entries []Entry, ageSeconds int) (recent, old []Entry) {
	now := time.Now().Unix()
	for _, e := range entries {
		if e.CreatedAtUnix > 0 && now-e.CreatedAtUnix < int64(ageSeconds) {
			recent = append(recent, e)
		} else {
			old = append(old, e)
		}
	}
	return recent, old
}

// compactByKind groups old entries by Kind and, for each kind, retains at most
// keepPerKind of the most recent entries verbatim, replacing the rest with a
// single summary entry (via summarizeBlock). Entry order within a kind is
// left-to-right oldest-to-newest, so the last entries are the most recent.
// Kinds are emitted in sorted order so the rewritten file's block order is
// deterministic across runs, matching Load's "in file order" doc contract.
func compactByKind(old []Entry, keepPerKind int) []Entry {
	byKind := make(map[string][]Entry)
	for _, e := range old {
		byKind[e.Kind] = append(byKind[e.Kind], e)
	}
	kinds := make([]string, 0, len(byKind))
	for kind := range byKind {
		kinds = append(kinds, kind)
	}
	sort.Strings(kinds)

	var compactedEntries []Entry
	for _, kind := range kinds {
		kindEntries := byKind[kind]
		if len(kindEntries) <= keepPerKind {
			// Fewer than the keep ceiling — preserve all verbatim.
			compactedEntries = append(compactedEntries, kindEntries...)
			continue
		}
		keep := kindEntries[len(kindEntries)-keepPerKind:]
		summarized := kindEntries[:len(kindEntries)-keepPerKind]
		compactedEntries = append(compactedEntries, keep...)

		// Generate a summary entry for the compacted block.
		if len(summarized) > 0 {
			if summary := summarizeBlock(kind, summarized); summary != nil {
				compactedEntries = append(compactedEntries, *summary)
			}
		}
	}
	return compactedEntries
}

// summarizeBlock creates a single summary entry for a block of compacted entries
// of the same kind. The summary carries the kind, a count of items summarized,
// and the time range covered. Returns nil when the block is empty (defensive).
func summarizeBlock(kind string, entries []Entry) *Entry {
	if len(entries) == 0 {
		return nil
	}
	// Determine time range of the compacted block.
	minTime, maxTime := entries[0].CreatedAtUnix, entries[0].CreatedAtUnix
	topics := make(map[string]int)
	total := 0
	for _, e := range entries {
		if e.CreatedAtUnix > 0 {
			if e.CreatedAtUnix < minTime || minTime == 0 {
				minTime = e.CreatedAtUnix
			}
			if e.CreatedAtUnix > maxTime {
				maxTime = e.CreatedAtUnix
			}
		}
		topics[e.Topic]++
		total++
	}

	// Build a compact detail string.
	timeRange := ""
	if minTime > 0 && maxTime > 0 {
		timeRange = fmt.Sprintf(", %d entries from [%d..%d]", total, minTime, maxTime)
	}
	topicSummary := ""
	if len(topics) > 0 {
		parts := make([]string, 0, len(topics))
		for t, c := range topics {
			parts = append(parts, fmt.Sprintf("%s:%d", t, c))
		}
		topicSummary = "; topics: " + strings.Join(parts, ", ")
	}
	detail := fmt.Sprintf("compacted %d %s entries%s%s", total, kind, timeRange, topicSummary)
	now := time.Now().Unix()
	return &Entry{
		Format:        "forgeos.memory.v1",
		Kind:          "compact_summary",
		Topic:         kind,
		Detail:        detail,
		CreatedAtUnix: now,
	}
}
