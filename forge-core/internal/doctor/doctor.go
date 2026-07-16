// Package doctor is the CORE LOGIC behind forge's health-diagnostic
// subcommands (`forge doctor`, `forge status`, `forge validate --models`) and
// the automatic pre-run health scan (quickDoctorCheck). It inspects the
// .forge/ runtime directory (checkpoint.json, trace.jsonl, memory.jsonl),
// governance asset directories (.agent/, harness/, .ai/prompts/), and
// workflow/agent-card cross-references, returning plain data — never
// printing. cmd/forge stays a thin CLI-dispatch layer: it parses flags, calls
// into this package, and formats the result to stdout/JSON in whatever shape
// the command needs. This is the same split as internal/migrate, internal/
// mode, and internal/risk: a deterministic, pure-Go distillation the cmd
// layer renders, not a printer itself.
//
// Split across files by seam:
//   - doctor.go:     `forge doctor`'s non-anomaly health checks (Run).
//   - quick.go:      the fast pre-run/pre-evolve scan (QuickChecks).
//   - anomaly.go:    checkpoint-history trend analysis (DetectAnomalies) and
//     the shared checkpoint-chain loader both `forge doctor --anomaly` and
//     `forge status --history` use.
//   - status.go:     `forge status`'s repo-state snapshot (Status).
//   - governance.go: `forge status --governance`'s asset/ADR report.
//   - models.go:     `forge validate --models`'s workflow/agent-card scan.
package doctor

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/persist"
)

// dotForgeDir returns <root>/.forge — the runtime state directory every
// check in this package inspects.
func dotForgeDir(root string) string { return filepath.Join(root, ".forge") }

// ageString renders how long ago t was as "today" / "N days ago", the one
// age format every command in this package shares (forge status, forge
// status --governance). The zero time (unknown/never) renders as "" so each
// caller can supply its own contextual fallback word ("never", "unknown").
func ageString(t time.Time) string {
	if t.IsZero() {
		return ""
	}
	if days := int(time.Since(t).Hours() / 24); days > 0 {
		return fmt.Sprintf("%d days ago", days)
	}
	return "today"
}

// sizeText renders a byte count as "N B" / "N.N KB" / "N.N MB" for
// human-readable text display (JSON output keeps the raw byte count instead).
func sizeText(size int64) string {
	if size > 1024*1024 {
		return fmt.Sprintf("%.1f MB", float64(size)/(1024*1024))
	}
	if size > 1024 {
		return fmt.Sprintf("%.1f KB", float64(size)/1024)
	}
	return fmt.Sprintf("%d B", size)
}

// Check is one named health-check result, mirroring `forge doctor`'s
// [PASS]/[FAIL] markers: Detail is populated (and printed by the CLI layer)
// only when OK is false.
type Check struct {
	Name   string
	OK     bool
	Detail string
}

// Line renders the check as "[PASS] name" or "[FAIL] name — detail" (detail
// is shown only on failure), without the CLI's leading "  " indent.
func (c Check) Line() string {
	if c.OK {
		return "[PASS] " + c.Name
	}
	return fmt.Sprintf("[FAIL] %s — %s", c.Name, c.Detail)
}

// Report is the result of the non-anomaly `forge doctor` health scan.
// NoForgeDir short-circuits everything else: when true, .forge/ does not
// exist yet (first run) and Checks/YAML2JSONShimPresent are zero-valued.
type Report struct {
	NoForgeDir           bool
	Checks               []Check
	YAML2JSONShimPresent bool
}

// Run performs `forge doctor`'s health checks: .forge/ integrity, .tmp
// residue, checkpoint.json readability, checkpoint history backups,
// trace.jsonl completeness, memory.jsonl parseability, and python3
// availability. The yaml2json shim's presence is reported separately
// (YAML2JSONShimPresent) since a MISSING shim is informational, not a
// failure (the native Go YAML parser is the primary path).
func Run(root string) Report {
	dotForge := dotForgeDir(root)
	if _, err := os.Stat(dotForge); os.IsNotExist(err) {
		return Report{NoForgeDir: true}
	}
	var checks []Check
	checks = append(checks, Check{Name: ".forge/ directory exists", OK: true})
	checks = append(checks, tmpResidueCheck(dotForge))
	cpCheck, cpFound := checkpointCheck(dotForge)
	checks = append(checks, cpCheck)
	if hc := checkpointHistoryCheck(dotForge, cpFound); hc != nil {
		checks = append(checks, *hc)
	}
	checks = append(checks, traceCheck(dotForge))
	checks = append(checks, memoryCheck(dotForge))
	checks = append(checks, python3Check())
	return Report{Checks: checks, YAML2JSONShimPresent: yaml2jsonShimPresent(root)}
}

