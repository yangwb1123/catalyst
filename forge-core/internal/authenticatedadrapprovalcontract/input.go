package authenticatedadrapprovalcontract

import (
	"bytes"
	"fmt"
)

// DecodeAuthorizationInput validates exact proposal, policy, complete
// revocation-snapshot, and request bytes against one caller-supplied root.
func DecodeAuthorizationInput(encoded EncodedAuthorizationInput,
	root *TrustRoot) (*AuthorizationInput, error) {
	if root == nil {
		return nil, fmt.Errorf("trust root is nil")
	}
	policyValue, err := parseStrictJSON(encoded.Policy, maxPolicyBytes)
	if err != nil {
		return nil, fmt.Errorf("authorization policy: %w", err)
	}
	policyNode, ok := policyValue.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("authorization policy root must be an object")
	}
	binding, err := validateProposalBinding(policyNode["proposal_binding"])
	if err != nil {
		return nil, err
	}
	metadata, err := validateProposalBytes(encoded.ProposalDocument, binding)
	if err != nil {
		return nil, err
	}
	policy, err := validatePolicy(policyNode, root.document, metadata)
	if err != nil {
		return nil, err
	}
	if err = requireExactBytes(encoded.Policy, policy, maxPolicyBytes, "authorization policy"); err != nil {
		return nil, err
	}
	snapshots, err := decodeInputSnapshots(encoded.RevocationSnapshots, root.document)
	if err != nil {
		return nil, err
	}
	request, err := decodeInputRequest(encoded.Request, root.document, policy, snapshots)
	if err != nil {
		return nil, err
	}
	items := make([]any, len(snapshots))
	for index, snapshot := range snapshots {
		items[index] = cloneValue(snapshot)
	}
	return &AuthorizationInput{proposal: append([]byte(nil), encoded.ProposalDocument...),
		policy: cloneValue(policy).(map[string]any), snapshots: items,
		request: cloneValue(request).(map[string]any), metadata: metadata, root: root}, nil
}

func decodeInputSnapshots(encoded [][]byte, root map[string]any) ([]map[string]any, error) {
	if len(encoded) == 0 || len(encoded) > maxRevocationSnapshots {
		return nil, fmt.Errorf("revocation snapshots must contain 1..%d documents", maxRevocationSnapshots)
	}
	items := make([]any, len(encoded))
	for index, raw := range encoded {
		node, err := decodeCanonicalObject(raw, maxRevocationBytes,
			fmt.Sprintf("revocation snapshot %d", index+1), func(value any) (map[string]any, error) {
				return validateRevocation(value, root)
			})
		if err != nil {
			return nil, err
		}
		items[index] = node
	}
	return validateRevocationChain(items, root)
}

func decodeInputRequest(raw []byte, root, policy map[string]any,
	snapshots []map[string]any) (map[string]any, error) {
	value, err := parseStrictJSON(raw, maxRequestBytes)
	if err != nil {
		return nil, fmt.Errorf("authorization request: %w", err)
	}
	node, ok := value.(map[string]any)
	if !ok {
		return nil, fmt.Errorf("authorization request root must be an object")
	}
	snapshot, err := snapshotForRequest(node, snapshots)
	if err != nil {
		return nil, err
	}
	request, err := validateRequest(node, root, policy, snapshot)
	if err != nil {
		return nil, err
	}
	if err = requireExactBytes(raw, request, maxRequestBytes, "authorization request"); err != nil {
		return nil, err
	}
	return request, nil
}

func requireExactBytes(raw []byte, value any, maximum int, label string) error {
	canonical, err := boundedCanonicalJSON(value, maximum, label)
	if err != nil {
		return err
	}
	if !bytes.Equal(raw, canonical) {
		return fmt.Errorf("%s is not exact compact canonical JSON", label)
	}
	return nil
}

func inputSnapshotMaps(input *AuthorizationInput) []map[string]any {
	result := make([]map[string]any, len(input.snapshots))
	for index, item := range input.snapshots {
		result[index] = item.(map[string]any)
	}
	return result
}

func inputSnapshot(input *AuthorizationInput) (map[string]any, error) {
	return snapshotForRequest(input.request, inputSnapshotMaps(input))
}
