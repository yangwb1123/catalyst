package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/artifact"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/outputbindingstore"
)

func TestWritesADRReceiptRejectsValidBytesChangedAfterProvenanceCommit(t *testing.T) {
	runtime, provenance, phase := boundWritesADRFixture(t, "engineering")
	name := "ADR-0008-bound-output.md"
	path := filepath.Join(runtime.root, "docs", "adr", name)
	valid := validWritesADRDocument(t, name, "ADR-0008", "Bound Output")
	writeContractArtifact(t, runtime.root, filepath.Join("docs", "adr", name), valid)
	if err := provenance.appendEmits(phase, nil); err != nil {
		t.Fatalf("capture valid ADR artifact: %v", err)
	}
	changed := validWritesADRDocument(t, name, "ADR-0008", "Changed Bound Output")
	if err := os.WriteFile(path, []byte(changed), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := runtime.commit(phase.Name, "done", "done", 0); err == nil ||
		!strings.Contains(err.Error(), "validate captured artifact outputs") {
		t.Fatalf("unvalidated receipt bytes error = %v", err)
	}
	if receipts, err := outputbindingstore.New(runtime.root).Load(); err != nil || len(receipts) != 0 {
		t.Fatalf("receipt journal = %+v, %v; want no receipt", receipts, err)
	}
}

func TestWritesADRDisabledConditionCommitsBoundOutputWithoutADR(t *testing.T) {
	runtime, provenance, phase := boundWritesADRFixture(t, "balanced")
	if err := provenance.appendEmits(phase, nil); err != nil {
		t.Fatalf("disabled writes_adr artifact boundary: %v", err)
	}
	if err := runtime.commit(phase.Name, "done", "done", 0); err != nil {
		t.Fatalf("disabled writes_adr receipt commit: %v", err)
	}
	receipts, err := outputbindingstore.New(runtime.root).Load()
	if err != nil || len(receipts) != 1 || len(receipts[0].ArtifactOutputs.Items) != 0 {
		t.Fatalf("disabled writes_adr receipts = %+v, %v", receipts, err)
	}
}

func boundWritesADRFixture(t *testing.T, runMode string) (
	*outputBindingRuntime, *artifactProvenance, asset.Phase,
) {
	t.Helper()
	root := t.TempDir()
	writeContractArtifact(t, root, "docs/adr/0007-existing.md", "legacy\n")
	runBindingGit(t, root, "init", "-q")
	runBindingGit(t, root, "add", "docs/adr/0007-existing.md")
	runBindingGit(t, root, "-c", "user.name=Fixture", "-c",
		"user.email=fixture@example.invalid", "commit", "-qm", "fixture")
	phase := asset.Phase{Name: "solution-architect", Agent: "architect", Readonly: true,
		WritesADR: &asset.WritesADR{Condition: "mode in [engineering, cto]", Target: "docs/adr/"}}
	wf := asset.Workflow{Stage: "design", OutputBindingContract: asset.OutputBindingContractLocalDigestV1,
		Phases: []asset.Phase{phase}}
	opts := runOpts{root: root, mode: runMode, lifecycle: "mvp",
		materiality: "materiality_not_bound", agentCmd: "agent"}
	provenance := newArtifactProvenance(root, wf.Stage, "writes-adr-bound-run")
	runtime := newOutputBindingRuntime(
		outputBindingWorkflowInfo{root: root, runID: "writes-adr-bound-run", wf: wf},
		outputBindingExecutionInfo{opts: opts, policy: mode.Effective(runMode, "mvp")},
		priorEmitsOf(wf), provenance.bindingOutputPaths,
	)
	runtime.setOutputValidator(provenance.validateBoundOutputManifest)
	if err := os.MkdirAll(filepath.Join(root, ".agent", "workflows"), 0o755); err != nil {
		t.Fatal(err)
	}
	writeBindingWorkflow(t, runtime)
	if err := runtime.prepare(phase, runMode); err != nil {
		t.Fatal(err)
	}
	effective := phase
	effective.WritesADR, _ = effectiveWritesADR(root, runMode, "mvp", phase.WritesADR)
	provenance.recordBuild(effective, "opus", "prompt", "", nil)
	runtime.recordBuild(effective, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, runMode, []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	return runtime, provenance, phase
}

func TestWritesADRRetryKeepsOriginalBaselineAndStopsBeforeSpawn(t *testing.T) {
	fixture := newWritesADRContractFixture(t, "0007-existing.md")
	fixture.write(t, "ADR-0008-failed.md", "not a valid ADR v2\n")
	fixture.prov.recordBuild(fixture.phase, "opus", "retry prompt", "", nil)
	attempt, ok := fixture.prov.attempt(fixture.phase.Name)
	if !ok || attempt.prepareErr == nil ||
		!strings.Contains(attempt.prepareErr.Error(), "original unchanged baseline") {
		t.Fatalf("retry attempt = %+v", attempt)
	}
	if _, err := fixture.prov.validateBuildPreparation(
		fixture.phase, "engineering", []string{"agent"},
	); err == nil || !strings.Contains(err.Error(), "attempt preflight") {
		t.Fatalf("retry pre-spawn validation error = %v", err)
	}
	if records, err := artifact.Load(fixture.root); err != nil || len(records) != 0 {
		t.Fatalf("failed retry records = %+v, %v", records, err)
	}
}

func TestWritesADRRetryRejectsRetargetedBaselineBeforeSpawn(t *testing.T) {
	fixture := newWritesADRContractFixture(t, "0007-existing.md")
	docs := filepath.Join(fixture.root, "docs")
	other := filepath.Join(docs, "other")
	if err := os.Mkdir(other, 0o755); err != nil {
		t.Fatal(err)
	}
	writeContractArtifact(t, fixture.root, "docs/other/0007-existing.md", "existing decision\n")
	if err := os.Rename(filepath.Join(docs, "adr"), filepath.Join(docs, "original")); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("other", filepath.Join(docs, "adr")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	fixture.prov.recordBuild(fixture.phase, "opus", "retry prompt", "", nil)
	attempt, ok := fixture.prov.attempt(fixture.phase.Name)
	if !ok || attempt.prepareErr == nil {
		t.Fatalf("retargeted retry attempt = %+v", attempt)
	}
	if _, err := fixture.prov.validateBuildPreparation(
		fixture.phase, "engineering", []string{"agent"},
	); err == nil {
		t.Fatal("retargeted writes_adr retry reached spawn boundary")
	}
}

func TestWritesADROverlappingEmitIsCapturedOnce(t *testing.T) {
	fixture := newWritesADRContractFixture(t, "0007-existing.md")
	name := "ADR-0008-overlap.md"
	fixture.phase.Emits = []string{"docs/adr/" + name}
	fixture.workflow.Phases[0].Emits = append([]string(nil), fixture.phase.Emits...)
	fixture.write(t, name, validWritesADRDocument(t, name, "ADR-0008", "Overlap"))
	if err := fixture.validate(); err != nil {
		t.Fatalf("overlapping emit: %v", err)
	}
	paths, err := fixture.prov.bindingOutputPaths(fixture.phase)
	if err != nil || len(paths) != 1 || paths[0] != "docs/adr/"+name {
		t.Fatalf("overlapping binding paths = %v, %v", paths, err)
	}
	if records, err := artifact.Load(fixture.root); err != nil || len(records) != 1 {
		t.Fatalf("overlapping records = %+v, %v", records, err)
	}
}
