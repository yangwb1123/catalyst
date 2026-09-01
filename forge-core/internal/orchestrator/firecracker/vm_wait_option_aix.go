//go:build aix

package firecracker

// AIX exposes wait4 but its syscall package does not export WNOHANG.
const vmWaitNoHang = 1
