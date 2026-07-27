package orchestrator

import (
	"context"
	"errors"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestCommandExecutor_ValidateConfigFailsBeforeBuild(t *testing.T) {
	built := false
	ex := CommandExecutor{
		ValidateConfig: func(asset.Phase, string) error {
			return errors.New("phase policy denied")
		},
		Build: func(asset.Phase, string) []string {
			built = true
			return []string{"echo", "must-not-run"}
		},
	}

	err := ex.Execute(context.Background(), asset.Phase{Name: "release"}, "balanced")
	execErr := requireExecError(t, err)
	if execErr.Kind != KindConfig {
		t.Fatalf("policy denial kind = %v, want KindConfig", execErr.Kind)
	}
	if built {
		t.Fatal("Build ran despite a phase policy denial")
	}
}
