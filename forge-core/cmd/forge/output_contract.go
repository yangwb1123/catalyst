package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/evolvescan"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/tasklist"
)

// parseQAVerdict extracts the strict qa_v1 handshake from a phase's raw output.
// It shares the reviewer parser's envelope isolation and exact final-non-empty-line
// rule, but accepts only the QA vocabulary and normalizes it to the two tokens the
// vendor-neutral orchestrator already understands. A missing, wrapped, malformed,
// or trailing-prose token returns ok=false; a qa_v1 phase's required-verdict seam
// turns that absence into a fail-closed run error.
func parseQAVerdict(output string) (verdict string, ok bool) {
	return parseQAVerdictForExecutor(output, false)
}

func parseQAVerdictForExecutor(output string, requireEnvelope bool) (verdict string, ok bool) {
	payload, validEnvelope := qaVerdictPayload(output, requireEnvelope)
	if !validEnvelope {
		return "", false
	}
	switch lastNonEmptyExactLine(payload) {
	case "QA_VERDICT: ACCEPTED":
		return VerdictApprove, true
	case "QA_VERDICT: REJECTED":
		return VerdictRequestChanges, true
	default:
		return "", false
	}
}

func lastNonEmptyExactLine(output string) string {
	lines := strings.Split(output, "\n")
	for i := len(lines) - 1; i >= 0; i-- {
		if strings.TrimSpace(lines[i]) != "" {
			if i < len(lines)-1 {
				return strings.TrimSuffix(lines[i], "\r")
			}
			return lines[i]
		}
	}
	return ""
}

type claudeResultEnvelope struct {
	Type    string  `json:"type"`
	Subtype string  `json:"subtype"`
	IsError *bool   `json:"is_error"`
	Result  *string `json:"result"`
}

func decodeClaudeResultEnvelope(output string) (claudeResultEnvelope, error) {
	var envelope claudeResultEnvelope
	if err := json.Unmarshal([]byte(strings.TrimSpace(output)), &envelope); err != nil {
		return envelope, err
	}
	return envelope, nil
}

func successfulClaudeResultPayload(output string) (string, error) {
	envelope, err := decodeClaudeResultEnvelope(output)
	if err != nil {
		return "", fmt.Errorf("must return one complete Claude JSON result envelope: %w", err)
	}
	if envelope.Type != "result" || envelope.Subtype != "success" ||
		envelope.IsError == nil || *envelope.IsError || envelope.Result == nil {
		return "", fmt.Errorf("envelope must be type=result, subtype=success, is_error=false, with a result")
	}
	return *envelope.Result, nil
}

// qaVerdictPayload accepts plain command output, but when output is a JSON result
// envelope it also requires the provider's complete success metadata. An
// `is_error`/failed/malformed envelope cannot certify QA merely by carrying an
// ACCEPTED string in its result field.
func qaVerdictPayload(output string, requireEnvelope bool) (string, bool) {
	envelope, err := decodeClaudeResultEnvelope(output)
	if err != nil {
		if requireEnvelope {
			return "", false
		}
		return output, true
	}
	if envelope.Result == nil {
		if requireEnvelope {
			return "", false
		}
		return output, true
	}
	if envelope.Type != "result" || envelope.Subtype != "success" ||
		envelope.IsError == nil || *envelope.IsError {
		return "", false
	}
	return *envelope.Result, true
}

// verdictContractOf resolves a phase's explicitly declared machine-verdict
// protocol. Unknown phases and omitted declarations return the empty advisory
// default; no behavior is inferred from the phase or agent name.
func verdictContractOf(wf asset.Workflow) func(name string) string {
	return func(name string) string {
		for _, p := range wf.Phases {
			if p.Name == name {
				return p.VerdictContract
			}
		}
		return ""
	}
}

func verdictContractFor(lookups []func(phase string) string, phase string) string {
	if len(lookups) == 0 || lookups[0] == nil {
		return ""
	}
	return lookups[0](phase)
}

// phaseOutputContract validates machine-readable planner output and every
// declared artifact postcondition after a successful command executor phase.
func phaseOutputContract(root string, wf asset.Workflow, provenance ...*artifactProvenance) func(phase, output string) error {
	return buildPhaseOutputContract(root, wf, "", provenance...)
}

func phaseOutputContractWithPolicy(root string, wf asset.Workflow, policy mode.Policy, provenance ...*artifactProvenance) func(phase, output string) error {
	return buildPhaseOutputContract(root, wf, policy.EvolveDepth, provenance...)
}

func buildPhaseOutputContract(root string, wf asset.Workflow, scanDepth string, provenance ...*artifactProvenance) func(phase, output string) error {
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
		return validatePhaseOutput(root, wf.Stage, p, output, scanDepth, prov)
	}
}

