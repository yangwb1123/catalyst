package migrationtxn

import "fmt"

type StateSummary struct {
	Pending    bool
	Operations []string
}

// InspectState returns bounded, side-effect-free migration status. Pending
// intent contents remain opaque; terminal receipts are decoded canonically.
func InspectState(root string) (StateSummary, error) {
	pending, err := Pending(root)
	if err != nil {
		return StateSummary{}, err
	}
	summary := StateSummary{Pending: pending}
	ops := realFileOps{}
	for _, operation := range []string{
		promotionOperationID,
		manualModeOperationID,
	} {
		_, present, err := loadReceiptForOperation(root, operation, ops)
		if err != nil {
			return StateSummary{}, err
		}
		if present {
			summary.Operations = append(summary.Operations, operation)
		}
	}
	if !pending {
		if err := ValidateExecutionState(root); err != nil {
			return StateSummary{}, err
		}
	}
	return summary, nil
}

// ValidateExecutionState enforces completed migration receipts as persistent
// governance floors before run/evolve consume project selectors. Repositories
// without migration receipts retain legacy optional-project defaults.
func ValidateExecutionState(root string) error {
	ops := realFileOps{}
	lifecycleReceipt, lifecyclePresent, err := loadReceiptForOperation(
		root, promotionOperationID, ops,
	)
	if err != nil {
		return fmt.Errorf("migrationtxn: validate lifecycle receipt: %w", err)
	}
	manualReceipt, manualPresent, err := loadReceiptForOperation(
		root, manualModeOperationID, ops,
	)
	if err != nil {
		return fmt.Errorf("migrationtxn: validate manual receipt: %w", err)
	}
	if !lifecyclePresent && !manualPresent {
		return nil
	}
	_, selectors, err := readProjectSelectors(root, ops)
	if err != nil {
		return fmt.Errorf("migrationtxn: migrated project state is invalid: %w", err)
	}
	if lifecyclePresent {
		if err := validateReceiptState(
			root, lifecycleReceipt, selectors.mode, selectors.lifecycle, ops,
		); err != nil {
			return fmt.Errorf("migrationtxn: lifecycle receipt state drift: %w", err)
		}
	}
	if manualPresent {
		if err := validateReceiptState(
			root, manualReceipt, selectors.mode, selectors.lifecycle, ops,
		); err != nil {
			return fmt.Errorf("migrationtxn: manual receipt state drift: %w", err)
		}
	}
	return nil
}

func validateOtherReceiptsForIntent(
	root string,
	intent promotionIntent,
	ops fileOps,
) error {
	before, err := strictProjectSelectors(intent.ProjectBefore.Data)
	if err != nil {
		return fmt.Errorf("migrationtxn: validate intent source selectors: %w", err)
	}
	for _, operation := range []string{
		promotionOperationID,
		manualModeOperationID,
	} {
		if operation == intent.Operation {
			continue
		}
		receipt, present, err := loadReceiptForOperation(root, operation, ops)
		if err != nil {
			return fmt.Errorf("migrationtxn: validate other terminal receipt: %w", err)
		}
		if !present {
			continue
		}
		if err := validateReceiptState(
			root, receipt, before.mode, before.lifecycle, ops,
		); err != nil {
			return fmt.Errorf("migrationtxn: other terminal receipt conflicts with intent: %w", err)
		}
	}
	return nil
}
