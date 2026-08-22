//go:build !unix

package authenticatedadrapprovalauthority

func readProtectedTrustRoot(Config) ([]byte, error) { return nil, errUnsupported }
