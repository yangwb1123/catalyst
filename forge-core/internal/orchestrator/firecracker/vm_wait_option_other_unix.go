//go:build unix && !aix

package firecracker

import "syscall"

const vmWaitNoHang = syscall.WNOHANG
