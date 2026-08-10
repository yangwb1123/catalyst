package gopackagedependencyobservationproducer

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"time"

	"forgeos/forge-core/internal/gitworktreesource"
	"forgeos/forge-core/internal/gopackagegraph"
)

type sourceCapture func(context.Context, string, []string) (gitworktreesource.Snapshot, error)
type regularReader func(
	context.Context,
	gitworktreesource.Snapshot,
	[]string,
	gitworktreesource.RegularReadLimits,
) ([]gitworktreesource.RegularFile, error)

type capturedInputs struct {
	analysis         *gopackagegraph.Analysis
	observedAtUnixMS int64
	parameters       ParametersManifest
	parametersSHA256 string
	source           gitworktreesource.Snapshot
}

// Produce explicitly observes one selected Go module in a local Unix Git
// worktree. Ordinary build, test, acceptance, and governance paths never call
// this function implicitly.
func Produce(
	ctx context.Context,
	repositoryRoot, moduleDirectory, runID string,
) (*Production, error) {
	if err := ensureSupportedPlatform(); err != nil {
		return nil, fmt.Errorf("local Go package dependency capture preflight: %w", err)
	}
	return produceWith(
		ctx, repositoryRoot, moduleDirectory, runID, os.Environ(), time.Now,
		gitworktreesource.Capture, gitworktreesource.ReadRegularFiles,
	)
}

func produceWith(
	ctx context.Context,
	repositoryRoot, moduleDirectory, runID string,
	environ []string,
	clock func() time.Time,
	captureSource sourceCapture,
	readRegular regularReader,
) (*Production, error) {
	if err := validateCaptureInputs(runID, clock, captureSource, readRegular); err != nil {
		return nil, fmt.Errorf("local Go package dependency capture preflight: %w", err)
	}
	environment, err := minimalSourceEnvironment(environ)
	if err != nil {
		return nil, fmt.Errorf("local Go package dependency capture preflight: %w", err)
	}
	captured, err := captureProductionInputs(
		ctx, repositoryRoot, moduleDirectory, environment, clock, captureSource, readRegular,
	)
	if err != nil {
		return nil, err
	}
	return captured.seal(runID)
}

func captureProductionInputs(
	ctx context.Context,
	repositoryRoot, moduleDirectory string,
	environment []string,
	clock func() time.Time,
	captureSource sourceCapture,
	readRegular regularReader,
) (capturedInputs, error) {
	parameters, _, parametersSHA256, err := buildParameters(moduleDirectory)
	if err != nil {
		return capturedInputs{}, err
	}
	pre, err := captureAuthorizedSource(ctx, repositoryRoot, environment, captureSource)
	if err != nil {
		return capturedInputs{}, err
	}
	plan, err := gopackagegraph.Prepare(moduleDirectory, graphSourceEntries(pre.Manifest))
	if err != nil {
		return capturedInputs{}, fmt.Errorf("prepare Go package graph: %w", err)
	}
	analysis, err := readAndAnalyze(ctx, pre, plan, readRegular)
	if err != nil {
		return capturedInputs{}, err
	}
	observedAtUnixMS := clock().UnixMilli()
	if observedAtUnixMS < 0 {
		return capturedInputs{}, fmt.Errorf("local observation clock is before Unix epoch")
	}
	post, err := captureSource(ctx, pre.Root, environment)
	if err != nil {
		return capturedInputs{}, fmt.Errorf("capture post-analysis source: %w", err)
	}
	if err := requireStableSource(pre, post); err != nil {
		return capturedInputs{}, err
	}
	return capturedInputs{
		analysis: analysis, observedAtUnixMS: observedAtUnixMS,
		parameters:       parameters,
		parametersSHA256: parametersSHA256, source: pre,
	}, nil
}

func captureAuthorizedSource(
	ctx context.Context,
	repositoryRoot string,
	environment []string,
	captureSource sourceCapture,
) (gitworktreesource.Snapshot, error) {
	authorizedRoot, err := validateRepositoryRootArgument(repositoryRoot)
	if err != nil {
		return gitworktreesource.Snapshot{}, err
	}
	pre, err := captureSource(ctx, repositoryRoot, environment)
	if err != nil {
		return gitworktreesource.Snapshot{}, fmt.Errorf("capture pre-analysis source: %w", err)
	}
	if pre.Root != repositoryRoot {
		return gitworktreesource.Snapshot{}, fmt.Errorf(
			"captured canonical repository root %q differs from authorized root %q",
			pre.Root, repositoryRoot)
	}
	currentRoot, err := os.Stat(repositoryRoot)
	if err != nil || !os.SameFile(authorizedRoot, currentRoot) {
		return gitworktreesource.Snapshot{}, fmt.Errorf(
			"authorized repository root identity changed during source capture")
	}
	return pre, nil
}

