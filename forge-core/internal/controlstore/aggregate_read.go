package controlstore

import (
	"context"
	"fmt"

	core "forgeos/forge-core/internal/platformcorecontract"
)

// AggregateEvents returns one integrity-checked aggregate page by version.
func (store *Store) AggregateEvents(
	ctx context.Context,
	aggregateType, aggregateID string,
	afterVersion int64,
	limit int,
) ([]StoredEvent, error) {
	if err := validateReadContext(ctx, store); err != nil {
		return nil, err
	}
	if err := validateAggregateReference(aggregateType, aggregateID); err != nil {
		return nil, err
	}
	if err := validatePage(afterVersion, limit); err != nil {
		return nil, err
	}
	rows, err := store.db.QueryContext(ctx, `SELECT `+storedEventColumns+`
FROM `+storedEventTables+`
WHERE e.aggregate_type = ? AND e.aggregate_id = ? AND e.aggregate_version > ?
ORDER BY e.aggregate_version LIMIT ?`, aggregateType, aggregateID, afterVersion, limit)
	if err != nil {
		return nil, fmt.Errorf("read aggregate events: %w", err)
	}
	defer func() { _ = rows.Close() }()
	return scanEvents(rows)
}

func validateAggregateReference(aggregateType, aggregateID string) error {
	if !isControlAggregate(core.EntityType(aggregateType)) {
		return fmt.Errorf("aggregate_type is not owned by Go Control")
	}
	ref := core.EntityRef{
		EntityID: aggregateID, EntityType: core.EntityType(aggregateType),
	}
	if err := core.ValidateReference(ref); err != nil {
		return fmt.Errorf("aggregate reference is invalid: %w", err)
	}
	return nil
}
