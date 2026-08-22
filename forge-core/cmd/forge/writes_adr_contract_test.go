package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
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
	fixture.write(t, "ADR-0008-new-boundary.md",
		validWritesADRDocument(t, "ADR-0008-new-boundary.md", "ADR-0008", "New Boundary"))
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
		{name: "empty file", want: "must begin", write: func(t *testing.T, f writesADRContractFixture) {
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

func TestWritesADRContractRejectsStructurallyInvalidV2(t *testing.T) {
	fixture := newWritesADRContractFixture(t, "0007-existing.md")
	fixture.write(t, "ADR-0008-plain-markdown.md", "# Plain Markdown\n")
	err := fixture.validate()
	if err == nil || !strings.Contains(err.Error(), "not valid proposed ADR v2") {
		t.Fatalf("plain Markdown error = %v", err)
	}
	records, loadErr := artifact.Load(fixture.root)
	if loadErr != nil || len(records) != 0 {
		t.Fatalf("invalid ADR records = %+v, %v; want none", records, loadErr)
	}
}

func TestWritesADRContractRejectsHardlinkArtifact(t *testing.T) {
	fixture := newWritesADRContractFixture(t, "0007-existing.md")
	target := filepath.Join(fixture.root, "outside.md")
	writeContractArtifact(t, fixture.root, "outside.md", validWritesADRDocument(
		t, "ADR-0008-linked.md", "ADR-0008", "Linked"))
	linked := filepath.Join(fixture.root, "docs", "adr", "ADR-0008-linked.md")
	if err := os.Link(target, linked); err != nil {
		t.Skipf("hardlink unavailable: %v", err)
	}
	err := fixture.validate()
	if err == nil || !strings.Contains(err.Error(), "single-link") {
		t.Fatalf("hardlink ADR error = %v", err)
	}
}

func TestWritesADRContractRejectsChangeAfterArtifactCapture(t *testing.T) {
	fixture := newWritesADRContractFixture(t, "0007-existing.md")
	name := "ADR-0008-race-boundary.md"
	fixture.write(t, name, validWritesADRDocument(t, name, "ADR-0008", "Race Boundary"))
	fixture.prov.beforeWritesADRFinalVerify = func() {
		path := filepath.Join(fixture.root, "docs", "adr", name)
		data, _ := os.ReadFile(path)
		data = []byte(strings.Replace(string(data), "This proposal is structural only.",
			"This proposal changed after capture.", 1))
		_ = os.WriteFile(path, data, 0o600)
	}
	err := fixture.validate()
	if err == nil || !strings.Contains(err.Error(), "changed after validation") {
		t.Fatalf("post-capture mutation error = %v", err)
	}
	records, loadErr := artifact.Load(fixture.root)
	if loadErr != nil || len(records) != 0 {
		t.Fatalf("post-capture mutation records = %+v, %v; want none", records, loadErr)
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

func TestWritesADRSequenceExhaustionFailsPreparation(t *testing.T) {
	fixture := newWritesADRContractFixture(t, "ADR-9999-last.md")
	attempt, err := prepareWritesADRAttempt(fixture.root, fixture.phase.WritesADR)
	if err == nil || !strings.Contains(err.Error(), "sequence space") || attempt != nil {
		t.Fatalf("exhausted ADR preparation = %+v, %v", attempt, err)
	}
	if err := validateWritesADRPreSpawn(fixture.root, "engineering", "mvp", fixture.phase.WritesADR); err == nil || !strings.Contains(err.Error(), "pre-spawn") {
		t.Fatalf("exhausted ADR pre-spawn error = %v", err)
	}
}

func TestWritesADRSequenceIgnoresMalformedNumericMarkdownNames(t *testing.T) {
	snapshot := map[string]string{
		"0007-existing.md": "legacy", "10000-not-an-adr.md": "note",
		"+9999-note.md": "note", "ADR-123-short.md": "note", "9999.md": "note",
	}
	if got := nextADRSequenceFromSnapshot(snapshot); got != 8 {
		t.Fatalf("next ADR sequence = %d, want 8", got)
	}
	snapshot["ADR-9999-last.md"] = "legacy"
	if _, err := availableADRSequence(snapshot); err == nil ||
		!strings.Contains(err.Error(), "exhausted") {
		t.Fatalf("canonical ADR-9999 exhaustion error = %v", err)
	}
}

func validWritesADRDocument(t *testing.T, name, adrID, title string) string {
	t.Helper()
	body := fmt.Sprintf(`# %s: %s

## Context
The runtime needs one deterministic proposed decision record.

## Decision
Require exact proposed-only ADR v2 bytes.

## Consequences
Malformed or changed records fail closed.

## Validation
Run strict runtime and mutation tests.

## Limitations
This proposal is structural only.
`, adrID, title)
	metadata := validWritesADRMetadata(name, adrID, title)
	metadata["body_sha256"] = writesADRTestDigest("forgeos.architecture-decision-record-body.v2\x00", []byte(body))
	blank := writesADRTestJSON(t, metadata)
	metadata["self_sha256"] = writesADRTestDigest(
		"forgeos.architecture-decision-record.v2\x00", append(append(blank, 0), []byte(body)...))
	return "---\n" + string(writesADRTestJSON(t, metadata)) + "\n---\n\n" + body
}

func validWritesADRMetadata(name, adrID, title string) map[string]any {
	return map[string]any{
		"acceptance_id": nil, "accepted_at_unix_ms": nil, "adr_id": adrID, "affected_node_ids": []string{},
		"alternatives": []map[string]any{
			{"alternative_id": "candidate-v2", "description": "Use exact v2 bytes.", "disposition": "candidate", "rationale": "It is deterministic."},
			{"alternative_id": "rejected-yaml", "description": "Use general YAML.", "disposition": "rejected", "rationale": "It is ambiguous."}},
		"api_version": "forgeos.architecture-decision-record/v2", "approver_refs": []string{"role:reviewer"},
		"assumption_claim_ids": []string{}, "body_sha256": "", "canonicalization": "forgeos.canonical-json/v1",
		"compatibility": "Legacy ADRs remain unchanged.", "consequences": []string{"New ADRs have deterministic bytes."},
		"context_claim_ids": []string{}, "decision": "Require exact proposed-only v2 documents.",
		"decision_driver_claim_ids": []string{}, "document_name": name, "evidence_record_ids": []string{},
		"expires_at_unix_ms": nil, "implementation_refs": []string{}, "kind": "ArchitectureDecisionRecord",
		"owner_refs": []string{"role:architect"}, "proposed_at_unix_ms": int64(1),
		"revisit_triggers": []map[string]any{{"condition": "An authority lifecycle is adopted.", "evidence_required": []string{"An adopted lifecycle contract."}, "trigger_id": "authority-lifecycle"}},
		"risks":            []any{}, "rollback": "Stop producing v2 files.", "rollout": "Use v2 for new proposals.",
		"scope_refs": []string{"repo:architecture"}, "self_sha256": "", "status": "proposed",
		"superseded_by": []string{}, "supersedes": []string{}, "title": title,
		"validation_plan": []map[string]any{{"description": "Run the validator.", "due_trigger": "Before completion.", "evidence_required": []string{"Passing runtime tests."}, "owner_ref": "role:architect", "success_criteria": "The validator accepts exact bytes.", "validation_id": "runtime-validation"}},
	}
}

func writesADRTestJSON(t *testing.T, value any) []byte {
	t.Helper()
	data, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return data
}

func writesADRTestDigest(prefix string, data []byte) string {
	digest := sha256.Sum256(append([]byte(prefix), data...))
	return hex.EncodeToString(digest[:])
}
