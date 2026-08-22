package adrv2

import (
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"unicode/utf8"
)

var (
	adrIDPattern      = regexp.MustCompile(`^ADR-[0-9]{4}$`)
	namePattern       = regexp.MustCompile(`^ADR-[0-9]{4}-[a-z0-9]+(?:-[a-z0-9]+)*\.md$`)
	hashPattern       = regexp.MustCompile(`^[a-f0-9]{64}$`)
	identifierPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)
	affectedPattern   = regexp.MustCompile(`^graph-node-[a-f0-9]{64}$`)
	pathPattern       = regexp.MustCompile(`^[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*(?:#L[1-9][0-9]*)?$`)
)

func validateSemantics(filename string, value *Frontmatter) error {
	if value.APIVersion != APIVersion || value.Kind != Kind ||
		value.Canonicalization != Canonicalization || value.Status != Status {
		return fmt.Errorf("ADR v2 vocabulary constants are invalid")
	}
	if value.AcceptedAtUnixMS != nil || value.AcceptanceID != nil || len(value.SupersededBy) != 0 {
		return fmt.Errorf("proposed-only acceptance and superseded_by fields must be null/empty")
	}
	if value.ProposedAtUnixMS < 0 {
		return fmt.Errorf("proposed_at_unix_ms must be nonnegative")
	}
	if value.ExpiresAtUnixMS != nil && *value.ExpiresAtUnixMS <= value.ProposedAtUnixMS {
		return fmt.Errorf("expires_at_unix_ms must be null or later than proposed_at_unix_ms")
	}
	if err := validateIdentity(filename, value); err != nil {
		return err
	}
	if err := validateTopLevelText(value); err != nil {
		return err
	}
	if err := validateReferenceLists(value); err != nil {
		return err
	}
	if err := validateAlternatives(value.Alternatives); err != nil {
		return err
	}
	if err := validateRisks(value.Risks); err != nil {
		return err
	}
	if err := validateValidationPlan(value.ValidationPlan, value.OwnerRefs); err != nil {
		return err
	}
	return validateRevisitTriggers(value.RevisitTriggers)
}

func validateIdentity(filename string, value *Frontmatter) error {
	if len(filename) > 255 || len(value.DocumentName) > 255 ||
		filename != value.DocumentName || !namePattern.MatchString(filename) {
		return fmt.Errorf("document_name must equal the canonical ADR filename")
	}
	if !adrIDPattern.MatchString(value.ADRID) || value.ADRID == "ADR-0000" ||
		filename[4:8] != value.ADRID[4:8] {
		return fmt.Errorf("adr_id must be ADR-0001..ADR-9999 and match document_name")
	}
	if err := narrative("title", value.Title, 160); err != nil {
		return err
	}
	for name, digest := range map[string]string{
		"body_sha256": value.BodySHA256, "self_sha256": value.SelfSHA256,
	} {
		if !hashPattern.MatchString(digest) {
			return fmt.Errorf("%s must be a lowercase bare SHA-256", name)
		}
	}
	return nil
}

func validateTopLevelText(value *Frontmatter) error {
	for name, text := range map[string]string{
		"compatibility": value.Compatibility, "decision": value.Decision,
		"rollback": value.Rollback, "rollout": value.Rollout,
	} {
		if err := narrative(name, text, 4096); err != nil {
			return err
		}
	}
	if len(value.Consequences) == 0 {
		return fmt.Errorf("consequences must be nonempty")
	}
	for index, text := range value.Consequences {
		if err := narrative(fmt.Sprintf("consequences[%d]", index), text, 4096); err != nil {
			return err
		}
	}
	return nil
}

func validateReferenceLists(value *Frontmatter) error {
	lists := []struct {
		name     string
		values   []string
		required bool
	}{
		{"owner_refs", value.OwnerRefs, true}, {"approver_refs", value.ApproverRefs, true},
		{"scope_refs", value.ScopeRefs, true}, {"context_claim_ids", value.ContextClaimIDs, false},
		{"decision_driver_claim_ids", value.DecisionDriverClaimIDs, false},
		{"assumption_claim_ids", value.AssumptionClaimIDs, false},
		{"evidence_record_ids", value.EvidenceRecordIDs, false},
	}
	for _, list := range lists {
		if err := identifierSet(list.name, list.values, list.required); err != nil {
			return err
		}
	}
	if err := matchedSet("affected_node_ids", value.AffectedNodeIDs, affectedPattern); err != nil {
		return err
	}
	if err := implementationSet(value.ImplementationRefs); err != nil {
		return err
	}
	return supersedesSet(value.ADRID, value.Supersedes)
}

func identifierSet(name string, values []string, required bool) error {
	if required && len(values) == 0 {
		return fmt.Errorf("%s must be nonempty", name)
	}
	for _, value := range values {
		if len(value) > 160 || !identifierPattern.MatchString(value) {
			return fmt.Errorf("%s contains invalid declared identifier %q", name, value)
		}
	}
	return sortedUnique(name, values)
}

func matchedSet(name string, values []string, pattern *regexp.Regexp) error {
	for _, value := range values {
		if !pattern.MatchString(value) {
			return fmt.Errorf("%s contains invalid reference %q", name, value)
		}
	}
	return sortedUnique(name, values)
}

