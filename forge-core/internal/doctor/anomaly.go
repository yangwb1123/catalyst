package doctor

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"forgeos/forge-core/internal/persist"
)

// AnomalyReport is the full result of `forge doctor --anomaly`. NoForgeDir
// and NoHistory are mutually exclusive early-exit states (.forge/ absent, or
// present but with no checkpoint history); otherwise SnapshotLines holds one
// pre-rendered display row per chain entry and Findings holds the detected
// anomalies, both in LoadCheckpointChain's order.
type AnomalyReport struct {
	NoForgeDir    bool
	NoHistory     bool
	SnapshotLines []string
	Findings      []AnomalyFinding
}

// Anomaly runs `forge doctor --anomaly`'s full checkpoint-history trend
// analysis: load the chain, render each snapshot, and detect anomalies.
func Anomaly(root string) AnomalyReport {
	if _, err := os.Stat(dotForgeDir(root)); os.IsNotExist(err) {
		return AnomalyReport{NoForgeDir: true}
	}
	chain := LoadCheckpointChain(root)
	if len(chain) == 0 {
		return AnomalyReport{NoHistory: true}
	}
	lines := make([]string, len(chain))
	for i, cp := range chain {
		lines[i] = snapshotLine(i, cp)
	}
	return AnomalyReport{SnapshotLines: lines, Findings: DetectAnomalies(chain)}
}

// snapshotLine renders one checkpoint history row:
// `  [N] iter=… roadmap=…% gates=… mode=…(age)(, spent=$…)`.
func snapshotLine(i int, cp persist.Checkpoint) string {
	age := ""
	if cp.UpdatedAtUnix > 0 {
		days := int(time.Now().Unix()-cp.UpdatedAtUnix) / 86400
		if days > 0 {
			age = fmt.Sprintf(" (%d days old)", days)
		} else {
			age = " (today)"
		}
	}
	spent := ""
	if cp.SpentUsdMicros > 0 {
		spent = fmt.Sprintf(", spent=$%.4f", float64(cp.SpentUsdMicros)/1e6)
	}
	return fmt.Sprintf("  [%d] iter=%d roadmap=%.0f%% gates=%v mode=%q%s%s",
		i+1, cp.Iteration, cp.RoadmapCompletion*100, cp.GatesGreen, cp.Mode, age, spent)
}

// LoadCheckpointChain loads the checkpoint history chain: the current
// checkpoint.json (if present and readable) followed by up to 5 numbered
// backups (.1 .. .5, retain=5), in that order — chain[0] is always the most
// recent snapshot. Shared by `forge doctor --anomaly` and `forge status
// --history`, which both walk the identical chain.
func LoadCheckpointChain(root string) []persist.Checkpoint {
	cpPath := filepath.Join(dotForgeDir(root), "checkpoint.json")
	var chain []persist.Checkpoint
	if cp, found, err := persist.Load(cpPath); err == nil && found {
		chain = append(chain, cp)
	}
	for i := 1; i <= 5; i++ {
		hPath := fmt.Sprintf("%s.%d", cpPath, i)
		if hcp, hfound, herr := persist.Load(hPath); herr == nil && hfound {
			chain = append(chain, hcp)
		}
	}
	return chain
}

// AnomalyFinding is one result of DetectAnomalies, classified WARN (counts
// toward "anomalies detected") or INFO (noteworthy but not a red flag) to
// match `forge doctor --anomaly`'s output markers.
type AnomalyFinding struct {
	Level   string // "WARN" | "INFO"
	Message string
}

