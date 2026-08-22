package bootstraprepoexecutionauthority

import "fmt"

// Replay authenticates raw identity documents against a self-contained Ledger.
// It requires no Manifest input, issuance Ledger, clock, private key, or repository.
func (ledger *Ledger) Replay(policyData, invocationData []byte) (interface {
	canonicalDocument() map[string]any
}, string, bool, bool, error) {
	if ledger == nil {
		return nil, "", false, false, nil
	}
	if len(policyData) == 64 && len(invocationData) == 64 {
		return ledger.replayDigests(string(policyData), string(invocationData))
	}
	policy, invocation, err := decodeReplayIdentity(policyData, invocationData, ledger.trust)
	if err != nil {
		return nil, "", false, false, err
	}
	group, found, conflict := findReplayGroup(ledger, policy, invocation)
	return replayResolvedGroup(group, found, conflict)
}

func (ledger *Ledger) replayDigests(policyDigest, invocationDigest string) (interface {
	canonicalDocument() map[string]any
}, string, bool, bool, error) {
	if validateHash(policyDigest, "execution_policy_sha256") != nil ||
		validateHash(invocationDigest, "invocation_sha256") != nil {
		return nil, "", false, false, fmt.Errorf("replay digests are invalid")
	}
	group, found, conflict := findReplayDigestGroup(ledger, policyDigest, invocationDigest)
	return replayResolvedGroup(group, found, conflict)
}

func replayResolvedGroup(group *usageGroup, found, conflict bool) (interface {
	canonicalDocument() map[string]any
}, string, bool, bool, error) {
	if !found || conflict {
		return nil, "", found, conflict, nil
	}
	if group.terminal == nil {
		state := group.reservation.document["state"].(string)
		if group.intent != nil {
			state = group.intent.document["state"].(string)
		}
		return nil, state, true, false, nil
	}
	state := group.terminal.receipt.document["state"].(string)
	delivery, err := BuildDelivery("exact_replay", nil, group.terminal.receipt,
		group.terminal.metadata)
	return delivery, state, true, false, err
}

func decodeReplayIdentity(policyData, invocationData []byte,
	trust *Trust) (*Policy, *Invocation, error) {
	policyDocument, err := decodeCanonical(policyData, maxPolicyBytes)
	if err != nil {
		return nil, nil, err
	}
	if err = validateReplayPolicyIdentity(policyDocument, trust); err != nil {
		return nil, nil, err
	}
	policy := &Policy{document: policyDocument}
	invocationDocument, err := decodeCanonical(invocationData, maxInvocationBytes)
	if err != nil {
		return nil, nil, err
	}
	if err = validateReplayInvocationIdentity(invocationDocument, trust, policy); err != nil {
		return nil, nil, err
	}
	return policy, &Invocation{document: invocationDocument}, nil
}

func validateReplayPolicyIdentity(document map[string]any, trust *Trust) error {
	if err := requireKeys(document, policyKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadExecutionPolicy: %w", err)
	}
	if err := validatePolicyShape(document, trust); err != nil {
		return err
	}
	if !sameCanonical(document["subject"], trust.keys["execution_request_auth"].principal) {
		return fmt.Errorf("ExecutionPolicy request principal binding is invalid")
	}
	return validateSigned(document, "execution_policy_sha256", policyDomain,
		policySignatureDomain, maxPolicyBytes, "ExecutionPolicy", trust, "execution_policy_sign", "")
}

func validateReplayInvocationIdentity(document map[string]any, trust *Trust,
	policy *Policy) error {
	if err := requireKeys(document, invocationKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadInvocation: %w", err)
	}
	if err := validateInvocationShape(document, trust); err != nil {
		return err
	}
	if !policy.AllowsExecution() {
		return fmt.Errorf("non-activating ExecutionPolicy cannot be replayed")
	}
	if err := validateReplayPairRelations(document, policy.document); err != nil {
		return err
	}
	return validateSigned(document, "invocation_sha256", invocationDomain,
		invocationSignatureDomain, maxInvocationBytes, "Invocation", trust,
		"execution_request_auth", "invocation_id")
}

func validateReplayPairRelations(invocation, policy map[string]any) error {
	fields := []string{"bindings", "capability", "execution_trust_epoch",
		"execution_trust_root_sha256", "grant_envelope_sha256", "grant_id",
		"grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256", "grant_policy_sha256",
		"grant_request_sha256", "grant_sha256", "idempotency_key", "issuance_trust_epoch",
		"issuance_trust_root_sha256", "manifest_sha256", "profile_id", "requested_action",
		"requested_action_sha256", "subject", "task_binding"}
	for _, field := range fields {
		if !sameCanonical(invocation[field], policy[field]) {
			return fmt.Errorf("Invocation differs from exact signed ExecutionPolicy")
		}
	}
	if invocation["execution_policy_sha256"] != policy["execution_policy_sha256"] {
		return fmt.Errorf("Invocation does not bind exact ExecutionPolicy")
	}
	return validateInvocationPolicyWindow(invocation, policy)
}

func findReplayGroup(ledger *Ledger, policy *Policy,
	invocation *Invocation) (*usageGroup, bool, bool) {
	grant := invocation.document["grant_envelope_sha256"].(string)
	record := recordKey(invocation.document["idempotency_key"].(string))
	grantGroup, grantFound := ledger.byGrant[grant]
	recordGroup, recordFound := ledger.byRecord[record]
	if !grantFound && !recordFound {
		return nil, false, false
	}
	if !grantFound || !recordFound || grantGroup != recordGroup ||
		!sameCanonical(grantGroup.policy.document, policy.document) ||
		!sameCanonical(grantGroup.invocation.document, invocation.document) {
		return nil, true, true
	}
	return grantGroup, true, false
}

func findReplayDigestGroup(ledger *Ledger, policyDigest,
	invocationDigest string) (*usageGroup, bool, bool) {
	var policyGroup, invocationGroup *usageGroup
	for _, group := range ledger.byGrant {
		if group.policy.document["execution_policy_sha256"] == policyDigest {
			if policyGroup != nil && policyGroup != group {
				return nil, true, true
			}
			policyGroup = group
		}
		if group.invocation.document["invocation_sha256"] == invocationDigest {
			if invocationGroup != nil && invocationGroup != group {
				return nil, true, true
			}
			invocationGroup = group
		}
	}
	if policyGroup == nil && invocationGroup == nil {
		return nil, false, false
	}
	if policyGroup == nil || invocationGroup == nil || policyGroup != invocationGroup {
		return nil, true, true
	}
	return policyGroup, true, false
}
