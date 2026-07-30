package migrationtxn

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"

	"forgeos/forge-core/internal/migrate"
)

func encodeIntent(intent promotionIntent) ([]byte, error) {
	if err := validateIntent(intent); err != nil {
		return nil, fmt.Errorf("migrationtxn: invalid intent: %w", err)
	}
	return marshalCanonical(intent)
}

func decodeIntent(data []byte) (promotionIntent, error) {
	var intent promotionIntent
	if err := unmarshalCanonical(data, &intent); err != nil {
		return promotionIntent{}, err
	}
	if err := validateIntent(intent); err != nil {
		return promotionIntent{}, err
	}
	canonical, err := marshalCanonical(intent)
	if err != nil || !bytes.Equal(canonical, data) {
		return promotionIntent{}, fmt.Errorf("intent is not in canonical encoding")
	}
	return intent, nil
}

func encodeReceipt(receipt promotionReceipt) ([]byte, error) {
	if err := validateReceipt(receipt); err != nil {
		return nil, fmt.Errorf("migrationtxn: invalid receipt: %w", err)
	}
	return marshalCanonical(receipt)
}

func decodeReceipt(data []byte) (promotionReceipt, error) {
	var receipt promotionReceipt
	if err := unmarshalCanonical(data, &receipt); err != nil {
		return promotionReceipt{}, err
	}
	if err := validateReceipt(receipt); err != nil {
		return promotionReceipt{}, err
	}
	canonical, err := marshalCanonical(receipt)
	if err != nil || !bytes.Equal(canonical, data) {
		return promotionReceipt{}, fmt.Errorf("receipt is not in canonical encoding")
	}
	return receipt, nil
}

func marshalCanonical(value any) ([]byte, error) {
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(data, '\n'), nil
}

func unmarshalCanonical(data []byte, destination any) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		return fmt.Errorf("decode state: %w", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return fmt.Errorf("decode state: multiple JSON values")
		}
		return fmt.Errorf("decode state trailing bytes: %w", err)
	}
	return nil
}

func validateReceipt(receipt promotionReceipt) error {
	if receipt.Format != receiptFormat {
		return fmt.Errorf("unsupported receipt format")
	}
	if receipt.TaskIDs == nil {
		return fmt.Errorf("receipt task manifest uses non-canonical null")
	}
	switch receipt.Operation {
	case promotionOperationID:
		return validateLifecycleReceipt(receipt)
	case manualModeOperationID:
		return validateManualReceipt(receipt)
	default:
		return fmt.Errorf("unsupported receipt operation %q", receipt.Operation)
	}
}

func validateLifecycleReceipt(receipt promotionReceipt) error {
	promotion, err := migrate.PromoteToProduction(receipt.FromMode, receipt.FromLifecycle)
	if err != nil {
		return err
	}
	if promotion.AlreadyProduction {
		return fmt.Errorf("receipt source lifecycle must be non-production")
	}
	if receipt.ToMode != promotion.ToMode ||
		receipt.ToLifecycle != promotion.ToLifecycle ||
		receipt.AutoMigration != promotion.AutoMigration ||
		receipt.RoadmapMigration != promotion.AutoMigration {
		return fmt.Errorf("receipt transition disagrees with canonical promotion")
	}
	wantTasks := promotionTaskIDs(promotion)
	if !equalStrings(receipt.TaskIDs, wantTasks) {
		return fmt.Errorf("receipt task manifest disagrees with canonical promotion")
	}
	return nil
}

func validateManualReceipt(receipt promotionReceipt) error {
	if receipt.FromMode != migrate.ModeExplorer ||
		receipt.ToMode != migrate.ModeEngineering ||
		receipt.FromLifecycle != receipt.ToLifecycle ||
		!knownProjectLifecycles[receipt.FromLifecycle] ||
		receipt.AutoMigration ||
		!receipt.RoadmapMigration {
		return fmt.Errorf("receipt transition disagrees with canonical manual migration")
	}
	wantTasks := promotionTaskIDsFromTasks(migrate.ExplorerToEngineering().Tasks)
	if !equalStrings(receipt.TaskIDs, wantTasks) {
		return fmt.Errorf("receipt task manifest disagrees with canonical manual migration")
	}
	return nil
}

