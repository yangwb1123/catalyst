package domain

func validateBudget(value BudgetLimit, label string) error {
	if value.MaxAttempts < 1 || value.MaxAttempts > maxAttempts {
		return invalidDomain(label+" max_attempts is outside its bound", nil)
	}
	if value.MaxDurationMS < 1 || value.MaxDurationMS > maxDurationMS {
		return invalidDomain(label+" max_duration_ms is outside its bound", nil)
	}
	if value.MaxCostMicroUSD < 0 || value.MaxCostMicroUSD > maxCostMicroUSD {
		return invalidDomain(label+" max_cost_micro_usd is outside its bound", nil)
	}
	return nil
}

func addWithin(total, value, maximum int64) (int64, bool) {
	if value < 0 || total < 0 || maximum < 0 || total > maximum || value > maximum-total {
		return 0, false
	}
	return total + value, true
}
