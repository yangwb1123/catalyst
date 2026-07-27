package doctor

import (
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/persist"
)

// TestAnomaly_NoForgeDir and TestAnomaly_NoHistory cover Anomaly's two
// early-exit states, distinct from a chain that actually has snapshots.
func TestAnomaly_NoForgeDir(t *testing.T) {
	rep := Anomaly(t.TempDir())
	if !rep.NoForgeDir {
		t.Fatal("NoForgeDir = false, want true when .forge is absent")
	}
}

func TestAnomaly_NoHistory(t *testing.T) {
	root := t.TempDir()
	mustMkdotForge(t, root)
	rep := Anomaly(root)
	if rep.NoForgeDir {
		t.Fatal("NoForgeDir = true, want false once .forge exists")
	}
	if !rep.NoHistory {
		t.Fatal("NoHistory = false, want true with no checkpoint.json present")
	}
}

// TestAnomaly_WithHistory covers the normal path: one checkpoint yields one
// snapshot line and DetectAnomalies runs against it.
func TestAnomaly_WithHistory(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	cp := persist.Checkpoint{
		FormatVersion: "forgeos.checkpoint.v1",
		Workflow:      "build", Mode: "balanced", Iteration: 1, UpdatedAtUnix: time.Now().Unix(),
	}
	if err := persist.Save(filepath.Join(dotForge, "checkpoint.json"), cp, 0); err != nil {
		t.Fatalf("seed checkpoint: %v", err)
	}

	rep := Anomaly(root)
	if rep.NoForgeDir || rep.NoHistory {
		t.Fatalf("unexpected early-exit state: %+v", rep)
	}
	if len(rep.SnapshotLines) != 1 {
		t.Errorf("SnapshotLines = %v, want exactly 1", rep.SnapshotLines)
	}
}

// TestLoadCheckpointChain_OrderAndBackups covers the chain's documented
// order: current checkpoint first (chain[0]), then numbered backups.
func TestLoadCheckpointChain_OrderAndBackups(t *testing.T) {
	root := t.TempDir()
	dotForge := mustMkdotForge(t, root)
	cpPath := filepath.Join(dotForge, "checkpoint.json")
	for i := 1; i <= 3; i++ {
		cp := persist.Checkpoint{
			FormatVersion: "forgeos.checkpoint.v1",
			Workflow:      "build", Mode: "balanced", Iteration: i,
		}
		if err := persist.Save(cpPath, cp, 5); err != nil {
			t.Fatalf("seed checkpoint #%d: %v", i, err)
		}
	}

	chain := LoadCheckpointChain(root)
	if len(chain) != 3 {
		t.Fatalf("chain length = %d, want 3 (1 current + 2 backups)", len(chain))
	}
	if chain[0].Iteration != 3 {
		t.Errorf("chain[0].Iteration = %d, want 3 (most recent first)", chain[0].Iteration)
	}
}

// anomalyCase is one DetectAnomalies table row: a hand-built chain (already
// in chain[0]=latest order) and the WARN/INFO level+substring expected to
// appear (or not) in the findings.
type anomalyCase struct {
	name       string
	chain      []persist.Checkpoint
	wantLevel  string
	wantSubstr string
	wantNone   bool // when true, no finding matching wantSubstr should exist
}

func anomalyCases() []anomalyCase {
	now := time.Now().Unix()
	stale := []persist.Checkpoint{{Iteration: 1, UpdatedAtUnix: now - 10*86400}}
	stuck := []persist.Checkpoint{{Iteration: 5, UpdatedAtUnix: now}, {Iteration: 5}, {Iteration: 5}}
	jumpUp := []persist.Checkpoint{{Iteration: 2, RoadmapCompletion: 0.9, UpdatedAtUnix: now}, {Iteration: 1, RoadmapCompletion: 0.1}}
	jumpDown := []persist.Checkpoint{{Iteration: 2, RoadmapCompletion: 0.1, UpdatedAtUnix: now}, {Iteration: 1, RoadmapCompletion: 0.9}}
	dryRun := []persist.Checkpoint{{Iteration: 3, SpentUsdMicros: 0, UpdatedAtUnix: now}}
	noProgress := []persist.Checkpoint{
		{Iteration: 1, RoadmapCompletion: 0.5, UpdatedAtUnix: now}, {Iteration: 1, RoadmapCompletion: 0.5}, {Iteration: 1, RoadmapCompletion: 0.5},
	}
	healthy := []persist.Checkpoint{{Iteration: 1, RoadmapCompletion: 0.5, SpentUsdMicros: 1_000_000, UpdatedAtUnix: now}}

	return []anomalyCase{
		{name: "stale checkpoint warns", chain: stale, wantLevel: "WARN", wantSubstr: "stalled"},
		{name: "stuck iteration across 3+ snapshots warns", chain: stuck, wantLevel: "WARN", wantSubstr: "iteration stalled"},
		{name: "roadmap jump up is INFO fast-convergence", chain: jumpUp, wantLevel: "INFO", wantSubstr: "fast convergence"},
		{name: "roadmap drop is WARN regression", chain: jumpDown, wantLevel: "WARN", wantSubstr: "regression"},
		{name: "zero spend on an active run is INFO dry-run", chain: dryRun, wantLevel: "INFO", wantSubstr: "dry-run"},
		{name: "no progress across every consecutive pair warns", chain: noProgress, wantLevel: "WARN", wantSubstr: "no measurable progress"},
		{name: "healthy single fresh checkpoint has no findings", chain: healthy, wantNone: true},
	}
}

// TestDetectAnomalies exercises each of the 5 sub-detectors (stale, stuck
// iteration, roadmap jump up/down, dry-run, no-progress) via the table above,
// plus a clean-chain negative case.
func TestDetectAnomalies(t *testing.T) {
	for _, tc := range anomalyCases() {
		t.Run(tc.name, func(t *testing.T) {
			findings := DetectAnomalies(tc.chain)
			matched := false
			for _, f := range findings {
				if f.Level == tc.wantLevel && strings.Contains(f.Message, tc.wantSubstr) {
					matched = true
				}
			}
			if tc.wantNone {
				if len(findings) != 0 {
					t.Errorf("findings = %+v, want none for a healthy chain", findings)
				}
				return
			}
			if !matched {
				t.Errorf("findings = %+v, want a %s finding containing %q", findings, tc.wantLevel, tc.wantSubstr)
			}
		})
	}
}

// TestDetectAnomalies_EmptyChain covers the degenerate empty-chain input.
func TestDetectAnomalies_EmptyChain(t *testing.T) {
	if findings := DetectAnomalies(nil); len(findings) != 0 {
		t.Errorf("findings = %+v, want none for an empty chain", findings)
	}
}