func validatePhaseOutput(root, stage string, p asset.Phase, output, scanDepth string, prov *artifactProvenance) error {
	if err := validateEvolveScanOutput(root, p, output, scanDepth); err != nil {
		return err
	}
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
			if err := requirePhaseEmit(root, p, emit, output); err != nil {
				return err
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

// materializeEmittedFile writes the phase output into a declared emit path
// when the agent did not create it (see validatePhaseOutput). The path must
// be repo-relative, resolve inside the repository (symlink-checked), and the
// parent directory is created on demand; the file is created 0644.
func materializeEmittedFile(root, emit, content string) error {
	full, err := resolveRepoRelative(root, emit)
	if err != nil {
		return err
	}
	if _, err := os.Lstat(full); err == nil {
		// Already present (even if empty): never overwrite — the existing
		// artifact keeps its own verdict (e.g. "required artifact is empty").
		return fmt.Errorf("emit already exists: %s", emit)
	}
	if err := os.MkdirAll(filepath.Dir(full), 0o755); err != nil {
		return err
	}
	return os.WriteFile(full, []byte(content), 0o644)
}

// resolveRepoRelative resolves a repo-relative emit path inside the root,
// rejecting absolute paths, escapes, and symlink escapes.
func resolveRepoRelative(root, emit string) (string, error) {
	if emit == "" || filepath.IsAbs(emit) {
		return "", fmt.Errorf("must be a non-empty repo-relative path")
	}
	rootAbs, err := filepath.Abs(root)
	if err != nil {
		return "", err
	}
	full, err := filepath.Abs(filepath.Join(rootAbs, emit))
	if err != nil {
		return "", err
	}
	rel, err := filepath.Rel(rootAbs, full)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("path escapes repository")
	}
	return full, nil
}

// requirePhaseEmit validates a declared emit, with the agent-agnostic
// fallback: a claude-family agent writes its declared emits itself; a generic
// agent (pi, codex, ...) often returns the report on stdout instead.
// Materialize the phase output into the DECLARED emit path (exact whitelist,
// escape-checked, never overwriting an existing file) so the artifact
// contract holds for both. Refused writes (empty output, escape, IO error)
// keep the original error — never a silent pass.
func requirePhaseEmit(root string, p asset.Phase, emit, output string) error {
	if err := validateEmittedFile(root, emit); err == nil {
		return nil
	} else if output != "" && materializeEmittedFile(root, emit, output) == nil {
		return nil
	} else {
		return fmt.Errorf("phase %s emit %q: %w", p.Name, emit, err)
	}
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

// scanContractOf resolves a phase's explicitly declared scan protocol without
// inferring from its name or agent role.
func scanContractOf(wf asset.Workflow) func(string) string {
	return func(name string) string {
		for _, phase := range wf.Phases {
			if phase.Name == name {
				return phase.ScanContract
			}
		}
		return ""
	}
}

// scanContractFor reads the second output-contract lookup passed to observeFor.
// The first lookup remains the existing verdict contract for compatibility.
func scanContractFor(lookups []func(string) string, phase string) string {
	if len(lookups) < 2 || lookups[1] == nil {
		return ""
	}
	return lookups[1](phase)
}

// appendEvolveScanPrompt adds the already-resolved effective scan profile to
// only the explicitly contracted phase.
func appendEvolveScanPrompt(text string, phase asset.Phase, depth string) string {
	if phase.ScanContract != asset.ScanContractEvolveV1 {
		return text
	}
	instructions, err := evolvescan.Instructions(depth)
	if err != nil {
		// Asset/policy validation should make this unreachable. Keeping an explicit
		// marker causes the output contract to fail closed if policy wiring drifts.
		return text + "\n\n[context:evolve-scan-policy]\nINVALID POLICY: " + err.Error()
	}
	return text + "\n\n" + instructions
}

// validateEvolveScanOutput applies the root-aware contract to an explicitly
// declared scan phase.
func validateEvolveScanOutput(root string, phase asset.Phase, output, depth string) error {
	if phase.ScanContract == "" {
		return nil
	}
	if phase.ScanContract != asset.ScanContractEvolveV1 {
		return fmt.Errorf("unsupported scan_contract %q", phase.ScanContract)
	}
	if _, err := evolvescan.Validate(root, output, depth); err != nil {
		return fmt.Errorf("%s: %w", phase.ScanContract, err)
	}
	return nil
}

// evolveScanRawOutputContract closes the provider-envelope trust boundary for
// Claude without imposing provider-specific framing on custom command executors.
// The payload itself is validated by the ordinary phase output contract.
func evolveScanRawOutputContract(wf asset.Workflow, requireEnvelope bool) func(phase, output string) error {
	contractOf := scanContractOf(wf)
	return func(phase, output string) error {
		if contractOf(phase) != asset.ScanContractEvolveV1 || !requireEnvelope {
			return nil
		}
		if _, err := successfulClaudeResultPayload(output); err != nil {
			return fmt.Errorf("%s %w", asset.ScanContractEvolveV1, err)
		}
		return nil
	}
}

func combineRawOutputContracts(contracts ...func(phase, output string) error) func(phase, output string) error {
	return func(phase, output string) error {
		for _, contract := range contracts {
			if contract == nil {
				continue
			}
			if err := contract(phase, output); err != nil {
				return err
			}
		}
		return nil
	}
}

func workflowRawOutputContract(wf asset.Workflow, agentCommand string) func(phase, output string) error {
	return combineRawOutputContracts(
		releaseRawOutputContract(wf),
		evolveScanRawOutputContract(wf, isClaudeExecutable(agentCommand)),
	)
}

// recordForwardedPhaseOutput gives a valid structured scan its own complete,
// canonical, bounded lane. A contracted scan never falls back to the historical
// 800-rune summary: live execution observes before ValidateOutput, so this error
// leaves the ledger untouched and the shared validator subsequently fails the
// phase; resume validates first, making this conversion infallible afterward.
// Other phase output retains the historical summary behavior.
func recordForwardedPhaseOutput(
	ledger *phaseOutputLedger,
	phase, output string,
	contractLookups []func(string) string,
) error {
	if scanContractFor(contractLookups, phase) == asset.ScanContractEvolveV1 {
		canonical, err := evolvescan.Canonicalize(output)
		if err != nil {
			return fmt.Errorf("%s canonical feed-forward: %w",
				asset.ScanContractEvolveV1, err)
		}
		ledger.recordExact(phase, canonical)
		return nil
	}
	ledger.record(phase, output)
	return nil
}
