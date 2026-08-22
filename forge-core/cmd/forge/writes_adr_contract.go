package main

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"forgeos/forge-core/internal/adrv2"
	"forgeos/forge-core/internal/artifact"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/statefs"
)

type writesADRAttempt struct {
	target         string
	resolvedTarget string
	nextSequence   int
	baseline       map[string]string
}

type writesADRValidation struct {
	path   string
	sha256 string
	size   int64
}

func (p *artifactProvenance) prepareWritesADRBuild(phase asset.Phase, attempt *artifactAttempt) {
	if phase.WritesADR == nil {
		return
	}
	prior, present := p.attempt(phase.Name)
	if present && prior.writesADR != nil && !prior.writesADRComplete {
		attempt.writesADR = prior.writesADR
		attempt.prepareErr = validateWritesADRRetryBaseline(p.root, prior.writesADR)
		return
	}
	var err error
	attempt.writesADR, err = prepareWritesADRAttempt(p.root, phase.WritesADR)
	if err != nil {
		attempt.prepareErr = err
	}
}

func appendUniqueArtifactPath(paths []string, candidate string) []string {
	result := append([]string(nil), paths...)
	for _, path := range result {
		if path == candidate {
			return result
		}
	}
	return append(result, candidate)
}

func prepareWritesADRAttempt(root string, declared *asset.WritesADR) (*writesADRAttempt, error) {
	if declared == nil {
		return nil, nil
	}
	targetDir, target, err := containedADRTarget(root, declared.Target)
	if err != nil {
		return nil, err
	}
	resolved, err := resolveExistingPrefix(targetDir)
	if err != nil {
		return nil, fmt.Errorf("resolve writes_adr baseline target: %w", err)
	}
	baseline, err := snapshotADRTree(targetDir)
	if err != nil {
		return nil, err
	}
	nextSequence, err := availableADRSequence(baseline)
	if err != nil {
		return nil, err
	}
	return &writesADRAttempt{
		target: target, resolvedTarget: resolved,
		nextSequence: nextSequence, baseline: baseline,
	}, nil
}

func validateWritesADRPreSpawn(root, runMode, lifecycle string, declared *asset.WritesADR) error {
	authorized, _ := effectiveWritesADR(root, runMode, lifecycle, declared)
	if authorized == nil {
		return nil
	}
	_, err := prepareWritesADRAttempt(root, authorized)
	if err != nil {
		return fmt.Errorf("writes_adr pre-spawn: %w", err)
	}
	return nil
}

func validateWritesADRAttempt(root string, attempt *writesADRAttempt) (writesADRValidation, error) {
	if attempt == nil {
		return writesADRValidation{}, nil
	}
	targetDir, target, err := containedADRTarget(root, attempt.target)
	if err != nil {
		return writesADRValidation{}, err
	}
	resolved, err := resolveExistingPrefix(targetDir)
	if err != nil || resolved != attempt.resolvedTarget || target != attempt.target {
		return writesADRValidation{}, fmt.Errorf("writes_adr target changed after build-time snapshot")
	}
	current, err := snapshotADRTree(targetDir)
	if err != nil {
		return writesADRValidation{}, err
	}
	added, altered := adrTreeDelta(attempt.baseline, current)
	if len(added) != 1 || len(altered) != 0 {
		return writesADRValidation{}, fmt.Errorf(
			"must create exactly one new ADR and leave the baseline unchanged (added=%v altered=%v)",
			added, altered,
		)
	}
	return validateADRCandidate(root, attempt, added[0])
}

func validateWritesADRRetryBaseline(root string, attempt *writesADRAttempt) error {
	if attempt == nil {
		return nil
	}
	targetDir, target, err := containedADRTarget(root, attempt.target)
	if err != nil {
		return err
	}
	resolved, err := resolveExistingPrefix(targetDir)
	if err != nil || resolved != attempt.resolvedTarget || target != attempt.target {
		return fmt.Errorf("writes_adr retry target changed after the original snapshot")
	}
	current, err := snapshotADRTree(targetDir)
	if err != nil {
		return err
	}
	added, altered := adrTreeDelta(attempt.baseline, current)
	if len(added) != 0 || len(altered) != 0 {
		return fmt.Errorf("writes_adr retry requires the original unchanged baseline (added=%v altered=%v)", added, altered)
	}
	return nil
}

