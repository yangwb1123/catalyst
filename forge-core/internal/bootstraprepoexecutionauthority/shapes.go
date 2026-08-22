package bootstraprepoexecutionauthority

import (
	"fmt"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
	"forgeos/forge-core/internal/capabilitygrantcontract"
)

var principalKeys = []string{"authority_domain", "principal_id", "principal_type"}
var taskKeys = []string{"attempt_id", "change_id", "environment_class", "environment_id",
	"node_id", "project_id", "role", "run_id", "target_id", "task_id"}
var budgetKeys = []string{"max_calls", "max_cost_usd_micros", "max_input_tokens",
	"max_network_bytes", "max_output_bytes", "max_output_tokens", "timeout_ms"}
var bindingKeys = []string{"context_sha256", "source_revision", "source_tree_sha256"}
var signatureKeys = []string{"key_id", "profile_id", "profile_sha256", "signature_base64url"}
var issuedProjectionKeys = []string{"bindings", "budget", "capability", "grant_envelope_sha256",
	"grant_id", "grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256",
	"grant_policy_sha256", "grant_request_sha256", "grant_sha256", "issuance_trust_epoch",
	"issuance_trust_root_sha256", "resources", "subject",
	"task_binding", "validity"}

type issuedGrant struct{ document map[string]any }

func decodeIssuedProjection(value *bootstrapgrantauthority.IssuedGrantProjection) (*issuedGrant, error) {
	if value == nil {
		return nil, fmt.Errorf("issued Grant projection is required")
	}
	data, err := value.CanonicalJSON()
	if err != nil {
		return nil, err
	}
	document, err := decodeCanonical(data, maxPolicyBytes)
	if err != nil {
		return nil, err
	}
	if err = validateIssuedProjection(document); err != nil {
		return nil, err
	}
	return &issuedGrant{document}, nil
}

func validateIssuedProjection(document map[string]any) error {
	if err := requireKeys(document, issuedProjectionKeys...); err != nil {
		return fmt.Errorf("issued Grant projection: %w", err)
	}
	for _, field := range []string{"grant_envelope_sha256", "grant_issuance_receipt_sha256",
		"grant_policy_sha256", "grant_request_sha256", "grant_sha256",
		"issuance_trust_root_sha256"} {
		if err := validateHashField(document, field, field); err != nil {
			return err
		}
	}
	grantHash, _ := stringValue(document, "grant_sha256")
	if document["grant_id"] != "capability-grant-"+grantHash {
		return fmt.Errorf("issued Grant identity is invalid")
	}
	sequence, sequenceErr := intValue(document, "grant_issuance_ledger_sequence")
	epoch, epochErr := intValue(document, "issuance_trust_epoch")
	if sequenceErr != nil || sequence < 1 || epochErr != nil || epoch < 1 {
		return fmt.Errorf("issued Grant sequence or trust epoch is invalid")
	}
	return validateIssuedParts(document)
}

func validateIssuedParts(document map[string]any) error {
	validators := []func() error{
		func() error { return validateBindings(document["bindings"], "issued bindings") },
		func() error { _, err := validateBudget(document["budget"], "issued budget"); return err },
		func() error { return validateCapability(document["capability"], "issued capability") },
		func() error { _, err := validatePrincipal(document["subject"], "issued subject"); return err },
		func() error { return validateTask(document["task_binding"], "issued task_binding") },
		func() error { return validateGrantValidity(document["validity"]) },
	}
	for _, validator := range validators {
		if err := validator(); err != nil {
			return err
		}
	}
	resources, err := arrayValue(document, "resources")
	if err != nil || len(resources) < 1 || len(resources) > 16 {
		return fmt.Errorf("issued Grant resources are invalid")
	}
	return validateActionResourceOrder(resources)
}

func validatePrincipal(value any, label string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, principalKeys...) != nil {
		return nil, fmt.Errorf("%s fields are invalid", label)
	}
	for _, key := range []string{"authority_domain", "principal_id"} {
		text, err := stringValue(node, key)
		if err != nil || validateText(text, label+" "+key, 160) != nil {
			return nil, fmt.Errorf("%s %s is invalid", label, key)
		}
	}
	principalType, err := stringValue(node, "principal_type")
	if err != nil || !oneOf(principalType, "agent", "service") {
		return nil, fmt.Errorf("%s principal_type is invalid", label)
	}
	return node, nil
}

func validateTask(value any, label string) error {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, taskKeys...) != nil || node["attempt_id"] != nil || node["target_id"] != nil {
		return fmt.Errorf("%s fields are invalid", label)
	}
	for _, key := range []string{"change_id", "environment_id", "node_id", "project_id", "role", "run_id", "task_id"} {
		text, err := stringValue(node, key)
		if err != nil || validateText(text, label+" "+key, 160) != nil {
			return fmt.Errorf("%s %s is invalid", label, key)
		}
	}
	environment, err := stringValue(node, "environment_class")
	if err != nil || !oneOf(environment, "development", "local", "test") {
		return fmt.Errorf("%s environment_class is invalid", label)
	}
	return nil
}

func validateBindings(value any, label string) error {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, bindingKeys...) != nil {
		return fmt.Errorf("%s fields are invalid", label)
	}
	if err := validateHashField(node, "context_sha256", label+" context_sha256"); err != nil {
		return err
	}
	if err := validateHashField(node, "source_tree_sha256", label+" source_tree_sha256"); err != nil {
		return err
	}
	revision, err := stringValue(node, "source_revision")
	if err != nil {
		return err
	}
	return validateText(revision, label+" source_revision", 160)
}

