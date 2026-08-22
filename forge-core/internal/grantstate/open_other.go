//go:build !unix

package grantstate

func openPlatform(config Config, port commitPort) (*Session, error) {
	return nil, newError(CodeUnsupported, "open", "protected grant state requires Unix", nil)
}

func openUsagePlatform(config Config, port commitPort) (*Session, error) {
	return nil, newError(CodeUnsupported, "open usage", "protected grant state requires Unix", nil)
}