// DetectAnomalies runs the checkpoint-history anomaly heuristics — stale
// checkpoint, stalled iteration, rapid roadmap convergence/regression,
// dry-run detection, and no-progress runs — against an already-loaded chain
// (chain[0] = latest/current, as LoadCheckpointChain returns it). Pure: no I/O.
func DetectAnomalies(chain []persist.Checkpoint) []AnomalyFinding {
	var findings []AnomalyFinding
	if len(chain) == 0 {
		return findings
	}
	warn := func(format string, args ...any) {
		findings = append(findings, AnomalyFinding{Level: "WARN", Message: fmt.Sprintf(format, args...)})
	}
	info := func(format string, args ...any) {
		findings = append(findings, AnomalyFinding{Level: "INFO", Message: fmt.Sprintf(format, args...)})
	}
	detectStale(chain, warn)
	detectStuckIteration(chain, warn)
	detectRoadmapJump(chain, warn, info)
	detectDryRun(chain, info)
	detectNoProgress(chain, warn)
	return findings
}

// detectStale flags a checkpoint not updated in 7+ days as possibly stalled.
func detectStale(chain []persist.Checkpoint, warn func(string, ...any)) {
	latest := chain[0]
	if latest.UpdatedAtUnix > 0 && time.Now().Unix()-latest.UpdatedAtUnix > 7*86400 {
		warn("checkpoint not updated for %d days — workflow may be stalled",
			int(time.Now().Unix()-latest.UpdatedAtUnix)/86400)
	}
}

// detectStuckIteration flags an iteration count identical across every
// snapshot in a chain of 3+ as a workflow that may not be advancing.
func detectStuckIteration(chain []persist.Checkpoint, warn func(string, ...any)) {
	if len(chain) < 3 {
		return
	}
	allSame := true
	for i := 1; i < len(chain); i++ {
		if chain[i].Iteration != chain[0].Iteration {
			allSame = false
			break
		}
	}
	if allSame && chain[0].Iteration > 0 {
		warn("iteration stalled at %d across all %d checkpoints — workflow may not be advancing",
			chain[0].Iteration, len(chain))
	}
}

// detectRoadmapJump flags a >50% roadmap_completion swing between the
// oldest and newest snapshot: a gain is fast-convergence (INFO), a drop is a
// possible regression (WARN).
func detectRoadmapJump(chain []persist.Checkpoint, warn, info func(string, ...any)) {
	if len(chain) < 2 {
		return
	}
	first, last := chain[len(chain)-1], chain[0]
	jump := last.RoadmapCompletion - first.RoadmapCompletion
	if jump > 0.5 {
		info("roadmap_completion jumped from %.0f%% → %.0f%% (%.0f%% gain) — fast convergence",
			first.RoadmapCompletion*100, last.RoadmapCompletion*100, jump*100)
	}
	if jump < -0.5 {
		warn("roadmap_completion dropped from %.0f%% → %.0f%% (%.0f%% loss) — possible regression",
			first.RoadmapCompletion*100, last.RoadmapCompletion*100, jump*-100)
	}
}

// detectDryRun flags a $0 cumulative spend on an active (iteration > 0) run
// as a dry-run or echo executor (informational, not a red flag).
func detectDryRun(chain []persist.Checkpoint, info func(string, ...any)) {
	latest := chain[0]
	if latest.SpentUsdMicros == 0 && latest.Iteration > 0 {
		info("cumulative spend is $0 — dry-run or echo executor detected (no real calls billed)")
	}
}

// detectNoProgress flags a chain where EVERY consecutive pair of snapshots
// has identical (iteration, roadmap_completion) — activity with no
// measurable progress.
func detectNoProgress(chain []persist.Checkpoint, warn func(string, ...any)) {
	if len(chain) < 2 {
		return
	}
	identicalCount := 0
	for i := 0; i < len(chain)-1; i++ {
		if chain[i].Iteration == chain[i+1].Iteration &&
			chain[i].RoadmapCompletion == chain[i+1].RoadmapCompletion {
			identicalCount++
		}
	}
	if identicalCount == len(chain)-1 && identicalCount > 0 {
		warn("%d consecutive checkpoints have identical state — no measurable progress", identicalCount+1)
	}
}
