package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/tasklist"
)

// phaseOutputContract validates machine-readable planner output and every
// declared artifact postcondition after a successful command executor phase.
func phaseOutputContract(root string, wf asset.Workflow, provenance ...*artifactProvenance) func(phase, output string) error {
	if err := asset.ValidateWorkflowStructure(wf); err != nil {
		return func(_, _ string) error {
			return fmt.Errorf("output contract: invalid workflow structure: %w", err)
		}
	}
	byName := make(map[string]asset.Phase, len(wf.Phases))
	for _, p := range wf.Phases {
		byName[p.Name] = p
	}
	var prov *artifactProvenance
	if len(provenance) > 0 {
		prov = provenance[0]
	}
	return func(phase, output string) error {
		p, ok := byName[phase]
		if !ok {
			return fmt.Errorf("phase %q is not declared by workflow %q", phase, wf.Stage)
		}
		return validatePhaseOutput(root, wf.Stage, p, output, prov)
	}
}

func validatePhaseOutput(root, stage string, p asset.Phase, output string, prov *artifactProvenance) error {
	if p.Agent == "planner" {
		if _, err := tasklist.Parse(sanitizeAgentOutput(output)); err != nil {
			return fmt.Errorf("planner TASK_LIST: %w", err)
		}
	}
	if releaseValidationPhase(stage, p) {
		releaseVerdict, ok := parseReviewerVerdict(output)
		if !ok {
			return fmt.Errorf("release validation stdout has no exact binary verdict")
		}
		if len(p.Emits) != 1 {
			return fmt.Errorf("release validation must declare exactly one report")
		}
		reportVerdict, err := releaseArtifactVerdict(root, p.Emits[0])
		if err != nil {
			return err
		}
		if reportVerdict != releaseVerdict {
			return fmt.Errorf("release validation stdout verdict %s does not match report verdict %s", releaseVerdict, reportVerdict)
		}
	}
	for _, emit := range p.Emits {
		if prov == nil {
			if err := validateEmittedFile(root, emit); err != nil {
				return fmt.Errorf("phase %s emit %q: %w", p.Name, emit, err)
			}
		}
	}
	if p.WritesADR != nil && prov == nil {
		return fmt.Errorf("phase %s writes_adr requires a build-time artifact baseline", p.Name)
	}
	if prov != nil {
		if err := prov.appendEmits(p, append([]string(nil), p.Emits...)); err != nil {
			return fmt.Errorf("phase %s artifact provenance: %w", p.Name, err)
		}
	}
	return nil
}

func validateEmittedFile(root, emit string) error {
	if emit == "" || filepath.IsAbs(emit) {
		return fmt.Errorf("must be a non-empty repo-relative path")
	}
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return err
	}
	full, err := filepath.Abs(filepath.Join(rootAbs, emit))
	if err != nil {
		return err
	}
	rel, err := filepath.Rel(rootAbs, full)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return fmt.Errorf("path escapes repository")
	}
	info, err := os.Stat(full)
	if err != nil {
		return fmt.Errorf("required artifact missing: %w", err)
	}
	resolvedRoot, err := filepath.EvalSymlinks(rootAbs)
	if err != nil {
		return err
	}
	resolvedFile, err := filepath.EvalSymlinks(full)
	if err != nil {
		return err
	}
	resolvedRel, err := filepath.Rel(resolvedRoot, resolvedFile)
	if err != nil || resolvedRel == ".." ||
		strings.HasPrefix(resolvedRel, ".."+string(filepath.Separator)) {
		return fmt.Errorf("path escapes repository through a symlink")
	}
	if !info.Mode().IsRegular() {
		return fmt.Errorf("required artifact is not a regular file")
	}
	data, err := os.ReadFile(full)
	if err != nil {
		return err
	}
	if strings.TrimSpace(string(data)) == "" {
		return fmt.Errorf("required artifact is empty")
	}
	return nil
}
