package graphplan

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"sort"
)

// Build validates an authored spec and creates an inert forge-core plan.
func Build(spec Spec, graphID, manifestSHA256 string) (Plan, error) {
	return buildWithProtocol(spec, graphID, manifestSHA256, SchedulerProtocolVersion)
}

func buildWithProtocol(
	spec Spec,
	graphID string,
	manifestSHA256 string,
	schedulerProtocolVersion uint16,
) (Plan, error) {
	waves, err := validateAndPlan(spec)
	if err != nil || !validText(graphID, maxIdentifierBytes) ||
		!isLowerHexDigest(manifestSHA256) || schedulerProtocolVersion == 0 {
		return Plan{}, errInvalidSpec
	}
	payload := planPayload{
		V:                         1,
		SchedulerProtocolVersion:  schedulerProtocolVersion,
		GraphVersion:              GraphVersion,
		GraphID:                   graphID,
		GraphManifestSHA256:       manifestSHA256,
		AuthoredNodeIDs:           authoredNodeIDs(spec.Nodes),
		Edges:                     canonicalEdges(spec.Edges),
		Waves:                     waves,
		ExecutionContractPresent:  false,
		DispatchAuthorityReleased: false,
	}
	digest, err := payloadDigest(payload)
	if err != nil {
		return Plan{}, err
	}
	return planFromPayload(payload, digest), nil
}

func authoredNodeIDs(nodes []Node) []string {
	identifiers := make([]string, len(nodes))
	for position, node := range nodes {
		identifiers[position] = node.NodeID
	}
	return identifiers
}

func canonicalEdges(edges []Edge) []Edge {
	canonical := make([]Edge, len(edges))
	copy(canonical, edges)
	sort.Slice(canonical, func(left, right int) bool {
		if canonical[left].FromNodeID == canonical[right].FromNodeID {
			return canonical[left].ToNodeID < canonical[right].ToNodeID
		}
		return canonical[left].FromNodeID < canonical[right].FromNodeID
	})
	return canonical
}

func payloadDigest(payload planPayload) (string, error) {
	encoded, err := encodeCanonical(payload)
	if err != nil {
		return "", err
	}
	digest := sha256.New()
	_, _ = digest.Write([]byte(planDigestDomain))
	_, _ = digest.Write(encoded)
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func planFromPayload(payload planPayload, digest string) Plan {
	return Plan{
		V:                         payload.V,
		SchedulerProtocolVersion:  payload.SchedulerProtocolVersion,
		GraphVersion:              payload.GraphVersion,
		GraphID:                   payload.GraphID,
		GraphManifestSHA256:       payload.GraphManifestSHA256,
		AuthoredNodeIDs:           payload.AuthoredNodeIDs,
		Edges:                     payload.Edges,
		Waves:                     payload.Waves,
		ExecutionContractPresent:  payload.ExecutionContractPresent,
		DispatchAuthorityReleased: payload.DispatchAuthorityReleased,
		PlanSHA256:                digest,
	}
}

// MarshalPlan returns compact canonical plan JSON without a trailing newline.
func MarshalPlan(plan Plan) ([]byte, error) {
	return encodeCanonical(plan)
}

func encodeCanonical(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidSpec
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func isLowerHexDigest(value string) bool {
	if len(value) != sha256.Size*2 {
		return false
	}
	for _, character := range []byte(value) {
		if (character < '0' || character > '9') &&
			(character < 'a' || character > 'f') {
			return false
		}
	}
	return true
}
