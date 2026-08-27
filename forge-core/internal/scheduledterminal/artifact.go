package scheduledterminal

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
)

var errInvalidArtifact = errors.New("terminal artifact is invalid")

// DecodeArtifact strictly decodes one bounded, exact canonical terminal
// artifact and verifies all of its content-addressed identities.
func DecodeArtifact(data []byte) (Artifact, error) {
	if len(data) == 0 || len(data) > maxArtifactBytes {
		return Artifact{}, errInvalidArtifact
	}
	var value Artifact
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&value); err != nil {
		return Artifact{}, errInvalidArtifact
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return Artifact{}, errInvalidArtifact
	}
	canonical, err := marshalCanonical(value)
	if err != nil || !bytes.Equal(canonical, data) || value.validate() != nil {
		return Artifact{}, errInvalidArtifact
	}
	return value, nil
}

// MarshalArtifact validates artifact facts, assigns a fresh content identity,
// and returns exact canonical JSON. Production artifacts are still derived
// from a terminal control; this constructor supports bounded source fixtures.
func MarshalArtifact(value Artifact) ([]byte, error) {
	if !artifactInputWithinBounds(value) {
		return nil, errInvalidArtifact
	}
	value.ArtifactID, value.ArtifactSHA256, value.ArtifactBytes = "", "", 0
	digest, err := digestWithoutField(
		value, []string{"artifact_id", "artifact_bytes", "artifact_sha256"}, artifactDomain,
	)
	if err != nil {
		return nil, errInvalidArtifact
	}
	value.ArtifactID = "scheduled-node-terminal-artifact-" + digest
	value.ArtifactSHA256 = digest
	encoded, err := settleArtifactBytes(value)
	if err != nil {
		return nil, errInvalidArtifact
	}
	validated, err := DecodeArtifact(encoded)
	if err != nil || validated != valueWithArtifactBytes(value, len(encoded)) {
		return nil, errInvalidArtifact
	}
	return encoded, nil
}

func artifactInputWithinBounds(value Artifact) bool {
	return len(value.OutputText) <= maxArtifactBytes && len(value.ArtifactKind) <= 32 &&
		len(value.Classification) <= 32 && len(value.GraphRunID) <= 256 &&
		len(value.NodeID) <= 256 && len(value.DispatchID) <= 256 &&
		len(value.ProviderRequestID) <= 256 && len(value.LaneOwnershipID) <= 256 &&
		len(value.ClaimEventSHA256) <= 64 && len(value.AuthorizationSHA256) <= 64 &&
		len(value.ProviderRequestSHA256) <= 64 && len(value.RequestBodySHA256) <= 64 &&
		len(value.PricingSnapshotSHA256) <= 64 && len(value.ProjectLaneSHA256) <= 64 &&
		len(value.OutputSHA256) <= 64
}

func settleArtifactBytes(value Artifact) ([]byte, error) {
	for range 4 {
		encoded, err := marshalCanonical(value)
		if err != nil || len(encoded) == 0 || len(encoded) > maxArtifactBytes {
			return nil, errInvalidArtifact
		}
		if value.ArtifactBytes == len(encoded) {
			return encoded, nil
		}
		value.ArtifactBytes = len(encoded)
	}
	return nil, errInvalidArtifact
}

func valueWithArtifactBytes(value Artifact, size int) Artifact {
	value.ArtifactBytes = size
	return value
}

// ValidatePredecessorContent verifies that one completed result receipt and
// its terminal artifact bind the exact disclosed predecessor output.
func ValidatePredecessorContent(receipt Receipt, artifact Artifact, content string) error {
	if content == "" || validateExactReceipt(receipt) != nil || artifact.validate() != nil {
		return errInvalidArtifact
	}
	completed := receipt.NodeOutcome == "completed" && receipt.ArtifactKind == "result" &&
		artifact.Classification == "completed" && artifact.ArtifactKind == "result"
	identityBound := receipt.GraphRunID == artifact.GraphRunID &&
		receipt.NodeID == artifact.NodeID && receipt.Attempt == artifact.Attempt &&
		receipt.DispatchID == artifact.DispatchID &&
		receipt.ProviderRequestID == artifact.ProviderRequestID &&
		receipt.ProjectLaneSHA256 == artifact.ProjectLaneSHA256 &&
		receipt.ArtifactID == artifact.ArtifactID &&
		receipt.ArtifactSHA256 == artifact.ArtifactSHA256
	if !completed || !identityBound || artifact.OutputText != content {
		return errInvalidArtifact
	}
	return nil
}

func validateExactReceipt(value Receipt) error {
	encoded, err := marshalReceipt(value)
	if err != nil {
		return errInvalidArtifact
	}
	validated, err := DecodeReceipt(encoded)
	if err != nil || validated != value {
		return errInvalidArtifact
	}
	return nil
}
