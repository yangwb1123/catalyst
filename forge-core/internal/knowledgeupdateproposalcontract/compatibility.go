package knowledgeupdateproposalcontract

import (
	"fmt"
	"sort"

	"forgeos/forge-core/internal/capabilitygrantcontract"
	"forgeos/forge-core/internal/contextpackagecontract"
)

// ProjectArtifactResources adds only ADR-0056 artifact scope_kind declarations.
// It does not read artifacts or verify digest preimages.
func ProjectArtifactResources(proposal map[string]any) ([]any, error) {
	if err := validateProposal(proposal); err != nil {
		return nil, err
	}
	artifacts := proposal["bindings"].(map[string]any)["artifacts"].([]any)
	resources := make([]any, len(artifacts))
	for index, value := range artifacts {
		resource := cloneNode(value.(map[string]any))
		resource["scope_kind"] = "artifact"
		resources[index] = resource
	}
	if err := validateSortedNodes(resources, "artifact resources"); err != nil {
		return nil, err
	}
	return resources, nil
}

// ProjectCapabilityGrantRef projects only the declared issuer domain and grant identity.
func ProjectCapabilityGrantRef(grant map[string]any) (map[string]any, error) {
	if _, err := capabilitygrantcontract.CanonicalGrantJSON(grant); err != nil {
		return nil, err
	}
	proof, ok := grant["authority_proof"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("CapabilityGrant authority_proof is invalid")
	}
	issuer, ok := proof["issuer"].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("CapabilityGrant issuer is invalid")
	}
	return map[string]any{
		"authority_domain": issuer["authority_domain"], "grant_id": grant["grant_id"],
		"grant_sha256": grant["grant_sha256"],
	}, nil
}

// AssessDeclaredGrantCompatibility compares complete, structurally valid Grant
// declarations. It does not authenticate the issuer or confer permission.
func AssessDeclaredGrantCompatibility(grant, proposal map[string]any) (map[string]any, error) {
	grantRef, err := ProjectCapabilityGrantRef(grant)
	if err != nil {
		return nil, err
	}
	if err := validateProposal(proposal); err != nil {
		return nil, err
	}
	scope := grant["scope"].(map[string]any)
	effectMatches := scope["effect_id"] == "knowledge.propose"
	scopeRelation, scopeReason := assessKnowledgeScope(scope, proposal["knowledge_scope"].(map[string]any), effectMatches)
	validity := grant["validity"].(map[string]any)
	submitted := proposal["submitted_at_unix_ms"].(int64)
	timeMatches := validity["not_before_unix_ms"].(int64) <= submitted &&
		submitted < validity["expires_at_unix_ms"].(int64)
	relations := map[string]any{
		"bindings":      sameRelation(proposalCompatibilityBindings(proposal), grantCompatibilityBindings(grant), "bindings"),
		"declared_time": relation(timeMatches, "same_declared_time", "declared_time_mismatch"),
		"effect":        relation(effectMatches, "same_declared_effect", "effect_mismatch"),
		"grant_ref":     sameRelation(proposal["capability_grant_ref"], grantRef, "grant_ref"),
		"proposer":      sameRelation(proposal["proposer"], grant["subject"], "proposer"),
		"scope":         scopeRelation,
		"task_binding":  sameRelation(proposal["task_binding"], grant["task_binding"], "task_binding"),
	}
	reasons := compatibilityReasons(relations, scopeReason)
	return map[string]any{
		"reason_codes": stringsToAny(reasons), "relations": relations, "result": grantCompatibilityResult,
	}, nil
}

func assessKnowledgeScope(scope, resource map[string]any, effectMatches bool) (string, string) {
	if !effectMatches {
		return "outside_declared_scope", ""
	}
	for _, value := range scope["deny"].([]any) {
		if canonicalValuesEqual(value, resource) {
			return "denied_by_declaration", "deny_matched"
		}
	}
	for _, clauseValue := range scope["allow"].([]any) {
		clause := clauseValue.(map[string]any)
		for _, value := range clause["resources"].([]any) {
			if canonicalValuesEqual(value, resource) {
				return "covered_by_declaration", ""
			}
		}
	}
	return "outside_declared_scope", "scope_not_covered"
}

