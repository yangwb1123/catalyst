package appserver

import (
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

// runForgeConsoleInventoryRefreshConvergenceE2EWithToken starts a fresh
// authenticated Web/App/Mobile API client after the accepted lifecycle image
// has been atomically replaced. It checks that the v1, lossless v2, and
// composed client-instance/resource readers all observe the same owner-bound
// device revision. The test is observation-only and cannot mutate the image.
func runForgeConsoleInventoryRefreshConvergenceE2EWithToken(
	t *testing.T,
	apiURL, token, issuer, subject, tenant string,
	expectedRevision, expectedGeneration, expectedHeartbeat int,
) {
	t.Helper()
	consoleRoot := os.Getenv("SNAPLINK_CONSOLE_ROOT")
	if consoleRoot == "" {
		workingDirectory, err := os.Getwd()
		if err != nil {
			t.Fatalf("resolve Console repository: %v", err)
		}
		repoRoot := filepath.Clean(filepath.Join(workingDirectory, "..", "..", ".."))
		consoleRoot = filepath.Join(filepath.Dir(repoRoot), "workspace", "demo", "snaplink-console")
	}
	if _, err := os.Stat(filepath.Join(consoleRoot, "pubspec.yaml")); err != nil {
		t.Fatalf("SNAPLINK_CONSOLE_ROOT must name the Flutter Console repository: %v", err)
	}
	flutterBinary := os.Getenv("FLUTTER_BIN")
	if flutterBinary == "" {
		flutterBinary = "flutter"
	}
	flutterExecutable, err := exec.LookPath(flutterBinary)
	if err != nil {
		t.Fatalf("Flutter is required for inventory convergence E2E: %v", err)
	}
	input := struct {
		APIURL             string `json:"api_url"`
		AccessToken        string `json:"access_token"`
		Issuer             string `json:"issuer"`
		Subject            string `json:"subject"`
		Tenant             string `json:"tenant_id"`
		ExpectedRevision   int    `json:"expected_revision"`
		ExpectedGeneration int    `json:"expected_generation"`
		ExpectedHeartbeat  int    `json:"expected_heartbeat"`
	}{apiURL, token, issuer, subject, tenant, expectedRevision, expectedGeneration, expectedHeartbeat}
	inputJSON, err := json.Marshal(input)
	if err != nil {
		t.Fatalf("encode inventory convergence input: %v", err)
	}
	temporaryDirectory := t.TempDir()
	inputPath := filepath.Join(temporaryDirectory, "inventory-convergence-input.json")
	if err := os.WriteFile(inputPath, inputJSON, 0o600); err != nil {
		t.Fatalf("write inventory convergence input: %v", err)
	}
	flutterHome := filepath.Join(temporaryDirectory, "home")
	if err := os.Mkdir(flutterHome, 0o700); err != nil {
		t.Fatalf("create inventory convergence Flutter home: %v", err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 120*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, flutterExecutable, "test", "--no-pub", "test/forge_device_inventory_refresh_convergence_e2e_test.dart")
	command.Dir = consoleRoot
	command.Env = append(
		forgeConsoleTestEnvironment(inputPath, flutterHome),
		"FORGE_DEVICE_INVENTORY_CONVERGENCE_E2E_INPUT="+inputPath,
	)
	var stdoutBuffer, stderrBuffer boundedCLIOutput
	command.Stdout = &stdoutBuffer
	command.Stderr = &stderrBuffer
	if err := command.Run(); err != nil {
		t.Fatalf("Flutter inventory convergence E2E failed: stdout=%q stderr=%q err=%v", stdoutBuffer.String(), stderrBuffer.String(), err)
	}
	if stdoutBuffer.exceeded || stderrBuffer.exceeded {
		t.Fatalf("inventory convergence Flutter output exceeded the size limit")
	}
}
