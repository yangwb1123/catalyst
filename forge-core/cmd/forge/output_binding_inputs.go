package main

import (
	"context"
	"fmt"
	"sort"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/productsource"
)

func (runtime *outputBindingRuntime) promptEmitBlocks(phase asset.Phase) ([]string, bool) {
	if runtime == nil {
		return nil, false
	}
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	attempt, ok := runtime.pending[phase.Name]
	if !ok {
		return nil, true
	}
	return append([]string(nil), attempt.artifactInputBlocks...), true
}

func (runtime *outputBindingRuntime) capturePromptInputSet(
	snapshot productsource.Snapshot, paths []string,
) (outputbinding.ArtifactManifest, []string, error) {
	files, err := productsource.ReadSingleLinkDeclaredFiles(
		context.Background(), snapshot, paths, productsource.RegularReadLimits{
			MaxFiles: 4_096, MaxFileBytes: 64 << 20,
			MaxTotalBytes: 512 << 20, MaxPathDepth: 128,
		},
	)
	if err != nil {
		return outputbinding.ArtifactManifest{}, nil, err
	}
	items := make([]outputbinding.ManifestItem, 0, len(files))
	blocks := make([]string, 0, len(files))
	for _, file := range files {
		digest := outputbinding.SHA256(file.Content)
		if digest != file.SHA256 {
			return outputbinding.ArtifactManifest{}, nil,
				fmt.Errorf("stable artifact input %q digest mismatch", file.Path)
		}
		items = append(items, outputbinding.ManifestItem{
			Bytes: int64(len(file.Content)), Path: file.Path, SHA256: digest,
		})
		content := strings.TrimSpace(string(file.Content))
		if content != "" {
			blocks = append(blocks, contextMarker("emit:"+file.Path, content))
		}
	}
	manifest, err := outputbinding.SealManifest(items)
	if err != nil {
		return outputbinding.ArtifactManifest{}, nil, err
	}
	return manifest, blocks, nil
}

// bindFrozenReleaseInputs makes the artifact-input projection exactly match
// the files that releasePromptCache actually read and injected. The cache
// freezes these digests before Build; the retained product snapshot supplies a
// stable, single-link reread even though docs/release is excluded from the
// product-source digest itself.
func (runtime *outputBindingRuntime) bindFrozenReleaseInputs(
	attempt *outputBindingAttempt, frozen map[string]string,
) error {
	paths := make([]string, 0, len(frozen))
	for candidate := range frozen {
		if !runtime.fixedReleasePromptInput(attempt.phase.Name, candidate) {
			return fmt.Errorf("unexpected fixed release prompt input %q", candidate)
		}
		paths = append(paths, candidate)
	}
	sort.Strings(paths)
	manifest, err := runtime.captureArtifacts(attempt.sourceBefore, paths)
	if err != nil {
		return fmt.Errorf("capture fixed release prompt inputs: %w", err)
	}
	for _, item := range manifest.Items {
		if frozen[item.Path] != item.SHA256 {
			return fmt.Errorf("fixed release prompt input %q differs from injected bytes", item.Path)
		}
	}
	attempt.artifactInputPaths = paths
	attempt.artifactInputs = manifest
	attempt.fixedReleaseInputDigests = cloneReleaseInputDigests(frozen)
	return runtime.verifyInputProvenance(attempt.phase.Name, manifest)
}

// validateCurrentArtifactInputs rechecks every input that was not deliberately
// overwritten by this phase. Input/output overlap is common for release
// planning: the prompt contains the prior file while the accepted output is its
// replacement. The old bytes remain content-addressed in the receipt and
// cannot truthfully be reconstructed from the live postflight tree.
func (runtime *outputBindingRuntime) validateCurrentArtifactInputs(
	snapshot productsource.Snapshot,
	frozen outputbinding.ArtifactManifest,
	outputPaths []string,
) error {
	outputs := make(map[string]bool, len(outputPaths))
	for _, candidate := range outputPaths {
		outputs[candidate] = true
	}
	want := make([]outputbinding.ManifestItem, 0, len(frozen.Items))
	paths := make([]string, 0, len(frozen.Items))
	for _, item := range frozen.Items {
		if outputs[item.Path] {
			continue
		}
		want = append(want, item)
		paths = append(paths, item.Path)
	}
	current, err := runtime.captureArtifacts(snapshot, paths)
	if err != nil {
		return err
	}
	if !sameManifestItems(current.Items, want) {
		return fmt.Errorf("non-output input bytes changed")
	}
	return nil
}

func sameManifestItems(first, second []outputbinding.ManifestItem) bool {
	if len(first) != len(second) {
		return false
	}
	for index := range first {
		if first[index] != second[index] {
			return false
		}
	}
	return true
}
