package domain

import "sort"

type criterionIndex map[string][]string

func indexCriteria(values []AcceptanceCriterion) criterionIndex {
	result := make(criterionIndex, len(values))
	for _, value := range values {
		result[value.CriterionID] = value.VerificationRequirements
	}
	return result
}

func validateCriterionRefs(
	values, itemRequirements []string,
	criteria criterionIndex,
	covered, verified map[string]bool,
) error {
	if err := validateTokenList(
		values, "WorkItem criterion refs", 1, maxItemListEntries, 64,
	); err != nil {
		return err
	}
	declared := stringSet(itemRequirements...)
	for _, criterionID := range values {
		requirements, exists := criteria[criterionID]
		if !exists {
			return invalidDomain("WorkItem criterion ref does not exist", nil)
		}
		covered[criterionID] = true
		for _, requirement := range requirements {
			if declared[requirement] {
				verified[criterionRequirementKey(criterionID, requirement)] = true
			}
		}
	}
	return nil
}

func validateCriterionCoverage(
	criteria criterionIndex, covered, verified map[string]bool,
) error {
	for _, criterionID := range sortedCriterionIDs(criteria) {
		requirements := criteria[criterionID]
		if !covered[criterionID] {
			return invalidDomain("WorkGraph does not cover every acceptance criterion", nil)
		}
		for _, requirement := range requirements {
			if !verified[criterionRequirementKey(criterionID, requirement)] {
				return invalidDomain("WorkGraph does not cover criterion verification", nil)
			}
		}
	}
	return nil
}

func sortedCriterionIDs(criteria criterionIndex) []string {
	result := make([]string, 0, len(criteria))
	for criterionID := range criteria {
		result = append(result, criterionID)
	}
	sort.Strings(result)
	return result
}

func criterionRequirementKey(criterionID, requirement string) string {
	return criterionID + "\x00" + requirement
}