func validateCapability(value any, label string) error {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, "capability_contract_sha256", "capability_id", "capability_version") != nil {
		return fmt.Errorf("%s fields are invalid", label)
	}
	if node["capability_id"] != "repository-reader" || node["capability_version"] != "1" {
		return fmt.Errorf("%s must be repository-reader/v1", label)
	}
	return validateHashField(node, "capability_contract_sha256", label+" capability digest")
}

func validateBudget(value any, label string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, budgetKeys...) != nil {
		return nil, fmt.Errorf("%s fields are invalid", label)
	}
	fixed := map[string]int64{"max_calls": 1, "max_cost_usd_micros": 0,
		"max_input_tokens": 0, "max_network_bytes": 0, "max_output_tokens": 0}
	for key, expected := range fixed {
		actual, err := intValue(node, key)
		if err != nil || actual != expected {
			return nil, fmt.Errorf("%s %s is invalid", label, key)
		}
	}
	output, outputErr := intValue(node, "max_output_bytes")
	timeout, timeoutErr := intValue(node, "timeout_ms")
	if outputErr != nil || output < 0 || output > maxContentBytes || timeoutErr != nil || timeout < 1 || timeout > maxFreshnessMillis {
		return nil, fmt.Errorf("%s output or cooperative timeout is invalid", label)
	}
	return node, nil
}

func validateRequestedAction(value any, claimed any) error {
	action, ok := value.(map[string]any)
	claimedHash, hashOK := claimed.(string)
	if !ok || !hashOK || validateHash(claimedHash, "requested_action_sha256") != nil {
		return fmt.Errorf("requested_action or digest is invalid")
	}
	if _, err := capabilitygrantcontract.CanonicalRequestedActionJSON(action); err != nil {
		return err
	}
	computed, err := capabilitygrantcontract.RequestedActionSHA256(action)
	if err != nil || computed != claimedHash || action["effect_id"] != "repo.read" {
		return fmt.Errorf("requested_action digest or effect is invalid")
	}
	resources, _ := arrayValue(action, "resources")
	if len(resources) < 1 || len(resources) > 16 {
		return fmt.Errorf("requested_action must contain 1..16 resources")
	}
	if err := validateActionResourceOrder(resources); err != nil {
		return err
	}
	usage, _ := objectValue(action, "usage")
	fixed := map[string]int64{"call_count": 1, "cost_usd_micros": 0, "input_tokens": 0,
		"network_bytes": 0, "output_tokens": 0}
	for field, expected := range fixed {
		if usage[field] != expected {
			return fmt.Errorf("requested_action violates bootstrap read-only usage")
		}
	}
	return nil
}

func validateActionResourceOrder(resources []any) error {
	prior := ""
	for index, value := range resources {
		node, ok := value.(map[string]any)
		if !ok || node["match"] != "exact" || node["scope_kind"] != "repo_path" {
			return fmt.Errorf("requested_action resource %d is not exact repo_path", index)
		}
		path, err := stringValue(node, "path")
		if err != nil || validateRepoPath(path) != nil || (index > 0 && prior >= path) {
			return fmt.Errorf("requested_action resource paths are invalid or unsorted")
		}
		prior = path
	}
	return nil
}

func validateBudgetCoversAction(budgetValue, actionValue any) error {
	budget := budgetValue.(map[string]any)
	action := actionValue.(map[string]any)
	usage, _ := objectValue(action, "usage")
	mapping := map[string]string{"call_count": "max_calls", "cost_usd_micros": "max_cost_usd_micros",
		"input_tokens": "max_input_tokens", "network_bytes": "max_network_bytes",
		"output_bytes": "max_output_bytes", "output_tokens": "max_output_tokens", "timeout_ms": "timeout_ms"}
	for usageKey, budgetKey := range mapping {
		used, usedErr := intValue(usage, usageKey)
		limit, limitErr := intValue(budget, budgetKey)
		if usedErr != nil || limitErr != nil || used > limit {
			return fmt.Errorf("requested_action usage exceeds budget")
		}
	}
	return nil
}

func validateGrantValidity(value any) error {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, "expires_at_unix_ms", "issued_at_unix_ms", "not_before_unix_ms", "transferable") != nil || node["transferable"] != false {
		return fmt.Errorf("issued Grant validity is invalid")
	}
	issued, issuedErr := intValue(node, "issued_at_unix_ms")
	start, startErr := intValue(node, "not_before_unix_ms")
	end, endErr := intValue(node, "expires_at_unix_ms")
	if issuedErr != nil || startErr != nil || endErr != nil || issued < 0 || start != issued || end <= start {
		return fmt.Errorf("issued Grant validity interval is invalid")
	}
	return nil
}

func validateSignature(value any, trust *Trust, usage, label string) (map[string]any, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, signatureKeys...) != nil {
		return nil, fmt.Errorf("%s signature fields are invalid", label)
	}
	key := trust.keys[usage]
	if node["key_id"] != key.id || node["profile_id"] != signatureProfile ||
		node["profile_sha256"] != trust.profileHash {
		return nil, fmt.Errorf("%s signature binding is invalid", label)
	}
	text, err := stringValue(node, "signature_base64url")
	if err != nil {
		return nil, err
	}
	if _, err = decodeBase64URL(text, label+" signature", 64); err != nil {
		return nil, err
	}
	return node, nil
}
