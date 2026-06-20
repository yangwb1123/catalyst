//go:build unix

package orchestrator

import (
	"os/exec"
	"syscall"
	"time"
)

// processGroupGrace bounds how long Wait blocks AFTER the context is cancelled and
// the kill has been issued, before os/exec forcibly closes the command's I/O pipes
// so cmd.Run() returns. It is the LAST-RESORT backstop, not the primary mechanism:
// the SIGKILL-to-the-whole-group below should already have reaped every process
// holding the pipe. The grace covers the residual race where a just-forked
// grandchild inherited the stdout/stderr pipe but had not yet been signalled — Wait
// would otherwise block until that writer closed the pipe on its own, which for a
// wedged grandchild is never. With WaitDelay, os/exec closes the pipes after this
// window and Run returns, so a tripped Timeout can never hang the orchestrator. Kept
// short (the group is already dead in the normal case); only the pathological
// inherited-pipe race consumes it.
const processGroupGrace = 2 * time.Second

// setupProcessGroup makes a tripped Timeout reliably interruptible on unix, closing
// the gap that the bare exec.CommandContext leaves: its default cancel SIGKILLs only
// the DIRECT child, so an agent (e.g. `claude -p`) that forks grandchildren via its
// own tools (Bash -> git/test/build) leaves those grandchildren — which inherited the
// command's stdout/stderr pipe — alive after the direct child dies. cmd.Run() then
// blocks until EVERY pipe writer closes, so a surviving grandchild wedges Run past the
// deadline and the Timeout is defeated (the orphan leak is the lesser harm; the hung
// Run is the one that matters). Three stdlib mechanisms together fix it:
//
//	(1) Setpgid puts the child in a NEW process group it leads (pgid == its pid); the
//	    grandchildren it forks inherit that same group, giving us one handle to all of
//	    them.
//	(2) Cancel overrides os/exec's default (kill the direct child) to SIGKILL the whole
//	    group: signalling the NEGATIVE pid (-pgid) delivers to every member, so the
//	    grandchildren die with the child rather than outliving it as orphans.
//	(3) WaitDelay backstops the residual race where a grandchild inherited the pipe but
//	    was not yet signalled when Cancel ran: after the grace, os/exec closes the pipes
//	    and Run returns regardless, so the deadline is honored even then.
//
// This is OS-level process management (no agent/vendor knowledge), so it lives in the
// generic executor layer. It is a no-op for a single-process command: Setpgid has no
// effect when nothing is forked, and Cancel/WaitDelay are reached only on the
// context-cancellation path — a normally-exiting command never triggers them, so
// echo/true/sleep stay byte-for-byte unchanged.
func setupProcessGroup(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	cmd.Cancel = func() error {
		// Negative pid -> the whole process group (child + its grandchildren). The
		// child leads the group (Setpgid above) so its pid IS the pgid. Best-effort:
		// if the group already exited, Kill returns ESRCH, which is fine — the work we
		// wanted done (nothing left alive) is already true.
		return syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
	}
	cmd.WaitDelay = processGroupGrace
}
