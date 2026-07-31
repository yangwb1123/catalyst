package orchestrator

import (
	"context"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
)

func TestRunNarratesEffectiveEvolveScanProfileWithoutClaimingCompletion(t *testing.T) {
	var logs []string
	wf := asset.Workflow{Stage: "evolve", Phases: []asset.Phase{
		{
			Name: "inventory", Agent: "explorer", Readonly: true, Effect: "observe",
			FeedsForward: true, ScanContract: asset.ScanContractEvolveV1,
		},
		{Name: "implement", Agent: "implementer", Effect: "mutate"},
	}}
	engine := Engine{
		Exec:       DryRunExecutor{Log: func(line string) { logs = append(logs, line) }},
		Log:        func(line string) { logs = append(logs, line) },
		ModePolicy: mode.Effective("explorer", "production"),
	}
	if err := engine.Run(wf, "explorer"); err != nil {
		t.Fatalf("dry run: %v", err)
	}
	joined := strings.Join(logs, "\n")
	for _, want := range []string{
		"scan_contract=evolve_scan_v1",
		"effective-depth=standard",
		"selected profile only",
		"requires a validated Agent report",
	} {
		if !strings.Contains(joined, want) {
			t.Errorf("logs lack %q:\n%s", want, joined)
		}
	}
}

func TestRunParallelRequiresEveryLaterPhaseAfterContractedScanWave(t *testing.T) {
	scan := asset.Phase{
		Name: "inventory", Agent: "explorer", Readonly: true, Effect: "observe",
		FeedsForward: true, ScanContract: asset.ScanContractEvolveV1,
	}
	wf := asset.Workflow{Stage: "evolve", Phases: []asset.Phase{
		scan,
		{Name: "gap", Agent: "architect", Readonly: true, Effect: "propose"},
	}}
	engine := Engine{Exec: DryRunExecutor{}}
	err := engine.RunParallel(context.Background(), wf, "engineering")
	if err == nil || !strings.Contains(err.Error(), "later dependency wave") {
		t.Fatalf("unordered parallel scan error = %v", err)
	}

	wf.Phases[1].DependsOn = []string{"inventory"}
	if err := engine.RunParallel(context.Background(), wf, "engineering"); err != nil {
		t.Fatalf("ordered parallel scan: %v", err)
	}
}
