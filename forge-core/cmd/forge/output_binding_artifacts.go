package main

import (
	"fmt"
	"sort"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/productsource"
)

func (runtime *outputBindingRuntime) presentPriorEmits(
	snapshot productsource.Snapshot, phase string,
) ([]string, error) {
	candidates := runtime.priorEmits(phase)
	selected := make([]string, 0, len(candidates))
	seen := map[string]bool{}
	for _, candidate := range candidates {
		owner, hasOwner := runtime.priorEmitOwner(phase, candidate)
		receipt, accepted := runtime.accepted[owner]
		if hasOwner && accepted {
			if !manifestHasPath(receipt.ArtifactOutputs, candidate) {
				return nil, fmt.Errorf("output binding: accepted phase %q lacks declared output %q", owner, candidate)
			}
			if !seen[candidate] {
				seen[candidate] = true
				selected = append(selected, candidate)
			}
			continue
		}
		if !seen[candidate] && sourceManifestHasPath(snapshot.Manifest, candidate) {
			seen[candidate] = true
			selected = append(selected, candidate)
		}
	}
	sort.Strings(selected)
	return selected, nil
}

func sourceManifestHasPath(manifest productsource.Manifest, want string) bool {
	index := sort.Search(len(manifest.Entries), func(index int) bool {
		return manifest.Entries[index].Path >= want
	})
	return index < len(manifest.Entries) && manifest.Entries[index].Path == want
}

func (runtime *outputBindingRuntime) verifyInputProvenance(phase string, manifest outputbinding.ArtifactManifest) error {
	for _, item := range manifest.Items {
		owner, ok := runtime.priorEmitOwner(phase, item.Path)
		if !ok {
			allowed, externalErr := runtime.allowExternalReleaseInput(phase, item)
			if externalErr != nil {
				return externalErr
			}
			if allowed {
				continue
			}
			if runtime.allowCurrentRunReworkInput(phase, item) {
				continue
			}
			return fmt.Errorf("output binding: artifact input %q has no earlier declared owner", item.Path)
		}
		receipt, ok := runtime.accepted[owner]
		if !ok || !manifestContains(receipt.ArtifactOutputs, item) {
			return fmt.Errorf("output binding: artifact input %q lacks current-run accepted provenance", item.Path)
		}
	}
	return nil
}

func (runtime *outputBindingRuntime) allowCurrentRunReworkInput(
	phase string, item outputbinding.ManifestItem,
) bool {
	reworkPath, ok := releaseReworkReport[phase]
	if !ok || item.Path != reworkPath {
		return false
	}
	owner := ""
	for _, candidate := range runtime.wf.Phases {
		for _, emit := range candidate.Emits {
			if emit == reworkPath {
				owner = candidate.Name
			}
		}
	}
	receipt, ok := runtime.accepted[owner]
	return ok && receipt.RunID == runtime.runID && manifestContains(receipt.ArtifactOutputs, item)
}

func (runtime *outputBindingRuntime) allowExternalReleaseInput(
	phase string, candidate outputbinding.ManifestItem,
) (bool, error) {
	if runtime.wf.Stage != "rollback" ||
		(phase != "rollback-planning" && phase != "rollback-plan-validation") ||
		candidate.Path != "docs/release/release-manifest.yml" {
		return false, nil
	}
	bound, err := resolveBoundChainArtifactInput(
		runtime.root, runtime.runID, "deploy", "release-planning", candidate.Path,
	)
	if err != nil {
		return false, fmt.Errorf("output binding: cross-stage release input %q: %w", candidate.Path, err)
	}
	if bound != candidate {
		return false, fmt.Errorf("output binding: cross-stage release input %q differs from its producing receipt", candidate.Path)
	}
	return true, nil
}

func (runtime *outputBindingRuntime) fixedReleasePromptInput(phase, candidate string) bool {
	spec, ok := releasePromptSpecs[phase]
	if !ok || spec.stage != runtime.wf.Stage {
		return false
	}
	paths := append(append([]string(nil), spec.required...), spec.optional...)
	if rework, exists := releaseReworkReport[phase]; exists {
		paths = append(paths, rework)
	}
	for _, allowed := range paths {
		if candidate == allowed {
			return true
		}
	}
	return false
}

func (runtime *outputBindingRuntime) priorEmitOwner(phase, path string) (string, bool) {
	owner := ""
	for _, candidate := range runtime.wf.Phases {
		if candidate.Name == phase {
			break
		}
		for _, emit := range candidate.Emits {
			if emit == path {
				owner = candidate.Name
			}
		}
	}
	return owner, owner != ""
}

func manifestContains(manifest outputbinding.ArtifactManifest, want outputbinding.ManifestItem) bool {
	index := sort.Search(len(manifest.Items), func(index int) bool {
		return manifest.Items[index].Path >= want.Path
	})
	return index < len(manifest.Items) && manifest.Items[index] == want
}

func manifestHasPath(manifest outputbinding.ArtifactManifest, want string) bool {
	index := sort.Search(len(manifest.Items), func(index int) bool {
		return manifest.Items[index].Path >= want
	})
	return index < len(manifest.Items) && manifest.Items[index].Path == want
}

func manifestPaths(manifest outputbinding.ArtifactManifest) []string {
	paths := make([]string, len(manifest.Items))
	for index, item := range manifest.Items {
		paths[index] = item.Path
	}
	return paths
}

func (runtime *outputBindingRuntime) invalidateFrom(phase string) {
	index := runtime.phaseIndex(phase)
	for candidate := range runtime.accepted {
		if runtime.phaseIndex(candidate) >= index && !runtime.reworkReceiptNeeded(phase, candidate) {
			delete(runtime.accepted, candidate)
			delete(runtime.acceptedSemantic, candidate)
		}
	}
}

func (runtime *outputBindingRuntime) reworkReceiptNeeded(target, candidate string) bool {
	path, ok := releaseReworkReport[target]
	if !ok {
		return false
	}
	owner, exists := findPhase(runtime.wf, candidate)
	if !exists {
		return false
	}
	for _, emit := range owner.Emits {
		if emit == path {
			return true
		}
	}
	return false
}

func (runtime *outputBindingRuntime) phaseIndex(name string) int {
	for index, phase := range runtime.wf.Phases {
		if phase.Name == name {
			return index
		}
	}
	return len(runtime.wf.Phases)
}

func findPhase(wf asset.Workflow, name string) (asset.Phase, bool) {
	for _, phase := range wf.Phases {
		if phase.Name == name {
			return phase, true
		}
	}
	return asset.Phase{}, false
}
