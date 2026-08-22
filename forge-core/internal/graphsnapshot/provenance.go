package graphsnapshot

import (
	"fmt"

	"forgeos/forge-core/internal/gopackagegraph"
)

func buildSource(observation gopackagegraph.Observation, graphSHA256 string) (Source, error) {
	identity := sourceIdentity{
		GraphAPIVersion: observation.APIVersion, GraphObservationSHA256: graphSHA256,
		GraphProfileID: observation.ProfileID, ObserverRunID: observation.Producer.RunID,
		SourceRevision:   observation.Source.SourceRevision,
		SourceTreeSHA256: observation.Source.SourceTreeSHA256, SourceType: "adr_0053_graph_observation",
	}
	identitySHA, err := digestValue(sourceIdentityDomain, identity)
	if err != nil {
		return Source{}, err
	}
	value := Source{
		GraphAPIVersion:        identity.GraphAPIVersion,
		GraphObservationSHA256: identity.GraphObservationSHA256,
		GraphProfileID:         identity.GraphProfileID, ObservedAtUnixMS: observation.ObservedAtUnixMS,
		ObserverParametersSHA256: observation.Producer.ParametersSHA256,
		ObserverProducerID:       observation.Producer.ProducerID,
		ObserverProducerType:     observation.Producer.ProducerType,
		ObserverProducerVersion:  observation.Producer.ProducerVersion,
		ObserverRunID:            identity.ObserverRunID, SourceID: "graph-source-" + identitySHA,
		SourceIdentitySHA256: identitySHA, SourceRevision: identity.SourceRevision,
		SourceTreeSHA256: identity.SourceTreeSHA256, SourceType: identity.SourceType,
	}
	value.SourceSHA256 = ""
	value.SourceSHA256, err = digestValue(sourceDomain, value)
	return value, err
}

func buildExtractor(source Source, projectorProfileID string) (Extractor, error) {
	identity := extractorIdentity{
		ExtractorType: "graph_snapshot_projector", ExtractorVersion: "v1",
		ProducerID: "forgeos.local-go-graph-snapshot-projector", ProjectorProfileID: projectorProfileID,
	}
	identitySHA, err := digestValue(extractorIdentityDomain, identity)
	if err != nil {
		return Extractor{}, err
	}
	value := Extractor{
		ExtractorID: "graph-extractor-" + identitySHA, ExtractorIdentitySHA256: identitySHA,
		ExtractorType: identity.ExtractorType, ExtractorVersion: identity.ExtractorVersion,
		InputGraphAPIVersion: source.GraphAPIVersion, InputGraphProfileID: source.GraphProfileID,
		InputSourceID: source.SourceID, ProducerID: identity.ProducerID,
		ProducerType: "tool", ProducerVersion: "v1", ProjectorProfileID: identity.ProjectorProfileID,
	}
	value.ExtractorSHA256 = ""
	value.ExtractorSHA256, err = digestValue(extractorDomain, value)
	return value, err
}

func (value *projector) claimIdentity(kind, digest, id string) error {
	if prior, exists := value.identityDigests[digest]; exists {
		return fmt.Errorf("%s identity digest collides with %s", kind, prior)
	}
	if digest == "" || id == "" {
		return fmt.Errorf("%s identity is empty", kind)
	}
	value.identityDigests[digest] = kind
	return nil
}
