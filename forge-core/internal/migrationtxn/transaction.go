package migrationtxn

import (
	"bytes"
	"errors"
	"fmt"
	"os"

	"forgeos/forge-core/internal/migrate"
	"forgeos/forge-core/internal/runlock"
)

const (
	intentFormat  = "forgeos.migration-intent.v1"
	receiptFormat = "forgeos.migration-receipt.v1"
)

type Status string

const (
	StatusPlanned   Status = "PLANNED"
	StatusApplied   Status = "APPLIED"
	StatusRecovered Status = "RECOVERED"
	StatusNoop      Status = "NOOP"
	StatusReplayed  Status = "REPLAYED"
)

type Request struct {
	ToLifecycle string
}

type Result struct {
	Status           Status
	FromMode         string
	ToMode           string
	FromLifecycle    string
	ToLifecycle      string
	AutoMigration    bool
	RoadmapMigration bool
	Tasks            []migrate.Task
}

type promotionReceipt struct {
	Format           string   `json:"format"`
	Operation        string   `json:"operation"`
	FromMode         string   `json:"from_mode"`
	ToMode           string   `json:"to_mode"`
	FromLifecycle    string   `json:"from_lifecycle"`
	ToLifecycle      string   `json:"to_lifecycle"`
	AutoMigration    bool     `json:"auto_migration"`
	RoadmapMigration bool     `json:"roadmap_migration"`
	TaskIDs          []string `json:"task_ids"`
}

type promotionIntent struct {
	Format         string           `json:"format"`
	Operation      string           `json:"operation"`
	Receipt        promotionReceipt `json:"receipt"`
	ProjectBefore  fileImage        `json:"project_before"`
	ProjectAfter   fileImage        `json:"project_after"`
	RoadmapManaged bool             `json:"roadmap_managed"`
	RoadmapBefore  fileImage        `json:"roadmap_before"`
	RoadmapAfter   fileImage        `json:"roadmap_after"`
}

type prepareOperation func(string, fileOps) (Result, *promotionIntent, error)

func Preview(root string, request Request) (Result, error) {
	if err := validateRequest(request); err != nil {
		return Result{}, err
	}
	return previewOperation(root, preparePromotion)
}

func Apply(root string, request Request) (Result, error) {
	if err := validateRequest(request); err != nil {
		return Result{}, err
	}
	return applyOperation(root, promotionOperationID, preparePromotion)
}

func applyOperation(
	root, operation string,
	prepare prepareOperation,
) (Result, error) {
	if !runlock.Supported() {
		return Result{}, fmt.Errorf("migrationtxn: apply requires a supported cross-process repository lock")
	}
	ops := realFileOps{}
	pending, err := pendingWithOps(root, ops)
	if err != nil {
		return Result{}, err
	}
	if !pending {
		result, previewErr := previewOperation(root, prepare)
		if previewErr != nil || result.Status == StatusNoop || result.Status == StatusReplayed {
			return result, previewErr
		}
	}
	lock, err := runlock.Acquire(root)
	if err != nil {
		return Result{}, fmt.Errorf("migrationtxn: acquire repository mutation lock: %w", err)
	}
	defer lock.Release()
	return applyOperationLocked(root, operation, prepare, ops)
}

// Pending is a read-only probe used by run/evolve to reject a partially
// published promotion. It never creates .forge or attempts recovery.
func Pending(root string) (bool, error) {
	return pendingWithOps(root, realFileOps{})
}

func validateRequest(request Request) error {
	if request.ToLifecycle != migrate.LifecycleProduction {
		return fmt.Errorf("migrationtxn: unsupported lifecycle target %q (supported: production)",
			request.ToLifecycle)
	}
	return nil
}

func pendingWithOps(root string, ops fileOps) (bool, error) {
	stateDirPresent, err := inspectStateDir(root)
	if err != nil || !stateDirPresent {
		return false, err
	}
	present, err := ops.inspectState(pendingPath(root))
	if err != nil {
		return false, fmt.Errorf("migrationtxn: inspect pending transaction: %w", err)
	}
	return present, nil
}

func applyLocked(root string, ops fileOps) (Result, error) {
	return applyOperationLocked(root, promotionOperationID, preparePromotion, ops)
}

func applyOperationLocked(
	root, operation string,
	prepare prepareOperation,
	ops fileOps,
) (Result, error) {
	if err := ensureMigrationStateDir(root, ops); err != nil {
		return Result{}, err
	}
	pending, err := pendingWithOps(root, ops)
	if err != nil {
		return Result{}, err
	}
	if pending {
		return recoverPendingOperation(root, operation, ops)
	}
	result, intent, err := prepare(root, ops)
	if err != nil || intent == nil {
		return result, err
	}
	if err := validateOtherReceiptsForIntent(root, *intent, ops); err != nil {
		return Result{}, err
	}
	data, err := encodeIntent(*intent)
	if err != nil {
		return Result{}, err
	}
	if err := ops.writeState(pendingPath(root), data); err != nil {
		return Result{}, fmt.Errorf("migrationtxn: publish intent: %w", err)
	}
	if err := finishIntent(root, *intent, ops); err != nil {
		return Result{}, fmt.Errorf("migrationtxn: promotion remains recoverable from pending intent: %w", err)
	}
	result.Status = StatusApplied
	return result, nil
}

