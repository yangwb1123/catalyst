package localcommandobservationproducer

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	commandcontract "forgeos/forge-core/internal/commandobservationevidencecontract"
)

func TestBuildProductionSealsExactProfilesAndObservation(t *testing.T) {
	prepared, _ := productionFixture(t)
	production, err := buildProduction(context.Background(), prepared, "run-1", successfulCapture())
	if err != nil {
		t.Fatal(err)
	}
	packageValue := production.Package()
	if production.Result() != ObservedLocalProcess || packageValue.APIVersion != ProductionAPIVersion {
		t.Fatalf("unexpected production result: %#v", production)
	}
	observation := packageValue.Observation
	if err := commandcontract.ValidateObservation(observation); err != nil {
		t.Fatalf("invalid command observation: %v", err)
	}
	if observation.Command.CWD != "." || observation.Command.StdinBytes != 0 ||
		observation.Command.StdinSHA256 != emptySHA256 {
		t.Fatalf("command contract drifted: %#v", observation.Command)
	}
	assertProductionDigests(t, production)
	assertProductionTopLevel(t, production.ProductionJSON())
	copyJSON := production.ProductionJSON()
	copyJSON[0] = '['
	if bytes.Equal(copyJSON, production.ProductionJSON()) {
		t.Fatal("ProductionJSON must return a defensive copy")
	}
	copyPackage := production.Package()
	copyPackage.Observation.Command.Argv[0] = "mutated"
	copyPackage.EnvironmentManifest.Variables[0].Value = "mutated"
	if copyPackage.Observation.Command.TimeoutMS != nil {
		*copyPackage.Observation.Command.TimeoutMS = 1
	}
	if copyPackage.Observation.Termination.ExitCode != nil {
		*copyPackage.Observation.Termination.ExitCode = 9
	}
	fresh := production.Package()
	if fresh.Observation.Command.Argv[0] == "mutated" ||
		fresh.EnvironmentManifest.Variables[0].Value == "mutated" ||
		(fresh.Observation.Termination.ExitCode != nil && *fresh.Observation.Termination.ExitCode == 9) {
		t.Fatal("Package must return a deep defensive copy")
	}
}

func TestBuildProductionRejectsPreparedAndPostExecutionDrift(t *testing.T) {
	t.Run("prepared command", func(t *testing.T) {
		prepared, _ := productionFixture(t)
		prepared.Command.Argv = []string{"node", "other.mjs"}
		if _, err := buildProduction(context.Background(), prepared, "run-1", successfulCapture()); err == nil {
			t.Fatal("modified prepared command was accepted")
		}
	})
	t.Run("tool", func(t *testing.T) {
		prepared, executable := productionFixture(t)
		if err := os.WriteFile(executable, []byte("changed-tool"), 0o755); err != nil {
			t.Fatal(err)
		}
		if _, err := buildProduction(context.Background(), prepared, "run-1", successfulCapture()); err == nil {
			t.Fatal("post-execution tool drift was accepted")
		}
	})
	t.Run("source", func(t *testing.T) {
		prepared, _ := productionFixture(t)
		if err := os.WriteFile(filepath.Join(prepared.Root, "tracked.txt"), []byte("changed"), 0o644); err != nil {
			t.Fatal(err)
		}
		if _, err := buildProduction(context.Background(), prepared, "run-1", successfulCapture()); err == nil {
			t.Fatal("post-execution source drift was accepted")
		}
	})
}

