//go:build !unix

package authenticatedadrlifecycleauthority

func checkStatePlatform() error                       { return errUnsupported }
func openProtectedState(Config) (stateSession, error) { return nil, errUnsupported }
func openProtectedReplayState(Config) (stateSession, error) {
	return nil, errUnsupported
}
func readProtectedMaterials(Config) ([][]byte, error) { return nil, errUnsupported }
