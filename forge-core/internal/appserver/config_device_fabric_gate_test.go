package appserver

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/devicefabricgate"
)

func TestConfigRejectsExplicitDeviceFabricActivationWhileADRIsProposed(t *testing.T) {
	config := Config{
		ListenAddress: "127.0.0.1:7467",
		StateDir:      t.TempDir(),
		Build:         BuildInfo{Version: "test"},
		DeviceFabricActivation: &devicefabricgate.Request{
			Mode:    devicefabricgate.ModeInventory,
			ADR0039: devicefabricgate.Decision{Status: "accepted", PlanningOnly: true},
			ADR0113: devicefabricgate.Decision{Status: "proposed"},
			ADR0114: devicefabricgate.Decision{Status: "proposed"},
		},
	}
	if err := config.Validate(); err == nil || !strings.Contains(err.Error(), "device fabric activation blocked") {
		t.Fatalf("proposed device fabric config error = %v", err)
	}
}

func TestConfigRequiresExecuteActivationForLeaseRegistry(t *testing.T) {
	config := testConfig(t)
	config.DeviceExecutionLeaseRegistryFile = filepath.Join(t.TempDir(), "leases.json")
	if err := config.Validate(); err == nil || !strings.Contains(err.Error(), "requires EXECUTE device fabric activation") {
		t.Fatalf("lease registry without execute activation error=%v", err)
	}
	config = testConfig(t)
	config.DeviceExecutionPolicyRegistryFile = filepath.Join(t.TempDir(), "policy.json")
	if err := config.Validate(); err == nil || !strings.Contains(err.Error(), "requires EXECUTE device fabric activation") {
		t.Fatalf("policy registry without execute activation error=%v", err)
	}
	accepted := acceptedInventoryActivation()
	accepted.Mode = devicefabricgate.ModeExecute
	accepted.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-001", AcceptedAtUnixMS: 1}
	accepted.Evidence.RunnerIsolation = true
	accepted.Evidence.LeaseFencing = true
	accepted.Evidence.CancellationAndUncertainWork = true
	accepted.Evidence.VaultArtifactAuthorization = true
	accepted.Evidence.AuditOutbox = true
	config = testConfig(t)
	config.DeviceFabricActivation = &accepted
	config.DeviceInventoryLifecycleRegistryFile = filepath.Join(t.TempDir(), "lifecycle.json")
	config.DeviceExecutionLeaseRegistryFile = filepath.Join(t.TempDir(), "leases.json")
	config.DeviceExecutionPolicyRegistryFile = filepath.Join(t.TempDir(), "policy.json")
	config.RuntimeExecutable = filepath.Join(t.TempDir(), "forge-runtime")
	if err := os.WriteFile(config.RuntimeExecutable, []byte("runtime"), 0o700); err != nil {
		t.Fatal(err)
	}
	config.RuntimeStateDir = filepath.Join(t.TempDir(), "runtime-state")
	config.SnaplinkIssuer = "https://identity.example"
	config.SnaplinkAudience = "forge-api"
	config.ExpectedTenantID = "tenant-1"
	config.ExpectedSubjectID = "user-1"
	if err := config.Validate(); err != nil {
		t.Fatalf("accepted execute lease/policy registries rejected: %v", err)
	}
}

func TestConfigZeroValueDeviceFabricRemainsDefaultOff(t *testing.T) {
	config := Config{ListenAddress: "127.0.0.1:7467", StateDir: t.TempDir(), Build: BuildInfo{Version: "test"}}
	if err := config.Validate(); err != nil {
		t.Fatalf("zero-value device fabric changed config validation: %v", err)
	}
}

func TestConfigRunnerExecutionAuthorityRemainsClosedWithoutIndependentGate(t *testing.T) {
	config := Config{
		RunnerExecutionAuthority: &devicefabricgate.RunnerAuthorityConfig{
			Enabled:     true,
			AuthorityID: "runner-authority-1",
			Decision: devicefabricgate.Decision{
				Status: "accepted", AcceptanceID: "runner-authority-1-acceptance", AcceptedAtUnixMS: 1,
			},
		},
	}
	err := config.Validate()
	if err == nil || !strings.Contains(err.Error(), "execution lease registry file") {
		t.Fatalf("Runner authority bypassed default/off gate: %v", err)
	}
}

func TestConfigExplicitlyDisabledRunnerAuthorityRemainsOff(t *testing.T) {
	config := testConfig(t)
	config.RunnerExecutionAuthority = &devicefabricgate.RunnerAuthorityConfig{}
	if err := config.Validate(); err != nil {
		t.Fatalf("explicitly disabled Runner authority changed default-off validation: %v", err)
	}
}