func recoverPending(root string, ops fileOps) (Result, error) {
	return recoverPendingOperation(root, promotionOperationID, ops)
}

func recoverPendingOperation(root, operation string, ops fileOps) (Result, error) {
	data, present, err := readOptionalState(root, pendingPath(root), ops)
	if err != nil || !present {
		if err == nil {
			err = errors.New("pending intent disappeared while repository lock was held")
		}
		return Result{}, fmt.Errorf("migrationtxn: recover pending intent: %w", err)
	}
	intent, err := decodeIntent(data)
	if err != nil {
		return Result{}, fmt.Errorf("migrationtxn: decode pending intent: %w", err)
	}
	if intent.Operation != operation {
		return Result{}, fmt.Errorf(
			"migrationtxn: pending operation %q cannot be recovered by %q; use `%s`",
			intent.Operation, operation, recoveryCommand(intent.Operation),
		)
	}
	if err := finishIntent(root, intent, ops); err != nil {
		return Result{}, fmt.Errorf("migrationtxn: pending promotion recovery failed: %w", err)
	}
	result := resultFromReceipt(intent.Receipt, StatusRecovered)
	return result, nil
}

func recoveryCommand(operation string) string {
	switch operation {
	case promotionOperationID:
		return "forge migrate --to-lifecycle production --apply"
	case manualModeOperationID:
		return "forge migrate --to engineering --apply"
	default:
		return "forge migrate with the original target and --apply"
	}
}

func finishIntent(root string, intent promotionIntent, ops fileOps) error {
	if err := validateIntent(intent); err != nil {
		return err
	}
	if err := validateOtherReceiptsForIntent(root, intent, ops); err != nil {
		return err
	}
	if intent.RoadmapManaged {
		if err := publishImage(roadmapPath(root), intent.RoadmapBefore,
			intent.RoadmapAfter, roadmapMaxBytes, ops); err != nil {
			return fmt.Errorf("publish ROADMAP before governance selector: %w", err)
		}
	}
	if err := publishImage(projectPath(root), intent.ProjectBefore,
		intent.ProjectAfter, projectMaxBytes, ops); err != nil {
		return fmt.Errorf("publish project selector: %w", err)
	}
	if err := publishReceipt(root, intent.Receipt, ops); err != nil {
		return err
	}
	if err := ops.removeState(pendingPath(root)); err != nil {
		return fmt.Errorf("remove completed intent: %w", err)
	}
	return nil
}

func publishImage(
	path string, before, after fileImage, maxBytes int64, ops fileOps,
) error {
	current, err := ops.readTracked(path, maxBytes)
	if err != nil {
		return err
	}
	if sameFileImage(current, after) {
		return nil
	}
	if !sameFileImage(current, before) {
		return fmt.Errorf("%s matches neither transaction before nor after image", path)
	}
	if !after.Present {
		return fmt.Errorf("%s transaction after image is absent", path)
	}
	if err := ops.writeTracked(path, before, after); err != nil {
		return err
	}
	committed, err := ops.readTracked(path, maxBytes)
	if err != nil {
		return err
	}
	if !sameFileImage(committed, after) {
		return fmt.Errorf("%s did not commit the intended complete image", path)
	}
	return nil
}

func preparePromotion(root string, ops fileOps) (Result, *promotionIntent, error) {
	project, err := ops.readTracked(projectPath(root), projectMaxBytes)
	if err != nil {
		return Result{}, nil, fmt.Errorf("migrationtxn: read project.yml: %w", err)
	}
	if !project.Present {
		return Result{}, nil, fmt.Errorf("migrationtxn: project.yml is required")
	}
	selectors, err := strictProjectSelectors(project.Data)
	if err != nil {
		return Result{}, nil, fmt.Errorf("migrationtxn: project selector: %w", err)
	}
	promotion, err := migrate.PromoteToProduction(selectors.mode, selectors.lifecycle)
	if err != nil {
		return Result{}, nil, fmt.Errorf("migrationtxn: plan promotion: %w", err)
	}
	result := resultFromPromotion(promotion, StatusPlanned)
	receipt, receiptPresent, err := loadReceiptForOperation(
		root, promotionOperationID, ops,
	)
	if err != nil {
		return Result{}, nil, err
	}
	if promotion.AlreadyProduction {
		return prepareAlreadyProduction(root, result, promotion, receipt, receiptPresent, ops)
	}
	if receiptPresent {
		return Result{}, nil, fmt.Errorf("migrationtxn: terminal receipt conflicts with non-production project state")
	}
	intent, err := buildIntent(root, project, selectors, promotion, ops)
	if err != nil {
		return Result{}, nil, err
	}
	return result, &intent, nil
}