func validateIntent(intent promotionIntent) error {
	if intent.Format != intentFormat || intent.Operation != intent.Receipt.Operation {
		return fmt.Errorf("unsupported intent format or operation")
	}
	if err := validateReceipt(intent.Receipt); err != nil {
		return err
	}
	if err := validateFileImage(intent.ProjectBefore, projectMaxBytes, "project before"); err != nil {
		return err
	}
	if err := validateFileImage(intent.ProjectAfter, projectMaxBytes, "project after"); err != nil {
		return err
	}
	if !intent.ProjectBefore.Present || !intent.ProjectAfter.Present {
		return fmt.Errorf("project images must both be present")
	}
	if intent.ProjectBefore.Mode != intent.ProjectAfter.Mode {
		return fmt.Errorf("project permission mode changed across intent")
	}
	if err := validateIntentProject(intent); err != nil {
		return err
	}
	if intent.RoadmapManaged != intent.Receipt.RoadmapMigration {
		return fmt.Errorf("roadmap management disagrees with migration receipt")
	}
	return validateIntentRoadmap(intent)
}

func validateIntentProject(intent promotionIntent) error {
	before, err := strictProjectSelectors(intent.ProjectBefore.Data)
	if err != nil {
		return fmt.Errorf("invalid project before image: %w", err)
	}
	after, err := strictProjectSelectors(intent.ProjectAfter.Data)
	if err != nil {
		return fmt.Errorf("invalid project after image: %w", err)
	}
	receipt := intent.Receipt
	if before.mode != receipt.FromMode || before.lifecycle != receipt.FromLifecycle ||
		after.mode != receipt.ToMode || after.lifecycle != receipt.ToLifecycle {
		return fmt.Errorf("project images disagree with receipt selectors")
	}
	expectedData := rewriteProjectSelectors(
		intent.ProjectBefore.Data, before, receipt.ToMode, receipt.ToLifecycle,
	)
	expected := newFileImage(expectedData, os.FileMode(intent.ProjectBefore.Mode), true)
	if !sameFileImage(expected, intent.ProjectAfter) {
		return fmt.Errorf("project after image is not the canonical selector-only rewrite")
	}
	return nil
}

func validateIntentRoadmap(intent promotionIntent) error {
	if !intent.RoadmapManaged {
		if intent.RoadmapBefore.Present || intent.RoadmapAfter.Present {
			return fmt.Errorf("unmanaged roadmap carries file images")
		}
		return nil
	}
	if err := validateFileImage(intent.RoadmapBefore, roadmapMaxBytes, "roadmap before"); err != nil {
		return err
	}
	if err := validateFileImage(intent.RoadmapAfter, roadmapMaxBytes, "roadmap after"); err != nil {
		return err
	}
	if !intent.RoadmapAfter.Present {
		return fmt.Errorf("managed roadmap after image must be present")
	}
	if intent.RoadmapBefore.Present &&
		intent.RoadmapBefore.Mode != intent.RoadmapAfter.Mode {
		return fmt.Errorf("roadmap permission mode changed across intent")
	}
	tasks := migrate.ExplorerToEngineering().Tasks
	beforePresent, err := validatePromotionRoadmap(intent.RoadmapBefore.Data, tasks)
	if err != nil || beforePresent {
		return fmt.Errorf("roadmap before image already carries promotion markers")
	}
	afterPresent, err := validatePromotionRoadmap(intent.RoadmapAfter.Data, tasks)
	if err != nil || !afterPresent {
		return fmt.Errorf("roadmap after image lacks canonical promotion markers")
	}
	expectedData, err := appendPromotionRoadmap(intent.RoadmapBefore.Data, tasks)
	if err != nil {
		return fmt.Errorf("derive canonical roadmap after image: %w", err)
	}
	expectedMode := os.FileMode(intent.RoadmapBefore.Mode)
	if !intent.RoadmapBefore.Present {
		expectedMode = 0o644
	}
	expected := newFileImage(expectedData, expectedMode, true)
	if !sameFileImage(expected, intent.RoadmapAfter) {
		return fmt.Errorf("roadmap after image is not the canonical append-only rewrite")
	}
	return nil
}

