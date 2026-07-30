//go:build !unix

package statefs

// Directory fsync is not portable through Go's standard library on non-Unix
// hosts. Callers that require crash-durable transactions reject those hosts;
// other state writers retain their pre-existing best-effort behavior.
func syncSecuredDirectory(_ string, _ bool) error {
	return nil
}
