package evolverepolocatorevidencecontract

// CanonicalObservationJSON validates one standalone locator observation and
// returns the exact compact canonical bytes used by the ADR-0050 adapter.
// It performs no repository reads and grants no truth or authority.
func CanonicalObservationJSON(observation Observation) ([]byte, error) {
	return canonicalObservationJSON(observation)
}
