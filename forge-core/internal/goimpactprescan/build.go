package goimpactprescan

import (
	"encoding/base64"
	"fmt"
	"sort"

	"forgeos/forge-core/internal/gopackagedependencyobservationproducer"
	"forgeos/forge-core/internal/gopackagegraph"
)

// Build evaluates exact caller-supplied ADR-0053 graph bytes. It performs no
// repository, process, network, clock, persistence, or authority operation.
func Build(
	graphJSON []byte,
	graphSHA256 string,
	runID string,
	changedPaths []string,
) (*Production, error) {
	paths, err := validateChangedPaths(changedPaths)
	if err != nil {
		return nil, err
	}
	observation, actualGraphSHA256, err :=
		gopackagedependencyobservationproducer.DecodeGraphObservation(graphJSON)
	if err != nil {
		return nil, err
	}
	if actualGraphSHA256 != graphSHA256 || observation.Producer.RunID != runID {
		return nil, fmt.Errorf("graph digest or run ID does not match request")
	}
	return buildValidated(graphJSON, graphSHA256, runID, paths, observation)
}

func buildValidated(
	graphJSON []byte,
	graphSHA256 string,
	runID string,
	changedPaths []string,
	observation gopackagegraph.Observation,
) (*Production, error) {
	encodedGraph, err := encodeGraphObservation(graphJSON)
	if err != nil {
		return nil, err
	}
	request, _, err := sealRequest(Request{
		APIVersion: RequestAPIVersion, Canonicalization: Canonicalization,
		ChangedPaths:              append([]string{}, changedPaths...),
		GraphObservationBase64URL: encodedGraph, GraphObservationSHA256: graphSHA256,
		RunID: runID,
	})
	if err != nil {
		return nil, fmt.Errorf("seal impact request: %w", err)
	}
	index, err := indexGraph(observation)
	if err != nil {
		return nil, err
	}
	resolved, unresolved, seedIDs, err := resolveSeeds(changedPaths, observation, index)
	if err != nil {
		return nil, err
	}
	nodes, edges, err := reverseClosure(index, seedIDs)
	if err != nil {
		return nil, err
	}
	status, reasons := closureStatus(observation, unresolved)
	report, _, err := sealReport(Report{
		APIVersion: ReportAPIVersion, Canonicalization: Canonicalization,
		ClosureReasonCodes: reasons, GraphObservationSHA256: graphSHA256,
		PackageLexicalClosureStatus: status, ReachableEdges: edges, ReachableNodes: nodes,
		RequestSHA256: request.RequestSHA256, ResolvedSeeds: resolved, Result: Result,
		RunID: runID, SystemImpactStatus: Unknown,
		SystemUnknownReasonCodes: append([]string{}, systemUnknownReasons...),
		UnresolvedSeeds:          unresolved,
	})
	if err != nil {
		return nil, fmt.Errorf("seal impact report: %w", err)
	}
	return sealEnvelope(request, report)
}

func validateChangedPaths(values []string) ([]string, error) {
	if len(values) == 0 || len(values) > maxChangedPaths {
		return nil, fmt.Errorf("changed paths must contain 1..%d items", maxChangedPaths)
	}
	result := append([]string{}, values...)
	for index, value := range result {
		if !validRepoPath(value) || index > 0 && result[index-1] >= value {
			return nil, fmt.Errorf("changed paths are invalid, duplicate, or unsorted")
		}
	}
	if !sort.StringsAreSorted(result) {
		return nil, fmt.Errorf("changed paths are not sorted")
	}
	return result, nil
}

func encodeGraphObservation(value []byte) (string, error) {
	if len(value) == 0 || len(value) > maxGraphBytes {
		return "", fmt.Errorf("graph observation exceeds %d bytes", maxGraphBytes)
	}
	encoded := base64.RawURLEncoding.EncodeToString(value)
	if len(encoded) > 22_369_622 {
		return "", fmt.Errorf("encoded graph observation exceeds bound")
	}
	return encoded, nil
}
