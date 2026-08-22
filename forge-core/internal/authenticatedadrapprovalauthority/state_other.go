//go:build !unix

package authenticatedadrapprovalauthority

func checkStatePlatform() error { return errUnsupported }

func openProtectedState(Config) (stateSession, error) { return nil, errUnsupported }