func prepareAlreadyProduction(
	root string,
	result Result,
	promotion migrate.Promotion,
	receipt promotionReceipt,
	receiptPresent bool,
	ops fileOps,
) (Result, *promotionIntent, error) {
	if !receiptPresent {
		result.Status = StatusNoop
		return result, nil, nil
	}
	if err := validateReceiptState(
		root, receipt, promotion.ToMode, promotion.ToLifecycle, ops,
	); err != nil {
		return Result{}, nil, err
	}
	result = resultFromReceipt(receipt, StatusReplayed)
	return result, nil, nil
}

func buildIntent(
	root string,
	project fileImage,
	selectors projectSelectors,
	promotion migrate.Promotion,
	ops fileOps,
) (promotionIntent, error) {
	return buildIntentFromReceipt(
		root, project, selectors, receiptFromPromotion(promotion),
		promotion.Migration.Tasks, ops,
	)
}

func buildIntentFromReceipt(
	root string,
	project fileImage,
	selectors projectSelectors,
	receipt promotionReceipt,
	tasks []migrate.Task,
	ops fileOps,
) (promotionIntent, error) {
	projectAfter := newFileImage(
		rewriteProjectSelectors(
			project.Data, selectors, receipt.ToMode, receipt.ToLifecycle,
		),
		os.FileMode(project.Mode), true,
	)
	intent := promotionIntent{
		Format: intentFormat, Operation: receipt.Operation, Receipt: receipt,
		ProjectBefore: project, ProjectAfter: projectAfter,
	}
	if !receipt.RoadmapMigration {
		return validatePreparedIntent(intent)
	}
	roadmap, err := ops.readTracked(roadmapPath(root), roadmapMaxBytes)
	if err != nil {
		return promotionIntent{}, fmt.Errorf("migrationtxn: read ROADMAP.md: %w", err)
	}
	roadmapAfterData, err := appendPromotionRoadmap(roadmap.Data, tasks)
	if err != nil {
		return promotionIntent{}, fmt.Errorf("migrationtxn: prepare ROADMAP.md: %w", err)
	}
	roadmapMode := os.FileMode(roadmap.Mode)
	if !roadmap.Present {
		roadmapMode = 0o644
	}
	intent.RoadmapManaged = true
	intent.RoadmapBefore = roadmap
	intent.RoadmapAfter = newFileImage(roadmapAfterData, roadmapMode, true)
	return validatePreparedIntent(intent)
}

func validatePreparedIntent(intent promotionIntent) (promotionIntent, error) {
	if err := validateIntent(intent); err != nil {
		return promotionIntent{}, fmt.Errorf("migrationtxn: validate planned transaction: %w", err)
	}
	return intent, nil
}

func resultFromPromotion(promotion migrate.Promotion, status Status) Result {
	return Result{
		Status:   status,
		FromMode: promotion.FromMode, ToMode: promotion.ToMode,
		FromLifecycle: promotion.FromLifecycle, ToLifecycle: promotion.ToLifecycle,
		AutoMigration:    promotion.AutoMigration,
		RoadmapMigration: promotion.AutoMigration,
		Tasks:            append([]migrate.Task(nil), promotion.Migration.Tasks...),
	}
}

func resultFromReceipt(receipt promotionReceipt, status Status) Result {
	result := Result{
		Status:   status,
		FromMode: receipt.FromMode, ToMode: receipt.ToMode,
		FromLifecycle: receipt.FromLifecycle, ToLifecycle: receipt.ToLifecycle,
		AutoMigration:    receipt.AutoMigration,
		RoadmapMigration: receipt.RoadmapMigration,
	}
	if receipt.RoadmapMigration {
		result.Tasks = migrate.ExplorerToEngineering().Tasks
	}
	return result
}

func receiptFromPromotion(promotion migrate.Promotion) promotionReceipt {
	receipt := promotionReceipt{
		Format: receiptFormat, Operation: promotionOperationID,
		FromMode: promotion.FromMode, ToMode: promotion.ToMode,
		FromLifecycle: promotion.FromLifecycle, ToLifecycle: promotion.ToLifecycle,
		AutoMigration:    promotion.AutoMigration,
		RoadmapMigration: promotion.AutoMigration,
		TaskIDs:          promotionTaskIDs(promotion),
	}
	return receipt
}

func publishReceipt(root string, expected promotionReceipt, ops fileOps) error {
	current, present, err := loadReceiptForOperation(root, expected.Operation, ops)
	if err != nil {
		return err
	}
	if present {
		if !sameReceipt(current, expected) {
			return fmt.Errorf("migrationtxn: terminal receipt conflicts with pending intent")
		}
		return nil
	}
	data, err := encodeReceipt(expected)
	if err != nil {
		return err
	}
	path, err := receiptPathForOperation(root, expected.Operation)
	if err != nil {
		return err
	}
	if err := ops.writeState(path, data); err != nil {
		return fmt.Errorf("migrationtxn: publish terminal receipt: %w", err)
	}
	return nil
}

func sameReceipt(left, right promotionReceipt) bool {
	leftBytes, leftErr := encodeReceipt(left)
	rightBytes, rightErr := encodeReceipt(right)
	return leftErr == nil && rightErr == nil && bytes.Equal(leftBytes, rightBytes)
}
