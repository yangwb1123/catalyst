//go:build aix || solaris

package authenticatedadrlifecycleauthority

func checkStatePlatform() error { return errUnsupported }
