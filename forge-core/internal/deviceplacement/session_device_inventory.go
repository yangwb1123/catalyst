package deviceplacement

// ValidateSessionDeviceObservationInventory validates the standalone
// inventory value used by the future owner-scoped read candidate. The value
// remains an unverified declaration: validation checks shape, bounds, owner
// equality, and deterministic ordering only. It does not authenticate a
// device, read a registry, or grant execution authority.
func ValidateSessionDeviceObservationInventory(value SessionDeviceObservationInventory) error {
	if value.EvaluatedAtMS <= 0 || value.EvaluatedAtMS > MaxSafeIntegerMS {
		return errInvalidRequest
	}
	return validateSessionDeviceObservationInventory(value, value.Owner, value.EvaluatedAtMS)
}
