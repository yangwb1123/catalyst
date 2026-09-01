// Package domain validates caller-owned Delivery planning drafts.
//
// Aggregate values remain mutable and validation is only a point-in-time
// relation check, never a durable validated token or atomic snapshot. Callers
// must keep the complete reachable slice and pointer graph race-free and stable
// for the entire call. R0-C3 intentionally has no production consumer. A later
// consumer must establish defensive ownership or validate its exact stable
// value again at the operation boundary before it can act.
package domain
