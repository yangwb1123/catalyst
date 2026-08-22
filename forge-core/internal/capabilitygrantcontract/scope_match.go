package capabilitygrantcontract

import "strings"

func assessScope(scope, action map[string]any) (string, string) {
	grantEffect, _ := stringValue(scope, "effect_id")
	actionEffect, _ := stringValue(action, "effect_id")
	if grantEffect != actionEffect {
		return "outside_declared_scope", "effect_mismatch"
	}
	actionResources, _ := arrayValue(action, "resources")
	deny, _ := arrayValue(scope, "deny")
	if anySelectorMatches(deny, actionResources) {
		return "denied_by_declaration", "deny_matched"
	}
	allow, _ := arrayValue(scope, "allow")
	for _, clauseValue := range allow {
		clause := clauseValue.(map[string]any)
		selectors, _ := arrayValue(clause, "resources")
		if clauseCoversAction(grantEffect, selectors, actionResources) {
			return "covered_by_declaration", ""
		}
	}
	return "outside_declared_scope", "scope_not_covered"
}

func clauseCoversAction(effectID string, selectors, resources []any) bool {
	if !allResourcesCovered(selectors, resources) {
		return false
	}
	return effectID != "migration.generate" || !containsResourceKind(selectors, "environment") ||
		containsResourceKind(resources, "environment")
}

func containsResourceKind(resources []any, target string) bool {
	for _, value := range resources {
		resource := value.(map[string]any)
		kind, _ := stringValue(resource, "scope_kind")
		if kind == target {
			return true
		}
	}
	return false
}

func anySelectorMatches(selectors, resources []any) bool {
	for _, selectorValue := range selectors {
		selector := selectorValue.(map[string]any)
		for _, resourceValue := range resources {
			if resourceMatches(selector, resourceValue.(map[string]any)) {
				return true
			}
		}
	}
	return false
}

func allResourcesCovered(selectors, resources []any) bool {
	for _, resourceValue := range resources {
		resource := resourceValue.(map[string]any)
		covered := false
		for _, selectorValue := range selectors {
			if resourceMatches(selectorValue.(map[string]any), resource) {
				covered = true
				break
			}
		}
		if !covered {
			return false
		}
	}
	return true
}

func resourceMatches(selector, resource map[string]any) bool {
	selectorKind, _ := stringValue(selector, "scope_kind")
	resourceKind, _ := stringValue(resource, "scope_kind")
	if selectorKind != resourceKind {
		return false
	}
	if selectorKind == "repo_path" {
		return repoPathMatches(selector, resource)
	}
	left, leftErr := canonicalJSON(selector)
	right, rightErr := canonicalJSON(resource)
	return leftErr == nil && rightErr == nil && string(left) == string(right)
}

func repoPathMatches(selector, resource map[string]any) bool {
	selectorPath, _ := stringValue(selector, "path")
	resourcePath, _ := stringValue(resource, "path")
	match, _ := stringValue(selector, "match")
	if match == "exact" {
		return selectorPath == resourcePath
	}
	if selectorPath == "." {
		return true
	}
	return resourcePath == selectorPath || strings.HasPrefix(resourcePath, selectorPath+"/")
}
