// trace_cmd.go — `forge trace`: human-readable trace log inspection.
// Reads .forge/trace.jsonl and displays events in a formatted table, with
// optional filtering by kind, status, model, or run id.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/statefs"
	"forgeos/forge-core/internal/trace"
)

// cmdTrace implements `forge trace [--kind K] [--status S] [--run-id ID]
// [--tail N] [--strict] [--root DIR]`.
func cmdTrace(args []string) int {
	fs := flag.NewFlagSet("trace", flag.ContinueOnError)
	kind := fs.String("kind", "", "filter by event kind (agent|gate|iteration|converge|decision|error|overload_backoff|stale_increment)")
	status := fs.String("status", "", "filter by event status (PASS|FAIL|ok|timeout|retry|recovered|failed|stale)")
	model := fs.String("model", "", "filter by model name (sonnet|opus|haiku)")
	runID := fs.String("run-id", "", "filter by run_id (full value or unique prefix)")
	tail := fs.Int("tail", 0, "show only the last N events (0 = all)")
	strict := fs.Bool("strict", false, "fail if any malformed JSONL record is encountered")
	root := fs.String("root", "", "repo root (default $FORGE_REPO_ROOT or .)")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	if *tail < 0 {
		fmt.Fprintln(os.Stderr, "forge trace: --tail must be >= 0")
		return 2
	}
	repoRoot := gate.RepoRoot(*root)
	if err := rejectTrackedForgeControlState(repoRoot); err != nil {
		fmt.Fprintf(os.Stderr, "forge trace: %v\n", err)
		return 1
	}
	events, malformed, err := loadTraceEvents(repoRoot, *kind, *status, *model, *runID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge trace: %v\n", err)
		return 1
	}
	if malformed > 0 {
		fmt.Fprintf(os.Stderr, "forge trace: WARNING skipped %d malformed JSONL record(s)\n", malformed)
		if *strict {
			return 1
		}
	}
	if len(events) == 0 {
		fmt.Println("forge trace: no matching events")
		return 0
	}
	if *tail > 0 && *tail < len(events) {
		events = events[len(events)-*tail:]
	}
	printTraceEvents(events)
	fmt.Printf("\n%d event(s) shown", len(events))
	if *kind != "" || *status != "" || *model != "" || *runID != "" {
		fmt.Printf(" (filtered)")
	}
	fmt.Println()
	return 0
}

// loadTraceEvents reads .forge/trace.jsonl and returns matching events plus the
// number of malformed records it had to skip. A run-id prefix is accepted so an
// operator can use the short value printed in the table.
func loadTraceEvents(root, kindFilter, statusFilter, modelFilter, runIDFilter string) ([]trace.Event, int, error) {
	traceFile := filepath.Join(root, ".forge", "trace.jsonl")
	data, found, err := statefs.ReadRegular(traceFile, 16<<20)
	if err != nil {
		return nil, 0, err
	}
	if !found {
		return nil, 0, fmt.Errorf("no trace file at %s (run a workflow first)", traceFile)
	}

	var events []trace.Event
	malformed := 0
	sc := bufio.NewScanner(bytes.NewReader(data))
	sc.Buffer(make([]byte, 0, 64*1024), 1024*1024)
	for sc.Scan() {
		var ev trace.Event
		if err := json.Unmarshal(sc.Bytes(), &ev); err != nil {
			malformed++
			continue
		}
		if err := trace.ValidateFormat(ev.Format); err != nil {
			malformed++
			continue
		}
		if kindFilter != "" && ev.Kind != kindFilter {
			continue
		}
		if statusFilter != "" && ev.Status != statusFilter {
			continue
		}
		if modelFilter != "" && ev.Model != modelFilter {
			continue
		}
		if runIDFilter != "" && !strings.HasPrefix(ev.RunID, runIDFilter) {
			continue
		}
		events = append(events, ev)
	}
	if err := sc.Err(); err != nil {
		return nil, malformed, fmt.Errorf("read error: %w", err)
	}
	return events, malformed, nil
}

// printTraceEvents prints a formatted table of trace events.
func printTraceEvents(events []trace.Event) {
	fmt.Printf("%-6s %-12s %-24s %-8s %-8s %-12s %s\n", "SEQ", "KIND", "NAME", "STATUS", "MODEL", "RUN_ID", "DETAIL")
	fmt.Println(strings.Repeat("-", 104))
	for _, ev := range events {
		modelStr := ev.Model
		if modelStr == "" {
			modelStr = "-"
		}
		detail := ev.Detail
		if ev.DurationMs > 0 {
			dur := fmt.Sprintf("%dms", ev.DurationMs)
			if ev.DurationMs >= 1000 {
				dur = fmt.Sprintf("%.1fs", float64(ev.DurationMs)/1000)
			}
			detail = joinDetail(detail, dur)
		}
		if ev.CostUsdMicros > 0 {
			detail = joinDetail(detail, fmt.Sprintf("$%.4f", float64(ev.CostUsdMicros)/1e6))
		}
		runID := ev.RunID
		if runID == "" {
			runID = "-"
		}
		fmt.Printf("%-6d %-12s %-24s %-8s %-8s %-12s %s\n",
			ev.Seq, ev.Kind, truncate(ev.Name, 23), ev.Status, modelStr, truncate(runID, 12), detail)
	}
}

// joinDetail concatenates two detail segments with a space separator.
func joinDetail(existing, addition string) string {
	if existing == "" {
		return addition
	}
	return existing + " " + addition
}

func truncate(s string, max int) string {
	if len(s) <= max {
		return s
	}
	return s[:max-1] + "…"
}
