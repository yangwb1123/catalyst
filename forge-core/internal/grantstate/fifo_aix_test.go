//go:build aix

package grantstate

func makeFIFO(path string, mode uint32) error {
	return errFIFOFixtureUnsupported
}
