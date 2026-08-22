// Package authenticatedadrlifecycleauthority authenticates and durably records
// immutable-source ADR-0082 lifecycle transitions. It writes only protected
// repository-external lifecycle state and never rewrites an ADR source file.
package authenticatedadrlifecycleauthority
