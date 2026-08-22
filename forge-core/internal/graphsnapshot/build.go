package graphsnapshot

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"

	"forgeos/forge-core/internal/gopackagedependencyobservationproducer"
	"forgeos/forge-core/internal/gopackagegraph"
)

type packageKey struct {
	directory string
	name      string
}

type projector struct {
	extractor       Extractor
	files           map[string]gopackagegraph.File
	identityDigests map[string]string
	nodesByID       map[string]Node
	nodesByKey      map[packageKey]Node
	observation     gopackagegraph.Observation
	profile         projectionProfile
	projectID       string
	source          Source
	testNodesByKey  map[packageKey]Node
}

// Build projects exact caller-supplied ADR-0053 bytes without observing any
// ambient source, process, provider, network, clock, environment, or store.
func Build(graphJSON []byte, graphSHA256, runID, projectID string) (*Production, error) {
	return buildWithProfile(graphJSON, graphSHA256, runID, projectID, legacyProfile)
}

// BuildTestSource projects the explicit ADR-0066 test-source profile without
// widening or negotiating the legacy Build endpoint.
func BuildTestSource(graphJSON []byte, graphSHA256, runID, projectID string) (*Production, error) {
	return buildWithProfile(graphJSON, graphSHA256, runID, projectID, testSourceProfile)
}

func buildWithProfile(
	graphJSON []byte, graphSHA256, runID, projectID string,
	profile projectionProfile,
) (*Production, error) {
	if len(graphJSON) == 0 || len(graphJSON) > maxGraphBytes {
		return nil, fmt.Errorf("graph bytes violate projection bounds")
	}
	observation, actualSHA256, err :=
		gopackagedependencyobservationproducer.DecodeGraphObservation(graphJSON)
	if err != nil {
		return nil, classifyGraphError(graphJSON, err)
	}
	if actualSHA256 != graphSHA256 {
		return nil, fmt.Errorf("graph digest or observer run ID does not match request")
	}
	if !validIdentifier(runID) || !validIdentifier(projectID) {
		return nil, fmt.Errorf("request run or project ID violates projection bounds")
	}
	if observation.Producer.RunID != runID {
		return nil, fmt.Errorf("graph digest or observer run ID does not match request")
	}
	request, requestJSON, err := buildRequest(graphJSON, graphSHA256, runID, projectID, profile)
	if err != nil {
		return nil, err
	}
	worker, err := newProjector(observation, graphSHA256, projectID, profile)
	if err != nil {
		return nil, err
	}
	snapshot, snapshotJSON, err := worker.buildSnapshot(request.RequestSHA256)
	if err != nil {
		return nil, err
	}
	return sealEnvelope(request, snapshot, requestJSON, snapshotJSON, profile)
}

func classifyGraphError(raw []byte, cause error) error {
	unsupported, err := unsupportedGraphProfile(raw)
	if err == nil && unsupported {
		return fmt.Errorf("unsupported_profile: unsupported ADR-0053 graph version or profile")
	}
	return cause
}

func unsupportedGraphProfile(raw []byte) (bool, error) {
	if err := validateDiscriminatorJSONShape(raw, maxGraphBytes); err != nil {
		return false, err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var value map[string]any
	if err := decoder.Decode(&value); err != nil {
		return false, err
	}
	if _, err := decoder.Token(); err != io.EOF {
		return false, fmt.Errorf("graph observation has trailing data")
	}
	encoded, err := canonicalJSON(value, maxGraphBytes)
	if err != nil || !bytes.Equal(encoded, raw) {
		return false, fmt.Errorf("graph observation is not exact canonical JSON")
	}
	api, apiIsString := value["api_version"].(string)
	profile, profileIsString := value["profile_id"].(string)
	return apiIsString && api != gopackagegraph.APIVersion ||
		profileIsString && profile != gopackagegraph.ProfileID, nil
}

func buildRequest(
	graphJSON []byte, graphSHA256, runID, projectID string,
	profile projectionProfile,
) (Request, []byte, error) {
	encodedGraph := base64.RawURLEncoding.EncodeToString(graphJSON)
	if len(encodedGraph) == 0 || len(encodedGraph) > maxBase64Bytes {
		return Request{}, nil, fmt.Errorf("encoded graph observation exceeds bound")
	}
	value := Request{
		APIVersion: profile.requestVersion, Canonicalization: canonicalization,
		GraphObservationBase64URL: encodedGraph, GraphObservationSHA256: graphSHA256,
		ProjectID: projectID, ProjectorProfileID: profile.profileID, RunID: runID,
	}
	value.RequestSHA256 = ""
	preimage, err := canonicalJSON(value, maxRequestBytes)
	if err != nil {
		return Request{}, nil, err
	}
	value.RequestSHA256 = domainDigest(profile.requestDomain, preimage)
	encoded, err := canonicalJSON(value, maxRequestBytes)
	return value, encoded, err
}

func newProjector(
	observation gopackagegraph.Observation,
	graphSHA256, projectID string, profile projectionProfile,
) (*projector, error) {
	if len(observation.Packages) > maxPackages || len(observation.Dependencies) > maxDependencies {
		return nil, fmt.Errorf("graph cardinality exceeds snapshot profile")
	}
	source, err := buildSource(observation, graphSHA256)
	if err != nil {
		return nil, err
	}
	extractor, err := buildExtractor(source, profile.profileID)
	if err != nil {
		return nil, err
	}
	value := &projector{
		extractor: extractor, files: make(map[string]gopackagegraph.File, len(observation.Files)),
		identityDigests: map[string]string{}, nodesByID: map[string]Node{},
		nodesByKey: map[packageKey]Node{}, observation: observation,
		profile: profile, projectID: projectID, source: source,
		testNodesByKey: map[packageKey]Node{},
	}
	if err := value.claimIdentity("source", source.SourceIdentitySHA256, source.SourceID); err != nil {
		return nil, err
	}
	if err := value.claimIdentity("extractor", extractor.ExtractorIdentitySHA256, extractor.ExtractorID); err != nil {
		return nil, err
	}
	for _, file := range observation.Files {
		if _, exists := value.files[file.Path]; exists {
			return nil, fmt.Errorf("duplicate graph file")
		}
		value.files[file.Path] = file
	}
	return value, nil
}