func loadReceipt(root string, ops fileOps) (promotionReceipt, bool, error) {
	return loadReceiptForOperation(root, promotionOperationID, ops)
}

func loadReceiptForOperation(
	root, operation string,
	ops fileOps,
) (promotionReceipt, bool, error) {
	path, err := receiptPathForOperation(root, operation)
	if err != nil {
		return promotionReceipt{}, false, err
	}
	data, present, err := readOptionalStateLimit(
		root, path, receiptMaxBytes, ops,
	)
	if err != nil || !present {
		return promotionReceipt{}, present, err
	}
	receipt, err := decodeReceipt(data)
	if err != nil {
		return promotionReceipt{}, false, fmt.Errorf("migrationtxn: decode terminal receipt: %w", err)
	}
	return receipt, true, nil
}

func validateReceiptState(
	root string,
	receipt promotionReceipt,
	currentMode, currentLifecycle string,
	ops fileOps,
) error {
	if err := validateReceipt(receipt); err != nil {
		return fmt.Errorf("migrationtxn: terminal receipt invalid: %w", err)
	}
	if receipt.Operation == promotionOperationID &&
		currentLifecycle != receipt.ToLifecycle {
		return fmt.Errorf(
			"migrationtxn: terminal receipt conflicts with non-production project state",
		)
	}
	if receipt.ToMode != currentMode {
		return fmt.Errorf("migrationtxn: terminal receipt conflicts with current project selectors")
	}
	if receipt.ToLifecycle != currentLifecycle &&
		!validComposedLifecycle(root, receipt, currentLifecycle, ops) {
		return fmt.Errorf("migrationtxn: terminal receipt conflicts with current project selectors")
	}
	if !receipt.RoadmapMigration {
		return nil
	}
	roadmap, err := ops.readTracked(roadmapPath(root), roadmapMaxBytes)
	if err != nil {
		return fmt.Errorf("migrationtxn: validate promoted ROADMAP.md: %w", err)
	}
	present, err := validatePromotionRoadmap(roadmap.Data, migrate.ExplorerToEngineering().Tasks)
	if err != nil || !present {
		return fmt.Errorf("migrationtxn: terminal receipt promotion markers drifted")
	}
	return nil
}

func validComposedLifecycle(
	root string,
	receipt promotionReceipt,
	currentLifecycle string,
	ops fileOps,
) bool {
	if receipt.Operation != manualModeOperationID ||
		currentLifecycle != migrate.LifecycleProduction {
		return false
	}
	lifecycleReceipt, present, err := loadReceiptForOperation(
		root, promotionOperationID, ops,
	)
	if err != nil || !present || validateReceipt(lifecycleReceipt) != nil {
		return false
	}
	return lifecycleReceipt.FromMode == receipt.ToMode &&
		lifecycleReceipt.ToMode == receipt.ToMode &&
		lifecycleReceipt.FromLifecycle == receipt.ToLifecycle &&
		lifecycleReceipt.ToLifecycle == currentLifecycle
}

func promotionTaskIDs(promotion migrate.Promotion) []string {
	return promotionTaskIDsFromTasks(promotion.Migration.Tasks)
}

func promotionTaskIDsFromTasks(tasks []migrate.Task) []string {
	ids := make([]string, 0, len(tasks))
	for _, task := range tasks {
		ids = append(ids, task.ID)
	}
	return ids
}

func equalStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
