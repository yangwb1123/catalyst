package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/artifact"
	"forgeos/forge-core/internal/asset"
)

type writesADRContractFixture struct {
	root     string
	phase    asset.Phase
	workflow asset.Workflow
	prov     *artifactProvenance
}

func newWritesADRContractFixture(t *testing.T, existing ...string) writesADRContractFixture {
	t.Helper()
	root := t.TempDir()
	for _, name := range existing {
		writeContractArtifact(t, root, filepath.Join("docs", "adr", name), "existing decision\n")
	}
	phase := asset.Phase{
		Name: "solution-architect", Agent: "architect",
		WritesADR: &asset.WritesADR{
			Condition: "mode in [engineering, cto]", Target: "docs/adr/",
		},
	}
	workflow := asset.Workflow{Stage: "design", Phases: []asset.Phase{phase}}
	prov := newArtifactProvenance(root, workflow.Stage, "writes-adr-run")
	prov.recordBuild(phase, "opus", "author the architecture decision", "", nil)
	return writesADRContractFixture{root: root, phase: phase, workflow: workflow, prov: prov}
}

func (f writesADRContractFixture) write(t *testing.T, name, content string) {
	t.Helper()
	writeContractArtifact(t, f.root, filepath.Join("docs", "adr", name), content)
}

func (f writesADRContractFixture) validate() error {
	return phaseOutputContract(f.root, f.workflow, f.prov)(f.phase.Name, "done")
}

func TestWritesADRContractCapturesOneFreshNumberedFile(t *testing.T) {
	fixture := newWritesADRContractFixture(t, "0007-existing.md")
	fixture.write(t, "ADR-0008-new-boundary.md", "# New boundary\n")
	if err := fixture.validate(); err != nil {
		t.Fatalf("valid writes_adr output: %v", err)
	}
	records, err := artifact.Load(fixture.root)
	if err != nil || len(records) != 1 {
		t.Fatalf("artifact records = %+v, %v; want one ADR", records, err)
	}
	if records[0].Path != "docs/adr/ADR-0008-new-boundary.md" ||
		records[0].Phase != "solution-architect" {
		t.Fatalf("ADR provenance = %+v", records[0])
	}
}

func TestWritesADRContractDisabledModeDoesNotRequireArtifact(t *testing.T) {
	fixture := newWritesADRContractFixture(t, "0007-existing.md")
	disabled := fixture.phase
	disabled.WritesADR = nil
	fixture.prov.recordBuild(disabled, "opus", "design without ADR", "", nil)
	if err := fixture.validate(); err != nil {
		t.Fatalf("policy-disabled writes_adr required an artifact: %v", err)
	}
	records, err := artifact.Load(fixture.root)
	if err != nil || len(records) != 0 {
		t.Fatalf("disabled writes_adr records = %+v, %v; want none", records, err)
	}
}

func TestWritesADRContractRejectsMissingAndPreexistingFiles(t *testing.T) {
	for _, test := range []struct {
		name     string
		existing []string
	}{
		{name: "missing", existing: []string{"0007-existing.md"}},
		{name: "preexisting canonical is stale", existing: []string{
			"0007-existing.md", "ADR-0008-stale.md",
		}},
	} {
		t.Run(test.name, func(t *testing.T) {
			fixture := newWritesADRContractFixture(t, test.existing...)
			err := fixture.validate()
			if err == nil || !strings.Contains(err.Error(), "exactly one new ADR") {
				t.Fatalf("stale/missing ADR error = %v", err)
			}
		})
	}
}

func TestWritesADRContractRejectsInvalidAttemptDeltas(t *testing.T) {
	tests := []struct {
		name  string
		write func(*testing.T, writesADRContractFixture)
		want  string
	}{
		{name: "multiple files", want: "exactly one new ADR", write: func(t *testing.T, f writesADRContractFixture) {
			f.write(t, "ADR-0008-a.md", "a")
			f.write(t, "ADR-0008-b.md", "b")
		}},
		{name: "noncanonical name", want: "must match ADR-0008", write: func(t *testing.T, f writesADRContractFixture) {
			f.write(t, "0008-not-canonical.md", "decision")
		}},
		{name: "empty file", want: "must be non-empty", write: func(t *testing.T, f writesADRContractFixture) {
			f.write(t, "ADR-0008-empty.md", " \n")
		}},
		{name: "baseline mutation", want: "leave the baseline unchanged", write: func(t *testing.T, f writesADRContractFixture) {
			f.write(t, "ADR-0008-new.md", "decision")
			f.write(t, "0007-existing.md", "tampered prior decision")
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fixture := newWritesADRContractFixture(t, "0007-existing.md")
			test.write(t, fixture)
			if err := fixture.validate(); err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("invalid ADR attempt error = %v, want %q", err, test.want)
			}
		})
	}
}

func TestWritesADRContractRejectsSymlinkOrDirectoryArtifact(t *testing.T) {
	for _, kind := range []string{"symlink", "directory"} {
		t.Run(kind, func(t *testing.T) {
			fixture := newWritesADRContractFixture(t, "0007-existing.md")
			path := filepath.Join(fixture.root, "docs", "adr", "ADR-0008-not-regular.md")
			if kind == "symlink" {
				target := filepath.Join(fixture.root, "docs", "adr", "0007-existing.md")
				if err := os.Symlink(target, path); err != nil {
					t.Skipf("symlink unavailable: %v", err)
				}
			} else if err := os.Mkdir(path, 0o700); err != nil {
				t.Fatal(err)
			}
			err := fixture.validate()
			if err == nil || !strings.Contains(err.Error(), "non-symlink regular file") {
				t.Fatalf("%s ADR error = %v", kind, err)
			}
		})
	}
}

func TestWritesADRContractNeedsBuildTimeBaseline(t *testing.T) {
	fixture := newWritesADRContractFixture(t)
	err := phaseOutputContract(fixture.root, fixture.workflow)(fixture.phase.Name, "done")
	if err == nil || !strings.Contains(err.Error(), "build-time artifact baseline") {
		t.Fatalf("missing provenance baseline error = %v", err)
	}
}
