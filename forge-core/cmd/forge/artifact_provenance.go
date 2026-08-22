package main

import (
	"fmt"
	"path"
	"sort"
	"strings"
	"sync"

	"forgeos/forge-core/internal/artifact"
	"forgeos/forge-core/internal/asset"
)

type releaseArtifactBaseline struct {
	exists bool
	sha256 string
}

type inventoryPathKind uint8

const (
	inventoryProductPath inventoryPathKind = iota
	inventoryForgeControlPath
	inventoryReleaseArtifactPath
)

type inventoryPathClassification struct {
	kind      inventoryPathKind
	canonical bool
}

func classifyInventoryPath(raw string) (inventoryPathClassification, error) {
	clean, canonical, err := cleanPortableInventoryPath(raw)
	if err != nil {
		return inventoryPathClassification{}, err
	}
	folded := foldInventoryPathASCII(clean)
	switch {
	case inventoryPathWithin(folded, ".forge"):
		return inventoryPathClassification{
			kind:      inventoryForgeControlPath,
			canonical: canonical && inventoryPathWithin(clean, ".forge"),
		}, nil
	case inventoryPathWithin(folded, "docs/release"):
		return inventoryPathClassification{
			kind:      inventoryReleaseArtifactPath,
			canonical: canonical && inventoryPathWithin(clean, "docs/release"),
		}, nil
	default:
		return inventoryPathClassification{canonical: canonical}, nil
	}
}