func grantCompatibilityBindings(grant map[string]any) map[string]any {
	return commonCompatibilityBindings(grant["bindings"].(map[string]any))
}

func proposalCompatibilityBindings(proposal map[string]any) map[string]any {
	return commonCompatibilityBindings(proposal["bindings"].(map[string]any))
}

func commonCompatibilityBindings(bindings map[string]any) map[string]any {
	keys := []string{"context_sha256", "impact_sha256", "plan_sha256", "policy_sha256",
		"risk_sha256", "source_revision", "source_tree_sha256"}
	result := make(map[string]any, len(keys))
	for _, key := range keys {
		result[key] = cloneValue(bindings[key])
	}
	return result
}

func compatibilityReasons(relations map[string]any, extra string) []string {
	reasons := make([]string, 0, len(relations)+1)
	for _, value := range relations {
		text, ok := value.(string)
		if !ok {
			continue
		}
		switch text {
		case "bindings_mismatch", "context_mismatch", "declared_time_mismatch", "effect_mismatch",
			"grant_ref_mismatch", "policy_mismatch", "proposer_mismatch",
			"source_mismatch", "task_binding_mismatch":
			reasons = append(reasons, text)
		case "outside_declared_freshness":
			reasons = append(reasons, "freshness_mismatch")
		}
	}
	if extra != "" {
		reasons = append(reasons, extra)
	}
	sort.Strings(reasons)
	return reasons
}

// AssessDeclaredContextCompatibility first verifies exact ContextPackage
// reassembly from the caller's request and counter, then compares declarations.
// Reassembly authenticates no source and does not turn Context into authority.
func AssessDeclaredContextCompatibility(request *contextpackagecontract.BuildRequest,
	packageValue *contextpackagecontract.ContextPackage, counter contextpackagecontract.TokenCounter,
	proposal map[string]any) (map[string]any, error) {
	if request == nil || packageValue == nil || counter == nil {
		return nil, fmt.Errorf("Context request, package, and token counter must not be nil")
	}
	if err := contextpackagecontract.ValidatePackage(request, packageValue, counter); err != nil {
		return nil, fmt.Errorf("ContextPackage exact reassembly: %w", err)
	}
	if err := validateProposal(proposal); err != nil {
		return nil, err
	}
	bindings := proposal["bindings"].(map[string]any)
	proposalTask := proposal["task_binding"].(map[string]any)
	contextTask := map[string]any{
		"change_id": packageValue.TaskBinding.ChangeID, "node_id": packageValue.TaskBinding.NodeID,
		"project_id": packageValue.TaskBinding.ProjectID, "role": packageValue.TaskBinding.Role,
		"run_id": packageValue.TaskBinding.RunID, "task_id": packageValue.TaskBinding.TaskID,
	}
	sharedTask := projectSharedTask(proposalTask)
	submitted := proposal["submitted_at_unix_ms"].(int64)
	fresh := packageValue.Freshness.EvaluatedAtUnixMS <= submitted &&
		(packageValue.Freshness.ExpiresAtUnixMS == nil || submitted < *packageValue.Freshness.ExpiresAtUnixMS)
	relations := map[string]any{
		"context": relation(bindings["context_sha256"] == packageValue.ContextSHA256,
			"same_declared_context", "context_mismatch"),
		"freshness": relation(fresh, "inside_declared_freshness", "outside_declared_freshness"),
		"policy": relation(bindings["policy_sha256"] == packageValue.SourceBinding.PolicySHA256,
			"same_declared_policy", "policy_mismatch"),
		"source": relation(bindings["source_revision"] == packageValue.SourceBinding.SourceRevision &&
			bindings["source_tree_sha256"] == packageValue.SourceBinding.SourceTreeSHA256,
			"same_declared_source", "source_mismatch"),
		"task_binding": sameRelation(sharedTask, contextTask, "task_binding"),
	}
	reasons := compatibilityReasons(relations, "")
	return map[string]any{
		"reason_codes": stringsToAny(reasons), "relations": relations, "result": contextCompatibilityResult,
	}, nil
}

func projectSharedTask(task map[string]any) map[string]any {
	keys := []string{"change_id", "node_id", "project_id", "role", "run_id", "task_id"}
	result := make(map[string]any, len(keys))
	for _, key := range keys {
		result[key] = task[key]
	}
	return result
}
