package migrationtxn

import (
	"fmt"

	"forgeos/forge-core/internal/migrate"
)

// PreviewManual plans the declared explorer -> engineering governance
// migration without writing repository or control state.
func PreviewManual(root string) (Result, error) {
	return previewOperation(root, prepareManualMigration)
}

// ApplyManual durably applies or recovers the declared explorer -> engineering
// governance migration under the shared repository mutation lock.
func ApplyManual(root string) (Result, error) {
	return applyOperation(root, manualModeOperationID, prepareManualMigration)
}

func prepareManualMigration(
	root string,
	ops fileOps,
) (Result, *promotionIntent, error) {
	project, selectors, err := readProjectSelectors(root, ops)
	if err != nil {
		return Result{}, nil, err
	}
	receipt, present, err := loadReceiptForOperation(
		root, manualModeOperationID, ops,
	)
	if err != nil {
		return Result{}, nil, err
	}
	switch selectors.mode {
	case migrate.ModeExplorer:
		if present {
			return Result{}, nil, fmt.Errorf(
				"migrationtxn: manual terminal receipt conflicts with explorer project state",
			)
		}
		return prepareManualExplorer(root, project, selectors, ops)
	case migrate.ModeEngineering:
		return prepareManualEngineering(root, selectors, receipt, present, ops)
	default:
		return Result{}, nil, fmt.Errorf(
			"migrationtxn: manual migration requires persistent mode %q; found %q",
			migrate.ModeExplorer, selectors.mode,
		)
	}
}

func readProjectSelectors(
	root string,
	ops fileOps,
) (fileImage, projectSelectors, error) {
	project, err := ops.readTracked(projectPath(root), projectMaxBytes)
	if err != nil {
		return fileImage{}, projectSelectors{},
			fmt.Errorf("migrationtxn: read project.yml: %w", err)
	}
	if !project.Present {
		return fileImage{}, projectSelectors{},
			fmt.Errorf("migrationtxn: project.yml is required")
	}
	selectors, err := strictProjectSelectors(project.Data)
	if err != nil {
		return fileImage{}, projectSelectors{},
			fmt.Errorf("migrationtxn: project selector: %w", err)
	}
	return project, selectors, nil
}

func prepareManualExplorer(
	root string,
	project fileImage,
	selectors projectSelectors,
	ops fileOps,
) (Result, *promotionIntent, error) {
	receipt := manualReceipt(selectors.lifecycle)
	result := resultFromReceipt(receipt, StatusPlanned)
	intent, err := buildIntentFromReceipt(
		root, project, selectors, receipt,
		migrate.ExplorerToEngineering().Tasks, ops,
	)
	if err != nil {
		return Result{}, nil, err
	}
	return result, &intent, nil
}

func prepareManualEngineering(
	root string,
	selectors projectSelectors,
	receipt promotionReceipt,
	present bool,
	ops fileOps,
) (Result, *promotionIntent, error) {
	if !present {
		return Result{
			Status:   StatusNoop,
			FromMode: selectors.mode, ToMode: selectors.mode,
			FromLifecycle: selectors.lifecycle, ToLifecycle: selectors.lifecycle,
		}, nil, nil
	}
	if err := validateReceiptState(
		root, receipt, selectors.mode, selectors.lifecycle, ops,
	); err != nil {
		return Result{}, nil, err
	}
	return resultFromReceipt(receipt, StatusReplayed), nil, nil
}

func manualReceipt(lifecycle string) promotionReceipt {
	tasks := migrate.ExplorerToEngineering().Tasks
	return promotionReceipt{
		Format: receiptFormat, Operation: manualModeOperationID,
		FromMode: migrate.ModeExplorer, ToMode: migrate.ModeEngineering,
		FromLifecycle: lifecycle, ToLifecycle: lifecycle,
		RoadmapMigration: true,
		TaskIDs:          promotionTaskIDsFromTasks(tasks),
	}
}
