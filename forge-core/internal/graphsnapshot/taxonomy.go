package graphsnapshot

import "fmt"

type relationRule struct {
	allowedAxes []string
	from        []string
	to          []string
	sameType    bool
}

var nodeFamilies = map[string][]string{
	"any": {
		"actor", "adr", "aggregate", "api", "bounded_context", "business_capability",
		"business_rule", "column", "debt", "deployment_unit", "domain_event", "entity",
		"environment", "event_contract", "gate", "incident", "job", "journey", "module",
		"owner", "package", "policy", "queue", "requirement", "runtime_signal", "schema",
		"symbol", "table", "test", "use_case", "value_object",
	},
	"business":             {"actor", "business_capability", "business_rule", "journey", "requirement"},
	"code":                 {"job", "module", "package", "symbol"},
	"container":            {"bounded_context", "business_capability", "deployment_unit", "environment", "module", "schema"},
	"contract":             {"api", "event_contract"},
	"data":                 {"column", "queue", "schema", "table"},
	"delivery":             {"deployment_unit", "environment"},
	"domain":               {"aggregate", "bounded_context", "domain_event", "entity", "use_case", "value_object"},
	"executable":           {"api", "job", "module", "package", "symbol", "use_case"},
	"governance":           {"adr", "debt", "gate", "owner", "policy"},
	"verification_runtime": {"incident", "runtime_signal", "test"},
}

var relationRules = map[string]relationRule{
	"affects":        rule("any", "any", "data", "ownership", "policy", "runtime", "static_source", "structural", "verification"),
	"calls":          rule("executable", "api|job|module|package|symbol|use_case", "runtime", "static_source"),
	"constrained_by": rule("any", "business_rule|gate|policy|requirement", "policy"),
	"consumes":       rule("executable", "api|event_contract|queue", "runtime", "static_source"),
	"contains":       rule("container", "any", "structural"),
	"decided_by":     rule("any", "adr", "policy"),
	"depends_on":     rule("any", "any", "data", "policy", "runtime", "static_source", "structural", "verification"),
	"deployed_as":    rule("api|job|module|package", "deployment_unit", "structural"),
	"exposes":        rule("module|package|deployment_unit", "api|event_contract", "static_source", "structural"),
	"governed_by":    rule("any", "adr|policy", "policy"),
	"implements":     rule("code|contract", "api|business_rule|event_contract|requirement|use_case", "static_source", "structural"),
	"observed_by":    rule("any", "runtime_signal|test", "runtime", "verification"),
	"owns":           rule("actor|owner", "any", "ownership"),
	"persists_to":    rule("aggregate|entity|executable", "queue|schema|table", "data", "runtime", "static_source"),
	"publishes":      rule("domain_event|event_contract|executable", "event_contract|queue", "runtime", "static_source"),
	"reads":          rule("executable", "column|queue|runtime_signal|schema|table", "data", "runtime", "static_source"),
	"realizes":       rule("code|contract|domain", "business|domain", "static_source", "structural"),
	"supersedes":     sameTypeRule("policy", "structural"),
	"verified_by":    rule("any", "gate|runtime_signal|test", "verification"),
	"writes":         rule("executable", "column|queue|runtime_signal|schema|table", "data", "runtime", "static_source"),
}

func rule(from, to string, axes ...string) relationRule {
	return relationRule{allowedAxes: axes, from: expandFamilies(from), to: expandFamilies(to)}
}

func sameTypeRule(axes ...string) relationRule {
	return relationRule{allowedAxes: axes, from: nodeFamilies["any"], to: nodeFamilies["any"], sameType: true}
}

func expandFamilies(value string) []string {
	result := []string{}
	for _, part := range splitPipe(value) {
		if family, ok := nodeFamilies[part]; ok {
			result = append(result, family...)
		} else {
			result = append(result, part)
		}
	}
	return uniqueSorted(result)
}

func validateEdgeTaxonomy(edge Edge, nodes map[string]Node) error {
	from, fromOK := nodes[edge.FromNodeID]
	to, toOK := nodes[edge.ToNodeID]
	rule, ruleOK := relationRules[edge.Relation]
	if !fromOK || !toOK || !ruleOK {
		return fmt.Errorf("edge endpoint or relation is outside the taxonomy")
	}
	if !contains(rule.from, from.NodeType) || !contains(rule.to, to.NodeType) ||
		rule.sameType && from.NodeType != to.NodeType {
		return fmt.Errorf("edge direction violates relation endpoint families")
	}
	if !strictSubset(edge.CategoryAxes, rule.allowedAxes) {
		return fmt.Errorf("edge axes violate relation taxonomy")
	}
	return nil
}

func strictSubset(values, allowed []string) bool {
	if len(values) == 0 || len(values) > len(allowed) {
		return false
	}
	for index, value := range values {
		if index > 0 && values[index-1] >= value || !contains(allowed, value) {
			return false
		}
	}
	return true
}

func contains(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}
