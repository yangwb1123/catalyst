//go:build !linux

package execbound

import "os/exec"

type processLifecycle struct{}

// setupProcessGroup uses race-safe os.Process.Kill for the direct child on
// non-Linux targets and adds the common positive WaitDelay. Grandchildren may
// survive, but inherited output descriptors cannot hold Run past the deadline
// plus the drain backstop. Run emits a degradation Log line on cancellation or
// incomplete drain (see Result.logDegradation).
//
// Reliable tree teardown on Windows needs a Job Object; other Unix targets
// would need the same non-reaping exit observation used by the Linux
// implementation before a numeric process-group signal can be proven safe.
// Those are deliberate future work rather than unsafe emulation.
func setupProcessGroup(cmd *exec.Cmd) *processLifecycle {
	cmd.WaitDelay = waitDelay
	return &processLifecycle{}
}

func (*processLifecycle) cancel(cmd *exec.Cmd) error {
	return cmd.Process.Kill()
}

func (*processLifecycle) wait(cmd *exec.Cmd) error {
	return cmd.Wait()
}

func platformGroupKillSupported() bool {
	return false
}
