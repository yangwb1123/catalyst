package domain

func validateTextList(
	values []string, label string, minimum, maximum, itemMaximum int,
) error {
	if len(values) < minimum || len(values) > maximum {
		return invalidDomain(label+" count is outside its bound", nil)
	}
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if err := validateText(value, label+" item", 1, itemMaximum); err != nil {
			return err
		}
		if _, exists := seen[value]; exists {
			return invalidDomain(label+" contains a duplicate", nil)
		}
		seen[value] = struct{}{}
	}
	return nil
}

func validateTokenList(
	values []string, label string, minimum, maximum, itemMaximum int,
) error {
	if len(values) < minimum || len(values) > maximum {
		return invalidDomain(label+" count is outside its bound", nil)
	}
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if err := validateToken(value, label+" item", itemMaximum); err != nil {
			return err
		}
		if _, exists := seen[value]; exists {
			return invalidDomain(label+" contains a duplicate", nil)
		}
		seen[value] = struct{}{}
	}
	return nil
}

func validateEntityIDList(
	values []string, entityType, label string, minimum, maximum int,
) error {
	if len(values) < minimum || len(values) > maximum {
		return invalidDomain(label+" count is outside its bound", nil)
	}
	seen := make(map[string]struct{}, len(values))
	for _, value := range values {
		if err := validateEntityID(value, entityType, label+" item"); err != nil {
			return err
		}
		if _, exists := seen[value]; exists {
			return invalidDomain(label+" contains a duplicate", nil)
		}
		seen[value] = struct{}{}
	}
	return nil
}
