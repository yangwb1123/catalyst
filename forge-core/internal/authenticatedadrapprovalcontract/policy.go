package authenticatedadrapprovalcontract

import "fmt"

var requiredDistinctions = []string{
	"approver_not_implementer",
	"approver_not_owner",
	"approver_not_requester",
	"approver_not_subject",
	"approvers_pairwise_distinct",
}

func validatePolicy(value any, root map[string]any, metadata proposalMetadata) (map[string]any, error) {
	label := "ArchitectureDecisionApprovalPolicy"
	fields := []string{"api_version", "approval_record_profile", "canonicalization",
		"disposition", "eligible_approver_key_ids", "kind", "max_request_validity_ms",
		"policy_id", "policy_sha256", "profile_id", "proposal_binding",
		"required_distinctions", "roles", "signature", "threshold", "trust_epoch",
		"trust_root_sha256", "validity", "veto_on_reject"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxPolicyBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != policyAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if err = validatePolicyScalars(node, metadata); err != nil {
		return nil, err
	}
	if err = validatePolicyRoles(node, root, metadata); err != nil {
		return nil, err
	}
	if err = validatePolicyAuthority(node, root); err != nil {
		return nil, err
	}
	if err = validatePolicyBinding(node["proposal_binding"], metadata); err != nil {
		return nil, err
	}
	digest, err := policySHA256(node)
	if err != nil || node["policy_sha256"] != digest {
		return nil, fmt.Errorf("policy self digest does not match")
	}
	return node, nil
}

func validatePolicyScalars(node map[string]any, metadata proposalMetadata) error {
	if _, err := stableID(node["policy_id"], "policy.policy_id"); err != nil {
		return err
	}
	if _, err := enumValue(node["disposition"], "policy.disposition", "allow", "deny"); err != nil {
		return err
	}
	if _, err := intValue(node["max_request_validity_ms"], "policy.max_request_validity_ms", 1, maxRequestValidityMS); err != nil {
		return err
	}
	if err := validatePolicyValidity(node["validity"], metadata); err != nil {
		return err
	}
	if err := validateApprovalProfile(node["approval_record_profile"]); err != nil {
		return err
	}
	return validatePolicyControls(node)
}

func validatePolicyValidity(value any, metadata proposalMetadata) error {
	node, err := requireKeys(value, "policy.validity", "expires_at_unix_ms", "not_before_unix_ms")
	if err != nil {
		return err
	}
	starts, err := intValue(node["not_before_unix_ms"], "policy.validity.not_before_unix_ms", 0, maxInt64)
	if err != nil {
		return err
	}
	expires, err := intValue(node["expires_at_unix_ms"], "policy.validity.expires_at_unix_ms", 0, maxInt64)
	if err != nil {
		return err
	}
	if starts >= expires || expires-starts > maxPolicyValidityMS {
		return fmt.Errorf("policy validity must be ordered within 24 hours")
	}
	if starts < metadata.ProposedAtUnixMS {
		return fmt.Errorf("policy validity begins before the proposal declared time")
	}
	if metadata.ExpiresAtUnixMS != nil && expires > *metadata.ExpiresAtUnixMS {
		return fmt.Errorf("policy validity extends beyond proposal declared expiry")
	}
	return nil
}

func validateApprovalProfile(value any) error {
	label := "policy.approval_record_profile"
	fields := []string{"change_id", "context_sha256", "environment_class", "environment_id",
		"gate_id", "impact_sha256", "materiality_level", "plan_sha256", "project_id",
		"risk_sha256", "source_revision", "source_tree_sha256", "subject"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return err
	}
	for _, field := range []string{"change_id", "environment_id", "project_id", "source_revision"} {
		if _, err = textValue(node[field], label+"."+field, 160); err != nil {
			return err
		}
	}
	if node["gate_id"] != gateID || node["materiality_level"] != "L4" {
		return fmt.Errorf("approval record profile must use the fixed gate at L4")
	}
	if _, err = enumValue(node["environment_class"], label+".environment_class",
		"development", "local", "production", "staging", "test"); err != nil {
		return err
	}
	for _, field := range []string{"context_sha256", "impact_sha256", "plan_sha256", "risk_sha256", "source_tree_sha256"} {
		if _, err = shaValue(node[field], label+"."+field); err != nil {
			return err
		}
	}
	_, err = validatePrincipal(node["subject"], label+".subject", "service")
	return err
}

func validatePolicyControls(node map[string]any) error {
	eligible, err := sortedUniqueStringValues(node["eligible_approver_key_ids"], "policy.eligible_approver_key_ids", 2, 16)
	if err != nil {
		return err
	}
	threshold, err := intValue(node["threshold"], "policy.threshold", 2, 16)
	if err != nil {
		return err
	}
	if threshold > int64(len(eligible)) {
		return fmt.Errorf("policy threshold exceeds eligible approver count")
	}
	distinctions, err := sortedUniqueStringValues(node["required_distinctions"], "policy.required_distinctions", 5, 5)
	if err != nil || !equalStrings(distinctions, requiredDistinctions) {
		return fmt.Errorf("policy required distinctions drifted from v1")
	}
	if node["veto_on_reject"] != true {
		return fmt.Errorf("policy veto_on_reject must be true")
	}
	return nil
}

func validatePolicyRoles(node map[string]any, root map[string]any, metadata proposalMetadata) error {
	roles, err := requireKeys(node["roles"], "policy.roles", "approver_bindings",
		"implementers", "owner_bindings", "requester")
	if err != nil {
		return err
	}
	approvers, err := validateApproverBindings(roles["approver_bindings"], root, metadata.ApproverRefs)
	if err != nil {
		return err
	}
	owners, err := validateOwnerBindings(roles["owner_bindings"], metadata.OwnerRefs)
	if err != nil {
		return err
	}
	implementers, err := validateImplementers(roles["implementers"])
	if err != nil {
		return err
	}
	requester, err := validatePrincipal(roles["requester"], "policy.roles.requester", "agent", "human", "operator", "service")
	if err != nil {
		return err
	}
	if !approverKeysMatchPolicy(approvers, node["eligible_approver_key_ids"].([]any)) {
		return fmt.Errorf("eligible approver keys differ from approver-ref mappings")
	}
	profile := node["approval_record_profile"].(map[string]any)
	return validateRoleSeparation(root, node["eligible_approver_key_ids"].([]any), owners,
		implementers, requester, profile["subject"].(map[string]any))
}

func validateApproverBindings(value any, root map[string]any, declared []string) ([]map[string]any, error) {
	items, err := sortedUniqueNodes(value, "policy.roles.approver_bindings", 2, 16)
	if err != nil {
		return nil, err
	}
	result, refs, seen := make([]map[string]any, len(items)), make([]string, len(items)), map[string]bool{}
	for index, item := range items {
		node, nodeErr := requireKeys(item, fmt.Sprintf("approver_bindings[%d]", index), "approver_ref", "key_id")
		if nodeErr != nil {
			return nil, nodeErr
		}
		refs[index], err = stableRef(node["approver_ref"], fmt.Sprintf("approver_bindings[%d].approver_ref", index))
		if err != nil {
			return nil, err
		}
		keyID, err := textValue(node["key_id"], fmt.Sprintf("approver_bindings[%d].key_id", index), 160)
		if err != nil || seen[keyID] {
			return nil, fmt.Errorf("approver bindings must use unique root keys")
		}
		key, keyErr := keyNodeByID(root, keyID)
		if keyErr != nil || key["usage"] != "architecture_approval_sign" {
			return nil, fmt.Errorf("approver binding key has the wrong root usage")
		}
		seen[keyID], result[index] = true, node
	}
	if !equalStrings(refs, declared) {
		return nil, fmt.Errorf("policy must map every exact proposal approver_ref")
	}
	return result, nil
}

func validateOwnerBindings(value any, declared []string) ([]map[string]any, error) {
	items, err := sortedUniqueNodes(value, "policy.roles.owner_bindings", 1, 64)
	if err != nil {
		return nil, err
	}
	result, refs, principals := make([]map[string]any, len(items)), make([]string, len(items)), map[string]bool{}
	for index, item := range items {
		node, nodeErr := requireKeys(item, fmt.Sprintf("owner_bindings[%d]", index), "owner_ref", "principal")
		if nodeErr != nil {
			return nil, nodeErr
		}
		refs[index], err = stableRef(node["owner_ref"], fmt.Sprintf("owner_bindings[%d].owner_ref", index))
		principal, principalErr := validatePrincipal(node["principal"], fmt.Sprintf("owner_bindings[%d].principal", index), "agent", "human", "operator", "service")
		if err != nil || principalErr != nil {
			return nil, fmt.Errorf("owner binding is malformed")
		}
		identity, _ := canonicalJSON(principal)
		if principals[string(identity)] {
			return nil, fmt.Errorf("owner bindings must map to unique principals")
		}
		principals[string(identity)], result[index] = true, node
	}
	if !equalStrings(refs, declared) {
		return nil, fmt.Errorf("policy must map every exact proposal owner_ref")
	}
	return result, nil
}

func validateImplementers(value any) ([]map[string]any, error) {
	items, err := sortedUniqueNodes(value, "policy.roles.implementers", 1, 32)
	if err != nil {
		return nil, err
	}
	result := make([]map[string]any, len(items))
	for index, item := range items {
		result[index], err = validatePrincipal(item, fmt.Sprintf("policy.roles.implementers[%d]", index), "agent", "human", "operator", "service")
		if err != nil {
			return nil, err
		}
	}
	return result, nil
}

func approverKeysMatchPolicy(bindings []map[string]any, eligible []any) bool {
	if len(bindings) != len(eligible) {
		return false
	}
	keys := make([]string, len(bindings))
	for index, binding := range bindings {
		keys[index] = binding["key_id"].(string)
	}
	sortStrings(keys)
	for index, item := range eligible {
		if item != keys[index] {
			return false
		}
	}
	return true
}

func validateRoleSeparation(root map[string]any, eligible []any, owners,
	implementers []map[string]any, requester, subject map[string]any) error {
	blocked := map[string]bool{principalIdentity(requester): true, principalIdentity(subject): true}
	for _, principal := range implementers {
		blocked[principalIdentity(principal)] = true
	}
	for _, owner := range owners {
		blocked[principalIdentity(owner["principal"].(map[string]any))] = true
	}
	for _, item := range eligible {
		key, err := keyNodeByID(root, item.(string))
		if err != nil {
			return err
		}
		if blocked[principalIdentity(key["principal"].(map[string]any))] {
			return fmt.Errorf("eligible approvers violate policy role separation")
		}
	}
	return nil
}

func validatePolicyAuthority(node, root map[string]any) error {
	if node["trust_root_sha256"] != root["root_sha256"] || node["trust_epoch"] != root["trust_epoch"] {
		return fmt.Errorf("policy does not bind the supplied trust root")
	}
	if _, err := intValue(node["trust_epoch"], "policy.trust_epoch", 1, maxInt64); err != nil {
		return err
	}
	signature, err := validateSignature(node["signature"], "policy.signature", signatureProfileSHA256Pin)
	if err != nil {
		return err
	}
	key, err := keyNodeForUsage(root, "approval_policy_sign")
	if err != nil || signature["key_id"] != key["key_id"] {
		return fmt.Errorf("policy signature uses the wrong root key usage")
	}
	return nil
}

func validatePolicyBinding(value any, metadata proposalMetadata) error {
	binding, err := validateProposalBinding(value)
	if err != nil {
		return err
	}
	expected := map[string]string{"adr_id": metadata.ADRID, "body_sha256": metadata.BodySHA256,
		"document_name": metadata.DocumentName, "self_sha256": metadata.SelfSHA256,
		"status": metadata.Status}
	for field, want := range expected {
		if binding[field] != want {
			return fmt.Errorf("policy ProposalBinding differs from supplied proposal metadata")
		}
	}
	return nil
}

func principalIdentity(value map[string]any) string {
	encoded, _ := canonicalJSON(value)
	return string(encoded)
}

func equalStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}

func sortStrings(values []string) {
	for index := 1; index < len(values); index++ {
		for cursor := index; cursor > 0 && values[cursor] < values[cursor-1]; cursor-- {
			values[cursor], values[cursor-1] = values[cursor-1], values[cursor]
		}
	}
}
