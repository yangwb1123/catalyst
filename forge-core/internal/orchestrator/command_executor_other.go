//go:build !unix

package orchestrator

import "os/exec"

// setupProcessGroup is a no-op on non-unix platforms: it leaves exec.CommandContext's
// default cancellation in place, which SIGKILLs (TerminateProcess on Windows) the
// DIRECT child only. The honest consequence is that the grandchild-pipe gap is NOT
// closed here — an agent that forks grandchildren can still leak them and wedge Run
// past the deadline, exactly the pre-fix behavior. Reliable group teardown on Windows
// needs a Job Object (CreateJobObject + AssignProcessToJobObject + KILL_ON_JOB_CLOSE),
// which has no portable stdlib analogue to the unix Setpgid/-pgid pair; that is left as
// deliberate future work rather than faked. Keeping the signature identical lets Execute
// call it unconditionally with no build-tagged branching at the call site.
func setupProcessGroup(_ *exec.Cmd) {}
