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

func TestConfigZeroValueDeviceFabricRemainsDefaultOff(t *testing.T) {
	config := Config{ListenAddress: "127.0.0.1:7467", StateDir: t.TempDir(), Build: BuildInfo{Version: "test"}}
	if err := config.Validate(); err != nil {
		t.Fatalf("zero-value device fabric changed config validation: %v", err)
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
