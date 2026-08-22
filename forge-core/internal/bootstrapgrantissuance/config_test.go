package bootstrapgrantissuance

import (
	"testing"

	"forgeos/forge-core/internal/grantstate"
)

func TestInvalidConfigurationHasNoStateSideEffect(t *testing.T) {
	layout := newIssuanceLayout(t)
	mutations := []func(*Config){
		func(config *Config) { config.RepositoryRoot = "relative/repository" },
		func(config *Config) { config.AuthorityRoot += "/" },
		func(config *Config) { config.StateDir = "." },
		func(config *Config) { config.TrustRootPath = "../trust.json" },
		func(config *Config) { config.PolicyPath = config.TrustRootPath },
		func(config *Config) { config.IssuerSeedPath = "state/issuer.seed" },
		func(config *Config) { config.PinnedRootSHA256 = "ABC" },
	}
	for index, mutate := range mutations {
		config := layout.config
		mutate(&config)
		opens := 0
		deps := dependencies{clock: &fixedClock{}, openState: func(grantstate.Config) (stateSession, error) {
			opens++
			return nil, nil
		}}
		if output, err := issueWith(config, deps); output != nil || ErrorCode(err) != CodeInvalidConfig {
			t.Errorf("mutation %d = %q, %v", index, output, err)
		}
		if opens != 0 {
			t.Errorf("mutation %d opened protected state", index)
		}
	}
}

func TestMissingRuntimeDependencyFailsBeforeState(t *testing.T) {
	layout := newIssuanceLayout(t)
	if _, err := issueWith(layout.config, dependencies{}); ErrorCode(err) != CodeInvalidConfig {
		t.Fatalf("missing dependencies: %v", err)
	}
}
