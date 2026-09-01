//go:build unix && !linux

package appserver

func trustedUnmappedOwner(int) bool { return false }
