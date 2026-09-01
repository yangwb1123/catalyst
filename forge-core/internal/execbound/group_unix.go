//go:build linux

package execbound

import (
	"errors"
	"os"
	"os/exec"
	"sync"
	"syscall"
	"unsafe"
)

const linuxProcessID = 1

var linuxWaitidAvailable = sync.OnceValue(probeLinuxWaitid)

var signalLinuxProcessGroup = func(pid int) error {
	return syscall.Kill(-pid, syscall.SIGKILL)
}

var beforeLinuxGroupCancel = func() {}
var afterLinuxWaitidLocked = func() {}

type processLifecycle struct {
	mu        sync.Mutex
	reaped    bool
	groupKill bool
}

// setupProcessGroup gives cancellation a best-effort process-group handle on
// Linux when waitid(WNOWAIT) is available. A direct-child kill can leave
// grandchildren that inherited stdout/stderr alive, so every writer must
// otherwise close before a drain can prove EOF. Three mechanisms bound that
// failure mode:
//
//	(1) Setpgid puts the child in a NEW process group it leads (pgid == its
//	    pid); the grandchildren it forks inherit that same group, giving us one
//	    handle to all of them.
//	(2) boundedCommand owns cancellation and serializes SIGKILL(-pgid) with
//	    reaping. waitid(WNOWAIT) observes exit without releasing the numeric PID;
//	    the lifecycle lock then keeps group signalling from running after
//	    Cmd.Wait makes that PID reusable.
//	(3) WaitDelay plus the shared raw-pipe drain controller bound escaped or
//	    raced descendants by closing the parent readers after the grace period.
//
// This is OS-level process management (no agent/vendor knowledge), so it lives
// in the generic bounded-run layer. It is a no-op for a single-process
// command: Setpgid has no effect when nothing is forked, and cancellation is
// reached only when the run context ends. A normally exiting command sends no
// signal. When waitid(WNOWAIT) is unavailable, setup deliberately falls back
// to os.Process.Kill for the direct child and keeps the drain backstop instead
// of risking a stale numeric process-group signal.
// A descendant may call setsid/setpgid and escape this handle. Process-group
// teardown is therefore best-effort interruption, not a containment boundary;
// A normal child exit sends no group signal, so a still-running descendant can
// survive even if it never escaped. WaitDelay plus Result.DrainIncomplete
// exposes an observed parent-reader drain escape, not descendant termination.
func setupProcessGroup(cmd *exec.Cmd) *processLifecycle {
	lifecycle := &processLifecycle{groupKill: linuxWaitidAvailable()}
	if lifecycle.groupKill {
		cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
	}
	cmd.WaitDelay = waitDelay
	return lifecycle
}

func (lifecycle *processLifecycle) cancel(cmd *exec.Cmd) error {
	beforeLinuxGroupCancel()
	lifecycle.mu.Lock()
	defer lifecycle.mu.Unlock()
	if lifecycle.reaped {
		return os.ErrProcessDone
	}
	if !lifecycle.groupKill {
		return cmd.Process.Kill()
	}
	groupErr := signalLinuxProcessGroup(cmd.Process.Pid)
	directErr := cmd.Process.Kill()
	if groupErr == nil {
		return nil
	}
	if errors.Is(groupErr, syscall.ESRCH) {
		return directErr
	}
	return errors.Join(groupErr, directErr)
}

func (lifecycle *processLifecycle) wait(cmd *exec.Cmd) error {
	if !lifecycle.groupKill {
		return cmd.Wait()
	}
	if err := waitLinuxProcessExit(cmd.Process.Pid); err != nil {
		lifecycle.mu.Lock()
		lifecycle.groupKill = false
		lifecycle.mu.Unlock()
		return errors.Join(err, cmd.Wait())
	}
	lifecycle.mu.Lock()
	afterLinuxWaitidLocked()
	err := cmd.Wait()
	lifecycle.reaped = true
	lifecycle.mu.Unlock()
	return err
}

func platformGroupKillSupported() bool {
	return linuxWaitidAvailable()
}

func probeLinuxWaitid() bool {
	err := linuxWaitid(os.Getpid(), syscall.WEXITED|syscall.WNOWAIT|syscall.WNOHANG)
	return errors.Is(err, syscall.ECHILD)
}

func waitLinuxProcessExit(pid int) error {
	for {
		err := linuxWaitid(pid, syscall.WEXITED|syscall.WNOWAIT)
		if !errors.Is(err, syscall.EINTR) {
			return err
		}
	}
}

func linuxWaitid(pid, options int) error {
	var info [128]byte
	_, _, errno := syscall.Syscall6(
		syscall.SYS_WAITID, linuxProcessID, uintptr(pid),
		uintptr(unsafe.Pointer(&info[0])), uintptr(options), 0, 0,
	)
	if errno != 0 {
		return errno
	}
	return nil
}
