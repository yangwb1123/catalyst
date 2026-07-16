package doctor

import (
	"os"
	"os/exec"
	"path/filepath"
	"time"
)

// DirReport is one governance asset directory's file count and last-modified
// age (pre-rendered via ageString; "" means empty/missing/never modified —
// the caller supplies its own "never" fallback word).
type DirReport struct {
	Label string
	Count int
	Age   string
}

// ADRCheck is one architecture decision record's on-disk implementation
// status, rendered as an emoji mark (✅ done / ❌ not started / ⚠️ partial).
type ADRCheck struct {
	Label string
	Mark  string
}

// GovernanceReport is the result of `forge status --governance`: the health
// of each governance asset directory, whether governance assets are
// evolving fast enough to approach the ADR-0003 submodule-sharing trigger,
// and each tracked ADR's implementation status.
type GovernanceReport struct {
	Dirs         []DirReport
	TotalChanges int
	Evolving     bool
	ADRs         []ADRCheck
}

// Governance inspects the governance asset directories, tallies changes in
// the last 30 days (the ADR-0003 "high churn" signal), and evaluates each
// tracked ADR's implementation status.
func Governance(root string) GovernanceReport {
	dirs := []struct{ label, path string }{
		{".agent/agents/", filepath.Join(root, ".agent", "agents")},
		{".agent/workflows/", filepath.Join(root, ".agent", "workflows")},
		{".agent/policies/", filepath.Join(root, ".agent", "policies")},
		{"harness/", filepath.Join(root, "harness")},
		{".ai/prompts/", filepath.Join(root, ".ai", "prompts")},
	}
	var rep GovernanceReport
	for _, d := range dirs {
		count, lastMod := dirStats(d.path)
		rep.Dirs = append(rep.Dirs, DirReport{Label: d.label, Count: count, Age: ageString(lastMod)})
		if !lastMod.IsZero() && time.Since(lastMod).Hours() < 30*24 {
			rep.TotalChanges += count
		}
	}
	rep.Evolving = rep.TotalChanges > 5
	rep.ADRs = []ADRCheck{
		{Label: "ADR-0001: Go 自研运行时 + 零外部依赖", Mark: checkADR0001(root)},
		{Label: "ADR-0002: Polyglot 栈分期引入 (Go ✅, ❌ Python/Rust/TS)", Mark: checkADR0002(root)},
		{Label: "ADR-0003: agent-os Submodule 共享", Mark: checkADR0003(root)},
		{Label: "ADR-0004: REVIEW 阶段 AI-SDLC 评审 (workflow ✅, uses_template ⚠️)", Mark: checkADR0004(root)},
	}
	return rep
}

// dirStats returns the number of files in a directory (non-recursive) and
// the latest modification time among them. A non-existent directory returns
// (0, zero time).
func dirStats(path string) (int, time.Time) {
	entries, err := os.ReadDir(path)
	if err != nil {
		return 0, time.Time{}
	}
	count := 0
	var latest time.Time
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		count++
		if fi, err := e.Info(); err == nil && fi.ModTime().After(latest) {
			latest = fi.ModTime()
		}
	}
	return count, latest
}

// checkADR0001 verifies forge-core still builds with zero external deps.
func checkADR0001(root string) string {
	cmd := exec.Command("go", "build", "-o", os.DevNull, "./cmd/forge")
	cmd.Dir = filepath.Join(root, "forge-core")
	if err := cmd.Run(); err != nil {
		return "❌"
	}
	return "✅"
}

// checkADR0002 checks the Go core exists and whether polyglot runtimes have
// started appearing (⚠️ advancing) or not (✅ Go-only, as designed for v1).
func checkADR0002(root string) string {
	if _, err := os.Stat(filepath.Join(root, "forge-core", "go.mod")); err != nil {
		return "❌"
	}
	if _, err := os.Stat(filepath.Join(root, "forge-ai")); err == nil {
		return "⚠️"
	}
	return "✅"
}

// checkADR0003 checks whether git submodules are configured (⚠️ possibly
// active) — the submodule sharing this ADR proposes has not started (❌).
func checkADR0003(root string) string {
	if _, err := os.Stat(filepath.Join(root, ".gitmodules")); err == nil {
		return "⚠️"
	}
	return "❌"
}

// checkADR0004 checks the review workflow exists (static field presence is
// the gate; runtime consumption of uses_template is not verified here).
func checkADR0004(root string) string {
	if _, err := os.Stat(filepath.Join(root, ".agent", "workflows", "review.yml")); err != nil {
		return "❌"
	}
	return "✅"
}