func TestConfigRunnerExecutionAuthorityRejectsSharedP4Acceptance(t *testing.T) {
	activation := acceptedInventoryActivation()
	activation.Mode = devicefabricgate.ModeExecute
	activation.P4 = devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-001", AcceptedAtUnixMS: 1}
	activation.Evidence.RunnerIsolation = true
	activation.Evidence.LeaseFencing = true
	activation.Evidence.CancellationAndUncertainWork = true
	activation.Evidence.VaultArtifactAuthorization = true
	activation.Evidence.AuditOutbox = true
	config := Config{
		RuntimeExecutable:                    "/usr/local/bin/forge-runtime",
		RuntimeStateDir:                      "/tmp/forge-runtime",
		SnaplinkIssuer:                       "https://issuer.example",
		SnaplinkAudience:                     "forge",
		ExpectedTenantID:                     "tenant-1",
		ExpectedSubjectID:                    "subject-1",
		DeviceFabricActivation:               &activation,
		DeviceInventoryLifecycleRegistryFile: "/tmp/forge/lifecycle.json",
		DeviceExecutionLeaseRegistryFile:     filepath.Join(t.TempDir(), "leases.json"),
		RunnerExecutionAuthority: &devicefabricgate.RunnerAuthorityConfig{
			Enabled: true, AuthorityID: "runner-authority-1",
			Decision: devicefabricgate.Decision{Status: "accepted", AcceptanceID: "p4-001", AcceptedAtUnixMS: 1},
		},
	}
	if err := config.Validate(); err == nil || !strings.Contains(err.Error(), "runner_authority_acceptance_must_be_distinct") {
		t.Fatalf("shared P4 acceptance was accepted: %v", err)
	}
}

func TestConfigAcceptsExecutionAdmissionModeWithCompleteGate(t *testing.T) {
	accepted := acceptedInventoryActivation()
	accepted.Mode = devicefabricgate.ModeExecute
	accepted.P4 = devicefabricgate.Decision{
		Status: "accepted", AcceptanceID: "p4-acceptance", AcceptedAtUnixMS: 1,
	}
	accepted.Evidence.RunnerIsolation = true
	accepted.Evidence.LeaseFencing = true
	accepted.Evidence.CancellationAndUncertainWork = true
	accepted.Evidence.VaultArtifactAuthorization = true
	accepted.Evidence.AuditOutbox = true
	root := t.TempDir()
	runtimeExecutable := filepath.Join(root, "forge-runtime")
	if err := os.WriteFile(runtimeExecutable, []byte("runtime"), 0o700); err != nil {
		t.Fatal(err)
	}
	config := Config{
		ListenAddress:                        "127.0.0.1:7467",
		StateDir:                             filepath.Join(root, "app-state"),
		Build:                                BuildInfo{Version: "test"},
		RuntimeExecutable:                    runtimeExecutable,
		RuntimeStateDir:                      filepath.Join(root, "runtime-state"),
		SnaplinkIssuer:                       "https://identity.example",
		SnaplinkAudience:                     "forge-api",
		ExpectedTenantID:                     "tenant-slate",
		ExpectedSubjectID:                    "account-42",
		DeviceFabricActivation:               &accepted,
		DeviceInventoryLifecycleRegistryFile: filepath.Join(root, "registry.json"),
	}
	if err := config.Validate(); err != nil {
		t.Fatalf("execution admission config error=%v", err)
	}
}

func TestConfigStillRejectsMigrationAndFederationModes(t *testing.T) {
	for _, mode := range []devicefabricgate.Mode{devicefabricgate.ModeMigrate, devicefabricgate.ModeFederate} {
		t.Run(string(mode), func(t *testing.T) {
			accepted := acceptedInventoryActivation()
			accepted.Mode = mode
			accepted.P4 = devicefabricgate.Decision{
				Status: "accepted", AcceptanceID: "p4-acceptance", AcceptedAtUnixMS: 1,
			}
			accepted.Evidence.RunnerIsolation = true
			accepted.Evidence.LeaseFencing = true
			accepted.Evidence.CancellationAndUncertainWork = true
			accepted.Evidence.VaultArtifactAuthorization = true
			accepted.Evidence.AuditOutbox = true
			root := t.TempDir()
			config := Config{
				ListenAddress:                        "127.0.0.1:7467",
				StateDir:                             filepath.Join(root, "app-state"),
				Build:                                BuildInfo{Version: "test"},
				DeviceFabricActivation:               &accepted,
				DeviceInventoryLifecycleRegistryFile: filepath.Join(root, "registry.json"),
			}
			if err := config.Validate(); err == nil {
				t.Fatalf("%s mode config error=%v", mode, err)
			}
		})
	}
}
