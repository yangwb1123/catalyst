package projectsnapshot

import (
	"bytes"
	"context"
	"fmt"
	"os"
)

type capturePassResult struct {
	counts   CoverageCounts
	manifest SourceManifest
	root     string
	rootInfo os.FileInfo
}

// Capture performs two complete bounded observations and emits a production
// package only when their canonical source manifests are byte-identical.
func Capture(
	ctx context.Context,
	root string,
	environment []string,
	projectID, runID string,
) (*Production, error) {
	return captureWith(ctx, root, environment, projectID, runID, nil)
}

func captureWith(
	ctx context.Context,
	root string,
	environment []string,
	projectID, runID string,
	observer captureObserver,
) (*Production, error) {
	request, err := buildRequest(projectID, runID)
	if err != nil {
		return nil, err
	}
	anchor, err := openCaptureAnchorWith(root, observer)
	if err != nil {
		return nil, err
	}
	defer anchor.close()
	first, err := capturePass(ctx, anchor, environment, observer)
	if err != nil {
		return nil, fmt.Errorf("project snapshot first pass: %w", err)
	}
	observe(observer, observeAfterFullPass, "")
	second, err := capturePass(ctx, anchor, environment, observer)
	if err != nil {
		return nil, fmt.Errorf("project snapshot second pass: %w", err)
	}
	if err := anchor.verify(); err != nil {
		return nil, err
	}
	if first.root != second.root || !stableDirectory(first.rootInfo, second.rootInfo) ||
		!stableDirectory(anchor.identity, second.rootInfo) ||
		!equalManifests(first.manifest, second.manifest) || first.counts != second.counts {
		return nil, fmt.Errorf("project snapshot full capture passes differ")
	}
	return buildProduction(request, second.manifest, second.counts)
}

func capturePass(ctx context.Context, anchor *treeRoot, environment []string,
	observer captureObserver) (capturePassResult, error) {
	repository, err := openGitRepositoryRoot(anchor)
	if err != nil {
		return capturePassResult{}, err
	}
	defer repository.close()
	git, err := openObservedGit(ctx, environment, observer)
	if err != nil {
		return capturePassResult{}, err
	}
	defer git.close()
	if err := anchor.verify(); err != nil {
		return capturePassResult{}, err
	}
	root, err := canonicalRepositoryRoot(ctx, anchor.path, repository, git, environment)
	if err != nil {
		return capturePassResult{}, err
	}
	inventory, err := enumerateInventory(ctx, repository, git, environment)
	if err != nil {
		return capturePassResult{}, err
	}
	tree, err := openTreeRoot(root)
	if err != nil {
		return capturePassResult{}, err
	}
	defer tree.close()
	if !stableDirectory(anchor.identity, tree.identity) {
		return capturePassResult{}, fmt.Errorf("repository root differs from capture anchor")
	}
	result, err := inspectRecords(ctx, tree, inventory, observer)
	if err != nil {
		return capturePassResult{}, err
	}
	if err := git.verify(ctx); err != nil {
		return capturePassResult{}, err
	}
	manifest, err := buildManifest(inventory, git, result)
	if err != nil {
		return capturePassResult{}, err
	}
	if err := anchor.verify(); err != nil {
		return capturePassResult{}, err
	}
	return capturePassResult{
		counts: result.counts, manifest: manifest, root: root, rootInfo: tree.identity,
	}, nil
}

func equalManifests(first, second SourceManifest) bool {
	firstJSON, firstErr := canonicalJSON(first, maxManifestBytes)
	secondJSON, secondErr := canonicalJSON(second, maxManifestBytes)
	return firstErr == nil && secondErr == nil && bytes.Equal(firstJSON, secondJSON)
}

func buildManifest(
	inventory gitInventory,
	git *observedGit,
	inspection inspection,
) (SourceManifest, error) {
	entryDigests := make([]string, len(inspection.entries))
	for index, entry := range inspection.entries {
		entryDigests[index] = entry.EntrySHA256
	}
	exclusionDigests := make([]string, len(inspection.exclusions))
	for index, exclusion := range inspection.exclusions {
		exclusionDigests[index] = exclusion.ExclusionSHA256
	}
	entrySet, err := setDigest(entrySetDomain, entryDigests)
	if err != nil {
		return SourceManifest{}, err
	}
	exclusionSet, err := setDigest(exclusionSetDomain, exclusionDigests)
	if err != nil {
		return SourceManifest{}, err
	}
	manifest := SourceManifest{
		APIVersion: manifestVersion, Canonicalization: canonicalization,
		Entries: inspection.entries, EntrySetSHA256: entrySet,
		Excluded: inspection.exclusions, ExclusionSetSHA256: exclusionSet,
		GitObserver: GitObserver{
			ExecutableBytes: git.bytes, ExecutableSHA256: git.sha256,
			IdentityAttestation: gitIdentityAttestation, LocalConfigIsolation: gitLocalConfigIsolation,
			NetworkContainment: gitNetworkContainment, Version: inventory.version,
		},
		IgnoredPathCount: inventory.ignored, PathPolicyID: pathPolicyID, ProfileID: profileID,
		SourceRevision: inventory.revision, UniverseCount: int64(len(inventory.records)),
	}
	if err := sealManifest(&manifest); err != nil {
		return SourceManifest{}, err
	}
	return manifest, nil
}

func sealManifest(value *SourceManifest) error {
	candidate := cloneManifest(*value)
	candidate.SourceManifestSHA256 = ""
	digest, err := domainDigest(manifestDomain, candidate, maxManifestBytes)
	if err != nil {
		return err
	}
	candidate.SourceManifestSHA256 = digest
	if _, err := canonicalJSON(candidate, maxManifestBytes); err != nil {
		return err
	}
	*value = candidate
	return nil
}

func buildRequest(projectID, runID string) (Request, error) {
	if err := validateIdentifier("project_id", projectID); err != nil {
		return Request{}, err
	}
	if err := validateIdentifier("run_id", runID); err != nil {
		return Request{}, err
	}
	value := Request{
		APIVersion: requestVersion, Canonicalization: canonicalization,
		ExtractorID: extractorID, ExtractorVersion: extractorVersion,
		PathPolicyID: pathPolicyID, ProfileID: profileID, ProjectID: projectID, RunID: runID,
	}
	digest, err := domainDigest(requestDomain, value, maxManifestBytes)
	value.RequestSHA256 = digest
	return value, err
}
