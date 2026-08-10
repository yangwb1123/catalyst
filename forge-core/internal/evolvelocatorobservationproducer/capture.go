package evolvelocatorobservationproducer

import (
	"context"
	"fmt"
	"os"
	"reflect"
	"regexp"
	"time"

	"forgeos/forge-core/internal/gitworktreesource"
)

var runIDPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)

type sourceCapture func(context.Context, string, []string) (gitworktreesource.Snapshot, error)

type capturedProductionInputs struct {
	facts            map[string]fileFact
	observedAtUnixMS int64
	occurrences      []occurrence
	parameters       ParametersManifest
	parametersSHA256 string
	reportManifest   ReportManifest
	source           gitworktreesource.Snapshot
}

// Produce explicitly captures one already-returned local Evolve report. The
// ordinary Evolve validation path never calls this function.
func Produce(
	ctx context.Context,
	root, output, expectedDepth, runID string,
) (*Production, error) {
	return produceWith(ctx, root, output, expectedDepth, runID, os.Environ(), time.Now, gitworktreesource.Capture)
}

func produceWith(
	ctx context.Context,
	root, output, expectedDepth, runID string,
	environ []string,
	clock func() time.Time,
	captureSource sourceCapture,
) (*Production, error) {
	if err := validateCaptureInputs(runID, expectedDepth, clock, captureSource); err != nil {
		return nil, fmt.Errorf("local Evolve locator capture preflight: %w", err)
	}
	environment, err := minimalSourceEnvironment(environ)
	if err != nil {
		return nil, fmt.Errorf("local Evolve locator capture preflight: %w", err)
	}
	captured, err := captureProductionInputs(
		ctx, root, output, expectedDepth, environment, clock, captureSource,
	)
	if err != nil {
		return nil, err
	}
	return captured.seal(expectedDepth, runID)
}

func captureProductionInputs(
	ctx context.Context,
	root, output, expectedDepth string,
	environment []string,
	clock func() time.Time,
	captureSource sourceCapture,
) (capturedProductionInputs, error) {
	parameters, parametersSHA256, err := buildParameters(expectedDepth)
	if err != nil {
		return capturedProductionInputs{}, err
	}
	pre, err := captureSource(ctx, root, environment)
	if err != nil {
		return capturedProductionInputs{}, fmt.Errorf("capture pre-report source: %w", err)
	}
	report, reportManifest, err := captureReport(pre.Root, output, expectedDepth)
	if err != nil {
		return capturedProductionInputs{}, err
	}
	occurrences, err := reportOccurrences(report)
	if err != nil {
		return capturedProductionInputs{}, err
	}
	facts, err := captureEvidenceFiles(ctx, pre.Root, occurrences, pre.Manifest)
	if err != nil {
		return capturedProductionInputs{}, fmt.Errorf("capture Evolve locator files: %w", err)
	}
	observedAtUnixMS := clock().UnixMilli()
	if observedAtUnixMS < 0 {
		return capturedProductionInputs{}, fmt.Errorf("local observation clock is before Unix epoch")
	}
	post, err := captureSource(ctx, pre.Root, environment)
	if err != nil {
		return capturedProductionInputs{}, fmt.Errorf("capture post-report source: %w", err)
	}
	if err := requireStableSource(pre, post); err != nil {
		return capturedProductionInputs{}, err
	}
	return capturedProductionInputs{
		facts: facts, observedAtUnixMS: observedAtUnixMS, occurrences: occurrences,
		parameters: parameters, parametersSHA256: parametersSHA256,
		reportManifest: reportManifest, source: pre,
	}, nil
}

func (captured capturedProductionInputs) seal(expectedDepth, runID string) (*Production, error) {
	observations, err := buildObservations(
		captured.occurrences, captured.facts, captured.parametersSHA256,
		captured.reportManifest.SHA256, runID, captured.source.Manifest.SourceRevision,
		captured.source.SHA256, captured.observedAtUnixMS,
	)
	if err != nil {
		return nil, err
	}
	setObservationDepth(observations, expectedDepth)
	return sealProduction(ProductionPackage{
		APIVersion: ProductionAPIVersion, Canonicalization: Canonicalization,
		Observations: observations, ParametersManifest: captured.parameters,
		ReportManifest: captured.reportManifest,
		SourceManifest: gitworktreesource.CloneManifest(captured.source.Manifest),
	})
}

func validateCaptureInputs(
	runID, expectedDepth string,
	clock func() time.Time,
	captureSource sourceCapture,
) error {
	if err := ensureSupportedPlatform(); err != nil {
		return err
	}
	if len(runID) > 160 || !runIDPattern.MatchString(runID) {
		return fmt.Errorf("run_id is not a valid bounded identifier")
	}
	if !validDepth(expectedDepth) {
		return fmt.Errorf("unsupported expected_depth %q", expectedDepth)
	}
	if clock == nil || captureSource == nil {
		return fmt.Errorf("capture dependencies are unavailable")
	}
	return nil
}

func requireStableSource(pre, post gitworktreesource.Snapshot) error {
	if pre.Root != post.Root || pre.SHA256 != post.SHA256 ||
		!reflect.DeepEqual(pre.Manifest, post.Manifest) {
		return fmt.Errorf("repository source changed during Evolve locator capture")
	}
	return nil
}
