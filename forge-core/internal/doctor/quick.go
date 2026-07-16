package doctor

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/persist"
)

// QuickCheck is one result from the fast pre-run/pre-evolve health scan.
// Status mirrors trace.Event's own vocabulary ("ok" | "FAIL" | "WARN") since
// these are recorded as trace events (kind="doctor"), not printed as CLI text.
type QuickCheck struct {
	Name   string
	Status string
	Detail string
}

// QuickChecks runs the fast subset of forge doctor's checks (all designed to
// complete in <5ms) used to auto-diagnose repo state before forge run/evolve
// (fourth-wave-architecture.md §方向2: forge doctor 的自动接入与主动诊断). A
// failing check is never a gate — the caller records every returned
// QuickCheck as an advisory trace event and proceeds regardless.
//
// When .forge/ does not exist yet (first run), a single "preflight" entry is
// returned and none of the other checks run. Otherwise each of checkpoint /
// trace / memory / tmp-residue contributes AT MOST one entry (some contribute
// none — e.g. a missing or empty trace.jsonl is unremarkable and silent), and
// a final "preflight complete" entry always closes the scan.
func QuickChecks(root string) []QuickCheck {
	dotForge := dotForgeDir(root)
	if _, err := os.Stat(dotForge); os.IsNotExist(err) {
		return []QuickCheck{{Name: "preflight", Status: "ok", Detail: ".forge not present (first run)"}}
	}
	var checks []QuickCheck
	checks = append(checks, quickCheckpointCheck(dotForge)...)
	checks = append(checks, quickTraceCheck(dotForge)...)
	checks = append(checks, quickMemoryCheck(dotForge)...)
	checks = append(checks, quickTmpResidueCheck(dotForge)...)
	checks = append(checks, QuickCheck{Name: "preflight", Status: "ok", Detail: "quick doctor check complete"})
	return checks
}

// quickCheckpointCheck reports checkpoint.json readability: FAIL on a read/
// decode error, "ok" when it parses, silent when simply absent (first run).
func quickCheckpointCheck(dotForge string) []QuickCheck {
	cpPath := filepath.Join(dotForge, "checkpoint.json")
	_, found, err := persist.Load(cpPath)
	if err != nil {
		return []QuickCheck{{Name: "checkpoint", Status: "FAIL", Detail: err.Error()}}
	}
	if found {
		return []QuickCheck{{Name: "checkpoint", Status: "ok", Detail: "readable"}}
	}
	return nil
}

// quickTraceCheck reports trace.jsonl completeness, but ONLY when the file
// exists and is non-empty; a missing/empty file (or a last line that is
// blank after trimming) is silent — unlike doctor.Run's traceCheck, which
// always emits.
func quickTraceCheck(dotForge string) []QuickCheck {
	tracePath := filepath.Join(dotForge, "trace.jsonl")
	st, err := os.Stat(tracePath)
	if err != nil || st.Size() == 0 {
		return nil
	}
	data, err := os.ReadFile(tracePath)
	if err != nil {
		return []QuickCheck{{Name: "trace", Status: "FAIL", Detail: err.Error()}}
	}
	lines := strings.Split(strings.TrimSpace(string(data)), "\n")
	if len(lines) == 0 || len(lines[len(lines)-1]) == 0 {
		return nil
	}
	lastLine := lines[len(lines)-1]
	if strings.HasPrefix(lastLine, "{") && strings.HasSuffix(lastLine, "}") {
		return []QuickCheck{{Name: "trace", Status: "ok", Detail: fmt.Sprintf("%d events", len(lines))}}
	}
	return []QuickCheck{{Name: "trace", Status: "FAIL", Detail: "last line truncated"}}
}

// quickMemoryCheck reports memory.jsonl parseability, but ONLY when the file
// exists and is non-empty (a missing/empty store is silent, matching
// quickTraceCheck's "only when there's something to say" convention).
func quickMemoryCheck(dotForge string) []QuickCheck {
	memPath := filepath.Join(dotForge, "memory.jsonl")
	st, err := os.Stat(memPath)
	if err != nil || st.Size() == 0 {
		return nil
	}
	entries, memErr := memory.Load(memPath)
	if memErr != nil {
		return []QuickCheck{{Name: "memory", Status: "FAIL", Detail: memErr.Error()}}
	}
	return []QuickCheck{{Name: "memory", Status: "ok", Detail: fmt.Sprintf("%d entries", len(entries))}}
}

// quickTmpResidueCheck warns about leftover *.tmp files; silent when none.
func quickTmpResidueCheck(dotForge string) []QuickCheck {
	files, _ := filepath.Glob(filepath.Join(dotForge, "*.tmp"))
	if len(files) == 0 {
		return nil
	}
	return []QuickCheck{{Name: "tmp-residue", Status: "WARN", Detail: fmt.Sprintf("%d leftover file(s)", len(files))}}
}
