//go:build !linux

package projectsnapshot

import "fmt"

func openCaptureAnchorWith(_ string, _ captureObserver) (*treeRoot, error) {
	return nil, fmt.Errorf("project snapshot live capture requires Linux")
}