func readAndAnalyze(
	ctx context.Context,
	snapshot gitworktreesource.Snapshot,
	plan *gopackagegraph.Plan,
	readRegular regularReader,
) (*gopackagegraph.Analysis, error) {
	limits := gopackagegraph.ReadLimits()
	goMod, err := readRegular(ctx, snapshot, []string{plan.GoModPath()}, gitworktreesource.RegularReadLimits{
		MaxFiles: 1, MaxFileBytes: limits.GoModBytes,
		MaxTotalBytes: limits.GoModBytes, MaxPathDepth: 4_096,
	})
	if err != nil {
		return nil, fmt.Errorf("read selected go.mod: %w", err)
	}
	goFiles, err := readRegular(ctx, snapshot, plan.GoFilePaths(), gitworktreesource.RegularReadLimits{
		MaxFiles: limits.GoFiles, MaxFileBytes: limits.GoFileBytes,
		MaxTotalBytes: limits.AggregateParserBytes, MaxPathDepth: 4_096,
	})
	if err != nil {
		return nil, fmt.Errorf("read selected Go files: %w", err)
	}
	analysis, err := gopackagegraph.Analyze(
		ctx, plan, graphRegularFile(goMod[0]), graphRegularFiles(goFiles),
	)
	if err != nil {
		return nil, fmt.Errorf("analyze selected Go module: %w", err)
	}
	return analysis, nil
}

func (captured capturedInputs) seal(runID string) (*Production, error) {
	observation, err := captured.analysis.Observation(
		captured.observedAtUnixMS,
		gopackagegraph.Producer{
			ParametersSHA256: captured.parametersSHA256, ProducerID: ProducerID,
			ProducerType: "tool", ProducerVersion: ProducerVersion, RunID: runID,
		},
		gopackagegraph.Source{
			SourceRevision:   captured.source.Manifest.SourceRevision,
			SourceTreeSHA256: captured.source.SHA256,
		},
	)
	if err != nil {
		return nil, fmt.Errorf("bind Go package graph observation: %w", err)
	}
	return sealProduction(ProductionPackage{
		APIVersion: ProductionAPIVersion, Canonicalization: Canonicalization,
		GraphObservation: observation, ParametersManifest: captured.parameters,
		SourceManifest: gitworktreesource.CloneManifest(captured.source.Manifest),
	})
}

func validateRepositoryRootArgument(root string) (os.FileInfo, error) {
	if root == "" || !filepath.IsAbs(root) || filepath.Clean(root) != root {
		return nil, fmt.Errorf("repository root must be a nonempty clean absolute path")
	}
	canonical, err := filepath.EvalSymlinks(root)
	if err != nil {
		return nil, fmt.Errorf("resolve canonical repository root: %w", err)
	}
	if canonical != root {
		return nil, fmt.Errorf("repository root must be its canonical real path")
	}
	identity, err := os.Stat(root)
	if err != nil || !identity.IsDir() {
		return nil, fmt.Errorf("repository root must be a readable real directory")
	}
	return identity, nil
}

func validateCaptureInputs(
	runID string,
	clock func() time.Time,
	captureSource sourceCapture,
	readRegular regularReader,
) error {
	if err := ensureSupportedPlatform(); err != nil {
		return err
	}
	if !runIDPattern.MatchString(runID) {
		return fmt.Errorf("run_id is not a valid bounded identifier")
	}
	if clock == nil || captureSource == nil || readRegular == nil {
		return fmt.Errorf("capture dependencies are unavailable")
	}
	return nil
}

func requireStableSource(pre, post gitworktreesource.Snapshot) error {
	if !gitworktreesource.SameCapturedRoot(pre, post) || pre.SHA256 != post.SHA256 ||
		!reflect.DeepEqual(pre.Manifest, post.Manifest) {
		return fmt.Errorf("repository source changed during Go package dependency capture")
	}
	return nil
}
