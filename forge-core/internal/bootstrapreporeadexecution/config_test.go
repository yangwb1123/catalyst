package bootstrapreporeadexecution

import (
	"strings"
	"testing"
)

func TestConfigRequiresClosedExplicitInputs(t *testing.T) {
	valid := validConfig()
	if err := validateConfig(valid); err != nil {
		t.Fatal(err)
	}
	mutations := []func(*Config){
		func(value *Config) { value.RepositoryRoot = "relative" },
		func(value *Config) { value.AuthorityRoot = "/authority/../authority" },
		func(value *Config) { value.StateDir = "../state" },
		func(value *Config) { value.ManifestPath = value.InvocationPath },
		func(value *Config) { value.ReceiptSeedPath = "usage/key"; value.StateDir = "usage" },
		func(value *Config) { value.PinnedIssuanceRootSHA256 = strings.Repeat("A", 64) },
		func(value *Config) { value.PinnedExecutionRootSHA256 = strings.Repeat("a", 63) },
	}
	for index, mutate := range mutations {
		candidate := valid
		mutate(&candidate)
		if err := validateConfig(candidate); err == nil {
			t.Errorf("mutation %d was accepted: %#v", index, candidate)
		}
	}
}

func validConfig() Config {
	return Config{
		RepositoryRoot: "/repository", AuthorityRoot: "/authority", StateDir: "usage",
		IssuanceTrustRootPath:  "inputs/issuance-root.json",
		IssuanceLedgerPath:     "inputs/issuance-ledger.json",
		ExecutionTrustRootPath: "inputs/execution-root.json",
		ExecutionPolicyPath:    "inputs/execution-policy.json",
		InvocationPath:         "inputs/invocation.json", ManifestPath: "inputs/manifest.json",
		ReceiptSeedPath:           "keys/execution-receipt.seed",
		PinnedIssuanceRootSHA256:  strings.Repeat("a", 64),
		PinnedExecutionRootSHA256: strings.Repeat("b", 64),
	}
}
