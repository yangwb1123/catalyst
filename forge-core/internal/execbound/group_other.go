//go:build !unix

package execbound

import "os/exec"

const groupKillAvailable = false

// setupProcessGroup is a no-op on non-unix platforms: it leaves
// exec.CommandContext's default cancellation in place, which SIGKILLs
// (TerminateProcess on Windows) the DIRECT child only. The honest consequence
// is that the grandchild-pipe gap is NOT closed here — a command that forks
// grandchildren can still leak them. WaitDelay (capture.go, common code) still
// backstops the HANG: Run returns ≤ deadline + 2s even when a surviving
// grandchild holds the pipes, and Run emits one honest degradation Log line on
// the kill path (see Result.logDegradation). Reliable group teardown on
// Windows needs a Job Object (CreateJobObject + AssignProcessToJobObject +
// KILL_ON_JOB_CLOSE), which has no portable stdlib analogue to the unix
// Setpgid/-pgid pair; that is left as deliberate future work rather than
// faked. Keeping the signature identical lets Run call it unconditionally
// with no build-tagged branching at the call site.
func setupProcessGroup(_ *exec.Cmd) {}