func snapshotADRTree(dir string) (map[string]string, error) {
	info, err := os.Lstat(dir)
	if os.IsNotExist(err) {
		return map[string]string{}, nil
	}
	if err != nil {
		return nil, fmt.Errorf("inspect writes_adr target: %w", err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.IsDir() {
		return nil, fmt.Errorf("writes_adr target must be a non-symlink directory")
	}
	snapshot := make(map[string]string)
	err = filepath.WalkDir(dir, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if path == dir {
			return nil
		}
		rel, err := filepath.Rel(dir, path)
		if err != nil {
			return err
		}
		fingerprint, err := adrEntryFingerprint(path)
		if err != nil {
			return err
		}
		snapshot[filepath.ToSlash(rel)] = fingerprint
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("snapshot writes_adr target: %w", err)
	}
	return snapshot, nil
}

func adrEntryFingerprint(path string) (string, error) {
	info, err := os.Lstat(path)
	if err != nil {
		return "", err
	}
	switch {
	case info.Mode().IsRegular():
		file, err := os.Open(path)
		if err != nil {
			return "", err
		}
		defer func() { _ = file.Close() }()
		opened, err := file.Stat()
		if err != nil || !os.SameFile(info, opened) || !opened.Mode().IsRegular() ||
			opened.Mode().Perm() != info.Mode().Perm() {
			return "", fmt.Errorf("ADR tree entry changed while opening")
		}
		hasher := sha256.New()
		if _, err := io.Copy(hasher, file); err != nil {
			return "", err
		}
		after, err := os.Lstat(path)
		if err != nil || !os.SameFile(opened, after) || after.Size() != opened.Size() ||
			after.ModTime() != opened.ModTime() || after.Mode().Perm() != opened.Mode().Perm() {
			return "", fmt.Errorf("ADR tree entry changed while fingerprinting")
		}
		return fmt.Sprintf("file:%04o:%s", info.Mode().Perm(),
			hex.EncodeToString(hasher.Sum(nil))), nil
	case info.IsDir():
		return fmt.Sprintf("dir:%04o", info.Mode().Perm()), nil
	case info.Mode()&os.ModeSymlink != 0:
		target, err := os.Readlink(path)
		if err != nil {
			return "", err
		}
		return "symlink:" + target, nil
	default:
		return "special:" + info.Mode().String(), nil
	}
}

func adrTreeDelta(before, after map[string]string) (added, altered []string) {
	for path, fingerprint := range after {
		previous, existed := before[path]
		if !existed {
			added = append(added, path)
		} else if previous != fingerprint {
			altered = append(altered, "changed:"+path)
		}
	}
	for path := range before {
		if _, exists := after[path]; !exists {
			altered = append(altered, "removed:"+path)
		}
	}
	sort.Strings(added)
	sort.Strings(altered)
	return added, altered
}

func validateADRCandidate(root string, attempt *writesADRAttempt, added string) (writesADRValidation, error) {
	if strings.Contains(added, "/") || !canonicalADRName(added, attempt.nextSequence) {
		return writesADRValidation{}, fmt.Errorf(
			"new ADR %q must match ADR-%04d-<title>.md in %s",
			added, attempt.nextSequence, attempt.target,
		)
	}
	relative := attempt.target + added
	full, normalized, err := containedRepoPath(root, relative)
	if err != nil || filepath.ToSlash(normalized) != relative {
		return writesADRValidation{}, fmt.Errorf("new ADR %q is not a normalized contained path", relative)
	}
	info, err := os.Lstat(full)
	if err != nil {
		return writesADRValidation{}, fmt.Errorf("inspect new ADR %q: %w", relative, err)
	}
	if info.Mode()&os.ModeSymlink != 0 || !info.Mode().IsRegular() {
		return writesADRValidation{}, fmt.Errorf("new ADR %q must be a non-symlink regular file", relative)
	}
	data, _, present, err := statefs.ReadTracked(full, adrv2.MaxDocumentBytes)
	if err != nil {
		return writesADRValidation{}, fmt.Errorf("read new ADR %q: %w", relative, err)
	}
	if !present {
		return writesADRValidation{}, fmt.Errorf("read new ADR %q: file disappeared", relative)
	}
	if _, err := adrv2.ValidateDocument(added, data); err != nil {
		return writesADRValidation{}, fmt.Errorf("new ADR %q is not valid proposed ADR v2: %w", relative, err)
	}
	return writesADRValidation{
		path: relative, sha256: artifact.Digest(data), size: int64(len(data)),
	}, nil
}

func (p *artifactProvenance) captureAttemptEmit(
	phase asset.Phase, emit string, attempt artifactAttempt,
	meta artifact.Metadata, expected writesADRValidation,
) (artifact.Record, error) {
	if emit != expected.path {
		return p.captureEmit(phase, emit, attempt, meta)
	}
	full, normalized, err := containedRepoPath(p.root, expected.path)
	if err != nil || filepath.ToSlash(normalized) != expected.path {
		return artifact.Record{}, fmt.Errorf("writes_adr capture path is not contained")
	}
	data, _, present, err := statefs.ReadTracked(full, adrv2.MaxDocumentBytes)
	if err != nil || !present || artifact.Digest(data) != expected.sha256 ||
		int64(len(data)) != expected.size {
		return artifact.Record{}, fmt.Errorf("writes_adr bytes changed before artifact capture")
	}
	if _, err := adrv2.ValidateDocument(filepath.Base(expected.path), data); err != nil {
		return artifact.Record{}, fmt.Errorf("writes_adr capture failed v2 validation: %w", err)
	}
	return artifact.Record{
		Format: artifact.FormatV1, RunID: meta.RunID, Workflow: meta.Workflow,
		Phase: meta.Phase, Agent: meta.Agent, Model: meta.Model, Path: expected.path,
		SHA256: expected.sha256, Size: expected.size, PromptSHA256: meta.PromptSHA256,
	}, nil
}

func verifyWritesADRValidation(root string, expected writesADRValidation, record artifact.Record) error {
	if expected.path == "" {
		return nil
	}
	if record.Path != expected.path || record.SHA256 != expected.sha256 ||
		record.Size != expected.size {
		return fmt.Errorf("writes_adr artifact capture does not match validated bytes")
	}
	full, normalized, err := containedRepoPath(root, expected.path)
	if err != nil || filepath.ToSlash(normalized) != expected.path {
		return fmt.Errorf("writes_adr final path is not normalized and contained")
	}
	data, _, present, err := statefs.ReadTracked(full, adrv2.MaxDocumentBytes)
	if err != nil || !present || artifact.Digest(data) != expected.sha256 ||
		int64(len(data)) != expected.size {
		return fmt.Errorf("writes_adr bytes changed after validation")
	}
	if _, err := adrv2.ValidateDocument(filepath.Base(expected.path), data); err != nil {
		return fmt.Errorf("writes_adr final bytes failed v2 validation: %w", err)
	}
	return nil
}

func (p *artifactProvenance) validateBoundOutputManifest(
	phase asset.Phase, manifest outputbinding.ArtifactManifest,
) error {
	attempt, ok := p.attempt(phase.Name)
	if !ok {
		return fmt.Errorf("missing frozen writes_adr attempt")
	}
	if attempt.writesADR == nil {
		return nil
	}
	validation, err := validateWritesADRAttempt(p.root, attempt.writesADR)
	if err != nil {
		return err
	}
	if !attempt.writesADRComplete || validation != attempt.writesADRCommitted {
		return fmt.Errorf("receipt ADR v2 bytes differ from committed artifact provenance")
	}
	for _, item := range manifest.Items {
		if item.Path == validation.path {
			if item.SHA256 != validation.sha256 || item.Bytes != validation.size {
				return fmt.Errorf("receipt artifact output does not match validated ADR v2 bytes")
			}
			return nil
		}
	}
	return fmt.Errorf("receipt artifact outputs omit validated ADR v2")
}

func (p *artifactProvenance) verifyCapturedWritesADR(
	expected writesADRValidation, records []artifact.Record,
) error {
	if expected.path == "" {
		return nil
	}
	if p.beforeWritesADRFinalVerify != nil {
		p.beforeWritesADRFinalVerify()
	}
	for _, record := range records {
		if record.Path == expected.path {
			return verifyWritesADRValidation(p.root, expected, record)
		}
	}
	return fmt.Errorf("writes_adr capture record is missing")
}

func (p *artifactProvenance) appendVerifiedArtifactRecords(
	expected writesADRValidation, records []artifact.Record,
) error {
	if err := p.verifyCapturedWritesADR(expected, records); err != nil {
		return err
	}
	return p.store.Append(records...)
}

func (p *artifactProvenance) validateBuildPreparation(
	phase asset.Phase, _ string, argv []string,
) ([]string, error) {
	attempt, ok := p.attempt(phase.Name)
	if ok && attempt.prepareErr != nil {
		return nil, fmt.Errorf("attempt preflight: %w", attempt.prepareErr)
	}
	return argv, nil
}

func (p *artifactProvenance) markWritesADRComplete(
	phase string, generation uint64, validation writesADRValidation,
) {
	p.mu.Lock()
	defer p.mu.Unlock()
	attempt, ok := p.attempts[phase]
	if ok && attempt.generation == generation {
		attempt.writesADRComplete = true
		attempt.writesADRCommitted = validation
		p.attempts[phase] = attempt
	}
}

func canonicalADRName(name string, sequence int) bool {
	prefix := fmt.Sprintf("ADR-%04d-", sequence)
	if !strings.HasPrefix(name, prefix) || !strings.HasSuffix(name, ".md") {
		return false
	}
	title := strings.TrimSuffix(strings.TrimPrefix(name, prefix), ".md")
	return title != "" && title != "." && !strings.Contains(title, "..") &&
		safeADRRelativePath(name)
}

func nextADRSequenceFromSnapshot(snapshot map[string]string) int {
	maximum := 0
	for name := range snapshot {
		if strings.Contains(name, "/") {
			continue
		}
		if number, ok := adrSequenceNumber(name); ok && number > maximum {
			maximum = number
		}
	}
	return maximum + 1
}

func availableADRSequence(snapshot map[string]string) (int, error) {
	next := nextADRSequenceFromSnapshot(snapshot)
	if next > 9999 {
		return 0, fmt.Errorf("ADR v2 sequence space ADR-0001..ADR-9999 is exhausted")
	}
	return next, nil
}

func adrSequenceNumber(name string) (int, bool) {
	if !strings.HasSuffix(name, ".md") {
		return 0, false
	}
	name = strings.TrimSuffix(strings.TrimPrefix(name, "ADR-"), ".md")
	if len(name) < 6 || name[4] != '-' || name[5:] == "" {
		return 0, false
	}
	for _, character := range name[:4] {
		if character < '0' || character > '9' {
			return 0, false
		}
	}
	number, err := strconv.Atoi(name[:4])
	return number, err == nil && number >= 1
}