func cleanPortableInventoryPath(raw string) (string, bool, error) {
	if raw == "" || strings.ContainsRune(raw, 0) {
		return "", false, fmt.Errorf("source path %q is not a valid portable path", raw)
	}
	slashed := strings.ReplaceAll(raw, `\`, "/")
	clean := path.Clean(slashed)
	if path.IsAbs(clean) || clean == ".." || strings.HasPrefix(clean, "../") {
		return "", false, fmt.Errorf("source path %q escapes the repository", raw)
	}
	return clean, raw == clean, nil
}

func foldInventoryPathASCII(value string) string {
	folded := []byte(value)
	for i, char := range folded {
		if char >= 'A' && char <= 'Z' {
			folded[i] = char + ('a' - 'A')
		}
	}
	return string(folded)
}

func inventoryPathWithin(value, directory string) bool {
	return value == directory || strings.HasPrefix(value, directory+"/")
}

func sourceInventoryPathExcluded(raw string, tracked bool) (bool, error) {
	classified, err := classifyInventoryPath(raw)
	if err != nil {
		return false, err
	}
	switch classified.kind {
	case inventoryForgeControlPath:
		if tracked {
			return false, fmt.Errorf("tracked Forge control state %q is forbidden", raw)
		}
		if !classified.canonical {
			return false, fmt.Errorf("source path %q is an ambiguous portable alias of Forge control state", raw)
		}
		return true, nil
	case inventoryReleaseArtifactPath:
		if !classified.canonical {
			return false, fmt.Errorf("source path %q is an ambiguous portable alias of release artifacts", raw)
		}
		return true, nil
	default:
		if !classified.canonical {
			return false, fmt.Errorf("source path %q is not a canonical portable path", raw)
		}
		return false, nil
	}
}

type artifactAttempt struct {
	model              string
	promptSHA256       string
	sourceRevision     string
	promptInputs       map[string]string
	writesADR          *writesADRAttempt
	writesADRComplete  bool
	writesADRCommitted writesADRValidation
	approvalContext    releaseApprovalContext
	contextFrozen      bool
	generation         uint64
	baselines          map[string]releaseArtifactBaseline
	releaseTree        releaseTreeSnapshot
	prepareErr         error
}

type artifactProvenance struct {
	root                       string
	runID                      string
	workflow                   string
	agentSHA                   string
	store                      *artifact.Store
	mu                         sync.RWMutex
	attempts                   map[string]artifactAttempt
	nextID                     uint64
	beforeWritesADRFinalVerify func()
}

func newArtifactProvenance(root, workflow, runID string, releaseAgentSHA ...string) *artifactProvenance {
	provenance := &artifactProvenance{
		root: root, runID: runID, workflow: workflow, store: artifact.NewStore(root),
		attempts: make(map[string]artifactAttempt),
	}
	if len(releaseAgentSHA) > 0 {
		provenance.agentSHA = strings.ToLower(strings.TrimSpace(releaseAgentSHA[0]))
	}
	return provenance
}

func (p *artifactProvenance) recordBuild(phase asset.Phase, model, promptText, sourceRevision string, promptInputs map[string]string) {
	attempt := artifactAttempt{
		model: model, promptSHA256: artifact.Digest([]byte(promptText)),
		sourceRevision: sourceRevision, promptInputs: cloneReleaseInputDigests(promptInputs),
	}
	p.prepareWritesADRBuild(phase, &attempt)
	if releaseApprovalStage(p.workflow) && phase.Agent == "release-engineer" {
		if sourceRevision == "" {
			attempt.prepareErr = fmt.Errorf("missing prompt-frozen release source revision")
		}
		if missing := missingFrozenReleaseInput(phase, promptInputs); missing != "" &&
			attempt.prepareErr == nil {
			attempt.prepareErr = fmt.Errorf("missing prompt-frozen release input %q", missing)
		}
		attempt.baselines = make(map[string]releaseArtifactBaseline, len(phase.Emits))
		var err error
		attempt.releaseTree, err = snapshotReleaseTree(p.root)
		if err != nil && attempt.prepareErr == nil {
			attempt.prepareErr = err
		}
		if releaseValidationPhase(p.workflow, phase) {
			if err := invalidateReleaseValidationReceipt(p.root, p.workflow); err != nil && attempt.prepareErr == nil {
				attempt.prepareErr = err
			}
		}
		for _, emit := range phase.Emits {
			data, present, err := readReleaseFileBytes(p.root, emit)
			if err != nil {
				if attempt.prepareErr == nil {
					attempt.prepareErr = err
				}
				continue
			}
			attempt.baselines[emit] = releaseArtifactBaseline{
				exists: present, sha256: artifact.Digest(data),
			}
		}
	}
	p.mu.Lock()
	p.nextID++
	attempt.generation = p.nextID
	p.attempts[phase.Name] = attempt
	p.mu.Unlock()
}

func missingFrozenReleaseInput(phase asset.Phase, inputs map[string]string) string {
	spec, ok := releasePromptSpecs[phase.Name]
	if !ok {
		return phase.Name
	}
	for _, required := range spec.required {
		if inputs[required] == "" {
			return required
		}
	}
	return ""
}

func (p *artifactProvenance) appendEmits(phase asset.Phase, emits []string) error {
	releaseAttempt := releaseApprovalStage(p.workflow) && phase.Agent == "release-engineer"
	if len(emits) == 0 && phase.WritesADR == nil && !releaseAttempt {
		return nil
	}
	attempt, ok := p.attempt(phase.Name)
	if !ok {
		return fmt.Errorf("missing frozen build metadata")
	}
	if attempt.prepareErr != nil {
		return fmt.Errorf("attempt preflight: %w", attempt.prepareErr)
	}
	adrValidation, err := validateWritesADRAttempt(p.root, attempt.writesADR)
	if err != nil {
		return fmt.Errorf("writes_adr postcondition: %w", err)
	}
	if adrValidation.path != "" {
		emits = appendUniqueArtifactPath(emits, adrValidation.path)
	}
	if releaseAttempt {
		if err := p.verifyReleaseAttempt(attempt, phase.Emits); err != nil {
			return err
		}
	}
	meta := artifact.Metadata{
		RunID: p.runID, Workflow: p.workflow, Phase: phase.Name,
		Agent: phase.Agent, Model: attempt.model, PromptSHA256: attempt.promptSHA256,
	}
	records := make([]artifact.Record, 0, len(emits))
	for _, emit := range emits {
		rec, err := p.captureAttemptEmit(phase, emit, attempt, meta, adrValidation)
		if err != nil {
			return err
		}
		records = append(records, rec)
	}
	context, err := p.freezeValidationContext(phase, attempt, records)
	if err != nil {
		return err
	}
	if err := p.appendVerifiedArtifactRecords(adrValidation, records); err != nil {
		return err
	}
	p.markWritesADRComplete(phase.Name, attempt.generation, adrValidation)
	if context.SourceRevision != "" {
		return p.setApprovalContext(phase.Name, attempt.generation, context)
	}
	return nil
}

func (p *artifactProvenance) bindingOutputPaths(phase asset.Phase) ([]string, error) {
	paths := append([]string(nil), phase.Emits...)
	if phase.WritesADR == nil {
		return paths, nil
	}
	attempt, ok := p.attempt(phase.Name)
	if !ok {
		return nil, fmt.Errorf("missing frozen writes_adr attempt")
	}
	validation, err := validateWritesADRAttempt(p.root, attempt.writesADR)
	if err != nil {
		return nil, err
	}
	if validation.path != "" {
		paths = appendUniqueArtifactPath(paths, validation.path)
	}
	sort.Strings(paths)
	return paths, nil
}

func (p *artifactProvenance) verifyReleaseAttempt(attempt artifactAttempt, emits []string) error {
	if err := p.verifyFrozenSource(attempt); err != nil {
		return fmt.Errorf("post-run release boundary: %w", err)
	}
	if err := p.verifyFrozenPromptInputs(attempt, emits); err != nil {
		return fmt.Errorf("post-run release boundary: %w", err)
	}
	if err := validateReleaseTreeDelta(p.root, attempt.releaseTree, emits); err != nil {
		return fmt.Errorf("post-run release boundary: %w", err)
	}
	return nil
}

func (p *artifactProvenance) verifyFrozenPromptInputs(attempt artifactAttempt, emits []string) error {
	writable := make(map[string]bool, len(emits))
	for _, emit := range emits {
		writable[emit] = true
	}
	paths := make([]string, 0, len(attempt.promptInputs))
	for path := range attempt.promptInputs {
		paths = append(paths, path)
	}
	sort.Strings(paths)
	for _, path := range paths {
		if writable[path] {
			continue
		}
		data, present, err := readReleaseFileBytes(p.root, path)
		if err != nil {
			return err
		}
		if !present || artifact.Digest(data) != attempt.promptInputs[path] {
			return fmt.Errorf("release prompt input %q changed after prompt freeze", path)
		}
	}
	return nil
}

func (p *artifactProvenance) verifyFrozenSource(attempt artifactAttempt) error {
	if attempt.sourceRevision == "" {
		return fmt.Errorf("missing prompt-frozen release source revision")
	}
	current, err := sourceStateRevision(p.root)
	if err != nil {
		return fmt.Errorf("verify prompt-frozen product source state: %w", err)
	}
	if current != attempt.sourceRevision {
		return fmt.Errorf("product source state changed after release prompt freeze")
	}
	return nil
}

func (p *artifactProvenance) freezeValidationContext(phase asset.Phase, attempt artifactAttempt, records []artifact.Record) (releaseApprovalContext, error) {
	if !releaseValidationPhase(p.workflow, phase) {
		return releaseApprovalContext{}, nil
	}
	context, err := currentReleaseApprovalContext(p.root, p.workflow)
	if err != nil {
		return releaseApprovalContext{}, fmt.Errorf("freeze release validation context: %w", err)
	}
	if context.SourceRevision != attempt.sourceRevision {
		return releaseApprovalContext{}, fmt.Errorf("product source state changed after release prompt freeze")
	}
	if err := validateReleaseTreeDelta(p.root, attempt.releaseTree, phase.Emits); err != nil {
		return releaseApprovalContext{}, fmt.Errorf("freeze release validation context: %w", err)
	}
	if err := p.verifyCapturedRecords(records); err != nil {
		return releaseApprovalContext{}, err
	}
	if err := p.verifyFrozenPromptInputs(attempt, phase.Emits); err != nil {
		return releaseApprovalContext{}, err
	}
	if err := p.verifyFrozenSource(attempt); err != nil {
		return releaseApprovalContext{}, err
	}
	return context, nil
}

func (p *artifactProvenance) verifyCapturedRecords(records []artifact.Record) error {
	for _, record := range records {
		data, present, err := readReleaseFileBytes(p.root, record.Path)
		if err != nil {
			return fmt.Errorf("freeze release validation context: %w", err)
		}
		if !present || artifact.Digest(data) != record.SHA256 ||
			int64(len(data)) != record.Size {
			return fmt.Errorf("release artifact context changed during validation postflight")
		}
	}
	return nil
}

func (p *artifactProvenance) setApprovalContext(phase string, generation uint64, context releaseApprovalContext) error {
	p.mu.Lock()
	defer p.mu.Unlock()
	attempt, ok := p.attempts[phase]
	if !ok || attempt.generation != generation {
		return fmt.Errorf("release validation attempt changed before context commit")
	}
	attempt.approvalContext = context
	attempt.contextFrozen = true
	p.attempts[phase] = attempt
	return nil
}

func (p *artifactProvenance) captureEmit(phase asset.Phase, emit string, attempt artifactAttempt, meta artifact.Metadata) (artifact.Record, error) {
	if !releaseApprovalStage(p.workflow) || phase.Agent != "release-engineer" {
		rec, err := artifact.Capture(p.root, emit, meta)
		if err != nil {
			return artifact.Record{}, fmt.Errorf("emit %q: %w", emit, err)
		}
		return rec, nil
	}
	if err := validateReleaseWriteRoot(p.root); err != nil {
		return artifact.Record{}, fmt.Errorf("post-run release boundary: %w", err)
	}
	data, present, err := readReleaseFileBytes(p.root, emit)
	if err != nil {
		return artifact.Record{}, fmt.Errorf("emit %q: %w", emit, err)
	}
	if !present || len(strings.TrimSpace(string(data))) == 0 {
		return artifact.Record{}, fmt.Errorf("emit %q is missing or empty", emit)
	}
	currentSHA := artifact.Digest(data)
	baseline, frozen := attempt.baselines[emit]
	if !frozen {
		return artifact.Record{}, fmt.Errorf("emit %q has no frozen pre-run baseline", emit)
	}
	if baseline.exists && baseline.sha256 == currentSHA {
		return artifact.Record{}, fmt.Errorf("emit %q was not created or content-changed by this attempt", emit)
	}
	return artifact.Record{
		RunID: meta.RunID, Workflow: meta.Workflow, Phase: meta.Phase,
		Agent: meta.Agent, Model: meta.Model, Path: emit,
		SHA256: currentSHA, Size: int64(len(data)),
		PromptSHA256: meta.PromptSHA256,
	}, nil
}

func (p *artifactProvenance) writeValidationReceipt(phase asset.Phase) error {
	if !releaseValidationPhase(p.workflow, phase) {
		return fmt.Errorf("phase %q is not the validation phase for stage %q", phase.Name, p.workflow)
	}
	attempt, ok := p.attempt(phase.Name)
	if !ok {
		return fmt.Errorf("missing frozen build metadata")
	}
	if attempt.prepareErr != nil {
		return fmt.Errorf("attempt preflight: %w", attempt.prepareErr)
	}
	if !attempt.contextFrozen || attempt.approvalContext.SourceRevision == "" ||
		attempt.approvalContext.ArtifactDigest == "" {
		return fmt.Errorf("release validation artifact context was not frozen")
	}
	if err := p.verifyReceiptContext(phase, attempt); err != nil {
		return err
	}
	phaseReceipt := assetPhaseReceipt{
		Name: phase.Name, RunID: p.runID, Model: attempt.model,
		AgentSHA256: p.agentSHA, PromptSHA256: attempt.promptSHA256,
	}
	_, bound, err := loadBoundApprovalWorkflow(p.root, p.workflow)
	if err != nil {
		return fmt.Errorf("load release approval binding: %w", err)
	}
	if !bound {
		return writeReleaseValidationReceipt(p.root, p.workflow, phaseReceipt, attempt.approvalContext)
	}
	verified, err := verifyBoundApprovalContext(p.root, p.workflow)
	if err != nil {
		return fmt.Errorf("verify release approval context: %w", err)
	}
	return writeBoundReleaseValidationReceipt(
		p.root, p.workflow, phaseReceipt, attempt.approvalContext, verified,
	)
}

func (p *artifactProvenance) verifyReceiptContext(phase asset.Phase, attempt artifactAttempt) error {
	if err := p.verifyFrozenSource(attempt); err != nil {
		return err
	}
	if err := p.verifyFrozenPromptInputs(attempt, phase.Emits); err != nil {
		return fmt.Errorf("release artifact context changed after validation: %w", err)
	}
	if err := validateReleaseTreeDelta(p.root, attempt.releaseTree, phase.Emits); err != nil {
		return fmt.Errorf("release artifact context changed after validation: %w", err)
	}
	current, err := currentReleaseApprovalContext(p.root, p.workflow)
	if err != nil {
		return fmt.Errorf("verify release validation context: %w", err)
	}
	if current.SourceRevision != attempt.sourceRevision {
		return fmt.Errorf("product source state changed after release prompt freeze")
	}
	if current != attempt.approvalContext {
		return fmt.Errorf("release artifact context changed after validation")
	}
	if err := p.verifyFrozenSource(attempt); err != nil {
		return err
	}
	if err := p.verifyFrozenPromptInputs(attempt, phase.Emits); err != nil {
		return fmt.Errorf("release artifact context changed after validation: %w", err)
	}
	return nil
}

func (p *artifactProvenance) attempt(phase string) (artifactAttempt, bool) {
	p.mu.RLock()
	defer p.mu.RUnlock()
	attempt, ok := p.attempts[phase]
	return attempt, ok
}

func (p *artifactProvenance) modelFor(phase string) string {
	attempt, _ := p.attempt(phase)
	return attempt.model
}