// tmpResidueCheck flags leftover *.tmp files in .forge/ (incomplete writes
// from a crashed run).
func tmpResidueCheck(dotForge string) Check {
	files, _ := filepath.Glob(filepath.Join(dotForge, "*.tmp"))
	if len(files) > 0 {
		return Check{Name: "no .tmp residue", OK: false,
			Detail: fmt.Sprintf("%d leftover temp file(s): %v", len(files), files)}
	}
	return Check{Name: "no .tmp residue", OK: true}
}

// checkpointCheck reports checkpoint.json's readability. The returned bool
// is persist.Load's raw "found" (true only once decode succeeds), needed by
// checkpointHistoryCheck to decide whether a zero-backup state is expected.
func checkpointCheck(dotForge string) (Check, bool) {
	cpPath := filepath.Join(dotForge, "checkpoint.json")
	_, found, err := persist.Load(cpPath)
	if err != nil {
		return Check{Name: "checkpoint.json", OK: false, Detail: err.Error()}, found
	}
	if found {
		return Check{Name: "checkpoint.json", OK: true, Detail: "readable"}, found
	}
	return Check{Name: "checkpoint.json", OK: true, Detail: "not present (first run)"}, found
}

// checkpointHistoryCheck reports the retain=5 backup chain — but ONLY when
// there is something to say: a backup count > 0, or a present checkpoint
// with zero backups. Neither condition holding (no checkpoint, no backups)
// emits no check at all, matching the original behavior.
func checkpointHistoryCheck(dotForge string, cpFound bool) *Check {
	if n := CheckpointHistoryCount(dotForge); n > 0 {
		c := Check{Name: "checkpoint history", OK: true, Detail: fmt.Sprintf("%d backup(s) present", n)}
		return &c
	}
	if cpFound {
		c := Check{Name: "checkpoint history", OK: true, Detail: "no backups (retain > 0 on next save creates them)"}
		return &c
	}
	return nil
}

// traceCheck reports trace.jsonl's completeness: missing/empty is fine, a
// truncated or unreadable last line is a failure.
func traceCheck(dotForge string) Check {
	tracePath := filepath.Join(dotForge, "trace.jsonl")
	st, err := os.Stat(tracePath)
	if err != nil {
		return Check{Name: "trace.jsonl", OK: true, Detail: "not present"}
	}
	if st.Size() == 0 {
		return Check{Name: "trace.jsonl", OK: true, Detail: "empty file"}
	}
	data, err := os.ReadFile(tracePath)
	if err != nil {
		return Check{Name: "trace.jsonl", OK: false, Detail: err.Error()}
	}
	lines := strings.Split(strings.TrimSpace(string(data)), "\n")
	if len(lines) == 0 || len(lines[len(lines)-1]) == 0 {
		return Check{Name: "trace.jsonl", OK: true, Detail: "empty file"}
	}
	lastLine := lines[len(lines)-1]
	if strings.HasPrefix(lastLine, "{") && strings.HasSuffix(lastLine, "}") {
		return Check{Name: "trace.jsonl", OK: true, Detail: fmt.Sprintf("%d events, last line complete", len(lines))}
	}
	return Check{Name: "trace.jsonl", OK: false, Detail: "last line may be truncated"}
}

// memoryCheck reports memory.jsonl's parseability (a missing/empty store is
// a normal cold start, not a failure).
func memoryCheck(dotForge string) Check {
	memPath := filepath.Join(dotForge, "memory.jsonl")
	entries, err := memory.Load(memPath)
	if err != nil {
		return Check{Name: "memory.jsonl", OK: false, Detail: err.Error()}
	}
	if st, statErr := os.Stat(memPath); statErr == nil && st.Size() > 0 {
		return Check{Name: "memory.jsonl", OK: true, Detail: fmt.Sprintf("%d entries", len(entries))}
	}
	return Check{Name: "memory.jsonl", OK: true, Detail: "not present (cold start)"}
}

// python3Check reports whether python3 is on PATH (required only for
// harness/check.py; yaml2json now has a native Go parser).
func python3Check() Check {
	if _, err := exec.LookPath("python3"); err != nil {
		return Check{Name: "python3 on PATH", OK: false, Detail: "required for check.py governance check"}
	}
	return Check{Name: "python3 on PATH", OK: true}
}

// yaml2jsonShimPresent reports whether the Python yaml2json fallback shim
// exists. A stat error OTHER than not-exist counts as "present" (matches the
// original: only os.IsNotExist gates the "not found" case).
func yaml2jsonShimPresent(root string) bool {
	shimPath := filepath.Join(root, "harness", "yaml2json.py")
	_, err := os.Stat(shimPath)
	return !os.IsNotExist(err)
}