func implementationSet(values []string) error {
	for _, value := range values {
		if len(value) > 4096 || !pathPattern.MatchString(value) || invalidPathSegments(value) {
			return fmt.Errorf("implementation_refs contains invalid repository path %q", value)
		}
		if marker := strings.LastIndex(value, "#L"); marker >= 0 {
			line, err := strconv.ParseInt(value[marker+2:], 10, 32)
			if err != nil || line < 1 {
				return fmt.Errorf("implementation_refs line is outside 1..2147483647")
			}
		}
	}
	return sortedUnique("implementation_refs", values)
}

func invalidPathSegments(value string) bool {
	path, _, _ := strings.Cut(value, "#L")
	segments := strings.Split(path, "/")
	if segments[0] == ".git" || segments[0] == ".forge" {
		return true
	}
	for _, segment := range segments {
		if segment == "." || segment == ".." {
			return true
		}
	}
	return false
}

func supersedesSet(self string, values []string) error {
	for _, value := range values {
		if !adrIDPattern.MatchString(value) || value == "ADR-0000" || value == self {
			return fmt.Errorf("supersedes contains invalid or self ADR reference %q", value)
		}
	}
	return sortedUnique("supersedes", values)
}

func validateAlternatives(values []Alternative) error {
	if len(values) == 0 {
		return fmt.Errorf("alternatives must be nonempty")
	}
	ids := make([]string, len(values))
	candidates, rejected := 0, 0
	for index, value := range values {
		ids[index] = value.AlternativeID
		if err := identifier("alternative_id", value.AlternativeID); err != nil {
			return err
		}
		if err := narratives(value.Description, value.Rationale); err != nil {
			return fmt.Errorf("alternatives[%d]: %w", index, err)
		}
		switch value.Disposition {
		case "candidate":
			candidates++
		case "rejected":
			rejected++
		default:
			return fmt.Errorf("alternatives[%d].disposition is invalid", index)
		}
	}
	if candidates == 0 || rejected == 0 {
		return fmt.Errorf("alternatives require at least one candidate and one rejected option")
	}
	return sortedUnique("alternative_id", ids)
}

func validateRisks(values []Risk) error {
	ids := make([]string, len(values))
	for index, value := range values {
		ids[index] = value.RiskID
		if err := identifier("risk_id", value.RiskID); err != nil {
			return err
		}
		if err := narratives(value.Description, value.Mitigation); err != nil {
			return fmt.Errorf("risks[%d]: %w", index, err)
		}
	}
	return sortedUnique("risk_id", ids)
}

func validateValidationPlan(values []ValidationItem, owners []string) error {
	if len(values) == 0 {
		return fmt.Errorf("validation_plan must be nonempty")
	}
	ownerSet := make(map[string]bool, len(owners))
	for _, owner := range owners {
		ownerSet[owner] = true
	}
	ids := make([]string, len(values))
	for index, value := range values {
		ids[index] = value.ValidationID
		if err := identifier("validation_id", value.ValidationID); err != nil || !ownerSet[value.OwnerRef] {
			return fmt.Errorf("validation_plan[%d] has invalid id or undeclared owner_ref", index)
		}
		if err := narratives(value.Description, value.DueTrigger, value.SuccessCriteria); err != nil {
			return fmt.Errorf("validation_plan[%d]: %w", index, err)
		}
		if err := narrativeList("validation evidence_required", value.EvidenceRequired); err != nil {
			return err
		}
	}
	return sortedUnique("validation_id", ids)
}

func validateRevisitTriggers(values []RevisitTrigger) error {
	if len(values) == 0 {
		return fmt.Errorf("revisit_triggers must be nonempty")
	}
	ids := make([]string, len(values))
	for index, value := range values {
		ids[index] = value.TriggerID
		if err := identifier("trigger_id", value.TriggerID); err != nil {
			return err
		}
		if err := narrative("condition", value.Condition, 4096); err != nil {
			return err
		}
		if err := narrativeList(fmt.Sprintf("revisit_triggers[%d].evidence_required", index), value.EvidenceRequired); err != nil {
			return err
		}
	}
	return sortedUnique("trigger_id", ids)
}

func narrativeList(name string, values []string) error {
	if len(values) == 0 {
		return fmt.Errorf("%s must be nonempty", name)
	}
	for index, value := range values {
		if err := narrative(fmt.Sprintf("%s[%d]", name, index), value, 4096); err != nil {
			return err
		}
	}
	return nil
}

func narratives(values ...string) error {
	for _, value := range values {
		if err := narrative("narrative", value, 4096); err != nil {
			return err
		}
	}
	return nil
}

func narrative(name, value string, maximum int) error {
	if !utf8.ValidString(value) || strings.TrimSpace(value) == "" || len(value) > maximum {
		return fmt.Errorf("%s must contain 1..%d UTF-8 bytes", name, maximum)
	}
	return validateJSONText(value)
}

func identifier(name, value string) error {
	if len(value) > 160 || !identifierPattern.MatchString(value) {
		return fmt.Errorf("%s is not a valid identifier", name)
	}
	return nil
}

func sortedUnique(name string, values []string) error {
	if !sort.StringsAreSorted(values) {
		return fmt.Errorf("%s must be raw-UTF-8 sorted", name)
	}
	for index := 1; index < len(values); index++ {
		if values[index] == values[index-1] {
			return fmt.Errorf("%s contains duplicate %q", name, values[index])
		}
	}
	return nil
}