func TestExitedProductionCanBeExplicitlyAdaptedWithoutAuthorityPromotion(t *testing.T) {
	prepared, _ := productionFixture(t)
	production, err := buildProduction(context.Background(), prepared, "run-adapt", successfulCapture())
	if err != nil {
		t.Fatal(err)
	}
	request := commandcontract.Request{
		APIVersion: commandcontract.APIVersion,
		Binding: commandcontract.Binding{
			AggregateID: "gate-run-0051", ContextSHA256: sha256Bytes([]byte("context")),
			PolicySHA256: sha256Bytes([]byte("policy")), ProjectID: "project-catalyst",
			Scope: "project", Sensitivity: "internal", Sequence: 1,
			Subjects: []string{"gate:structural", "run:0051"}, SupersedesRecordIDs: []string{},
		},
		Canonicalization: commandcontract.Canonicalization,
		Observation:      production.Package().Observation,
	}
	adaptation, err := commandcontract.Adapt(request)
	if err != nil {
		t.Fatalf("explicit ADR-0049 adaptation failed: %v", err)
	}
	if adaptation.Result != commandcontract.AdaptedShadow || adaptation.Evidence == nil {
		t.Fatalf("adaptation promoted or omitted shadow evidence: %#v", adaptation)
	}
	if !bytes.Equal(adaptation.ObservationJSON(), production.ObservationJSON()) {
		t.Fatal("ADR-0049 adaptation did not replay the exact produced observation")
	}
	if production.Result() != ObservedLocalProcess {
		t.Fatalf("producer result changed during adaptation: %q", production.Result())
	}
}

func productionFixture(t *testing.T) (*preparedProfiles, string) {
	t.Helper()
	sanitizeFixtureEnvironment(t)
	root, _ := sourceFixture(t)
	bin := t.TempDir()
	executable := filepath.Join(bin, "node")
	if err := os.WriteFile(executable, []byte("fixture-node"), 0o755); err != nil {
		t.Fatal(err)
	}
	gitPath, err := exec.LookPath("git")
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", strings.Join([]string{bin, filepath.Dir(gitPath)}, string(filepath.ListSeparator)))
	timeout := int64(5_000)
	prepared, err := prepareProfiles(context.Background(), root, CommandGate, &timeout)
	if err != nil {
		t.Fatal(err)
	}
	return prepared, executable
}

func sanitizeFixtureEnvironment(t *testing.T) {
	t.Helper()
	for _, entry := range os.Environ() {
		name, value, ok := strings.Cut(entry, "=")
		if !ok || !validEnvironmentName(name) {
			continue
		}
		if !secretEnvironmentName(name) {
			if err := validateText("fixture environment", value, true); err != nil {
				t.Setenv(name, "")
			}
		}
	}
}

func successfulCapture() capture {
	empty := commandcontract.Stream{
		Bytes: 0, RetainedBytes: 0, RetainedSHA256: emptySHA256, SHA256: emptySHA256,
	}
	exitCode := int64(0)
	return capture{
		StartedAtUnixMS: 10, EndedAtUnixMS: 20,
		Streams:     commandcontract.Streams{Combined: empty, Stderr: empty, Stdout: empty},
		Termination: commandcontract.Termination{ExitCode: &exitCode, Kind: "exited"},
	}
}

func assertProductionDigests(t *testing.T, production *Production) {
	t.Helper()
	packageValue := production.Package()
	observation := packageValue.Observation
	_, environmentDigest, _ := digestManifest(environmentDigestDomain, packageValue.EnvironmentManifest)
	_, toolDigest, _ := digestManifest(toolDigestDomain, packageValue.ToolManifest)
	_, sourceDigest, _ := digestManifest(sourceDigestDomain, packageValue.SourceManifest)
	if observation.Command.EnvironmentSHA256 != environmentDigest ||
		observation.Command.ToolSnapshotSHA256 != toolDigest ||
		observation.Source.SourceTreeSHA256 != sourceDigest ||
		observation.Source.SourceRevision != packageValue.SourceManifest.SourceRevision {
		t.Fatalf("manifest/observation binding drifted: %#v", observation)
	}
	wantProductionDigest := domainDigest(productionDigestDomain, production.ProductionJSON())
	if production.SHA256() != wantProductionDigest {
		t.Fatalf("production digest=%s want=%s", production.SHA256(), wantProductionDigest)
	}
}

func assertProductionTopLevel(t *testing.T, encoded []byte) {
	t.Helper()
	var root map[string]json.RawMessage
	if err := json.Unmarshal(encoded, &root); err != nil {
		t.Fatal(err)
	}
	want := []string{
		"api_version", "canonicalization", "environment_manifest",
		"observation", "source_manifest", "tool_manifest",
	}
	if len(root) != len(want) {
		t.Fatalf("production top-level fields = %v", root)
	}
	for _, name := range want {
		if _, exists := root[name]; !exists {
			t.Fatalf("production omitted top-level field %q", name)
		}
	}
}
