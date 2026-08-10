package firecracker

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"forgeos/forge-core/internal/orchestrator/sandbox"
)

// FirecrackerRunner executes agent commands inside a KVM-backed microVM.
// It is the wired implementation behind SandboxConfig.Type "firecracker":
// the rootfs template is copied, the guest /init is replaced with a script
// that mounts proc/sysfs, runs the command, records its exit code in
// /forge-exit, and powers off. The host polls the marker file back out of the
// (write-back) rootfs via debugfs; guest stdout is captured from the
// Firecracker serial log. Every failure is classified so the executor can map
// it onto ExecError kinds: missing binaries or KVM are permanent config
// faults, a slow guest is a retryable timeout, and a clean non-zero exit is a
// KindFailed run.
type FirecrackerRunner struct {
	// Binary is the firecracker executable (default "firecracker").
	Binary string
	// DebugFS is the debugfs executable (default "debugfs").
	DebugFS string
	// Kernel is the vmlinux.bin path; empty means the runner is not ready.
	Kernel string
	// RootDir is a rootfs template directory (busybox layout: bin/,
	// etc/inittab, init); empty means not ready. Each run builds a fresh
	// ext4 image from it with mke2fs -d, so no journal or stale block
	// state can leak between runs.
	RootDir string
	// Mke2fs is the mke2fs executable (default "mke2fs").
	Mke2fs string
	// MemoryMB caps the microVM RAM; 0 uses the shared safe default.
	MemoryMB int
	// MaxOutputBytes bounds host serial capture; zero uses the executor's
	// 10 MiB default. Overflow stops the VM and fails explicitly.
	MaxOutputBytes int
	// PollInterval is the marker poll period (default 250ms).
	PollInterval time.Duration
	// Logf receives runner diagnostics; nil disables them.
	Logf func(format string, args ...any)
}

// guestInitScript renders the injected /init: mount the virtual filesystems,
// run the requested command, persist its exit code to /forge-exit, then power
// off. The command is shell-quoted so argv survives the busybox /bin/sh.
func guestInitScript(argv []string, stdin string) string {
	var quoted []string
	for _, arg := range argv {
		quoted = append(quoted, "'"+strings.ReplaceAll(arg, "'", `'"'"'`)+"'")
	}
	command := strings.Join(quoted, " ")
	stdinRedirect := ""
	if stdin != "" {
		stdinRedirect = "< /forge-stdin"
	}
	return fmt.Sprintf(`#!/bin/sh
mount -t proc none /proc
mount -t sysfs none /sys
mount -t devtmpfs devtmpfs /dev
echo "FORGE-GUEST-START"
%s %s
echo $? > /forge-exit
sync
echo "FORGE-GUEST-DONE"
poweroff -f
`, command, stdinRedirect)
}

// Run executes argv inside a fresh microVM. It returns the guest stdout (from
// the serial log) and the guest exit code. A nil error with a non-zero code
// means the command ran and failed; every infrastructure failure returns an
// error (config faults and timeouts are distinguished by the caller through
// exec error kinds).
func (r *FirecrackerRunner) Run(
	ctx context.Context,
	argv []string,
	stdin string,
	timeout time.Duration,
) (string, int, error) {
	started := time.Now()
	runCtx, cancel := firecrackerRunContext(ctx, timeout)
	defer cancel()
	firecracker, debugfs, mke2fs := r.toolBinaries()
	if err := r.checkReady(firecracker, debugfs, mke2fs); err != nil {
		return "", 0, err
	}
	if err := runCtx.Err(); err != nil {
		return "", 0, firecrackerContextError(err, timeout)
	}
	dir, err := os.MkdirTemp("", "forge-vm-*")
	if err != nil {
		return "", 0, fmt.Errorf("vm workspace: %w", err)
	}
	defer func() { _ = os.RemoveAll(dir) }()
	rootfs := filepath.Join(dir, "rootfs.ext4")
	if err := r.prepareWorkspace(runCtx, dir, argv, stdin, mke2fs); err != nil {
		return "", 0, err
	}
	stop, serial, err := r.launchVM(runCtx, dir, rootfs, firecracker)
	if err != nil {
		return "", 0, err
	}
	defer stop()
	exitCode, err := r.waitForMarker(runCtx, debugfs, rootfs, serial, timeout)
	if err != nil {
		return "", 0, err
	}
	return r.finish(stop, serial, exitCode, started)
}

func (r *FirecrackerRunner) finish(stop func(), serial *serialCapture, exitCode int, started time.Time) (string, int, error) {
	stop()
	log, _ := serial.snapshot()
	if err := serial.limitError(); err != nil {
		return guestOutput(log), 0, err
	}
	if r.Logf != nil {
		r.Logf("firecracker: guest exit %d after %s", exitCode, time.Since(started).Round(time.Millisecond))
	}
	return guestOutput(log), exitCode, nil
}

// prepareWorkspace copies the template, injects the init script, and builds
// a fresh ext4 image from the copy.
func (r *FirecrackerRunner) prepareWorkspace(
	ctx context.Context,
	dir string,
	argv []string,
	stdin string,
	mke2fs string,
) error {
	rootfs := filepath.Join(dir, "rootfs.ext4")
	rootdir := filepath.Join(dir, "root")
	if err := copyTree(ctx, r.RootDir, rootdir); err != nil {
		return fmt.Errorf("vm rootdir copy: %w", err)
	}
	if stdin != "" {
		if err := writeInjectedFile(
			filepath.Join(rootdir, "forge-stdin"), []byte(stdin), 0o600,
		); err != nil {
			return fmt.Errorf("vm stdin: %w", err)
		}
	}
	initScript := filepath.Join(rootdir, "init")
	if err := writeInjectedFile(initScript, []byte(guestInitScript(argv, stdin)), 0o755); err != nil {
		return fmt.Errorf("vm init script: %w", err)
	}
	if err := buildRootfs(ctx, mke2fs, rootdir, rootfs); err != nil {
		return err
	}
	return nil
}

// launchVM starts Firecracker, waits for its API socket, and drives the boot
// sequence. It returns a stop function (kill + reap) and the bounded in-memory
// serial capture carrying guest stdout.
func (r *FirecrackerRunner) launchVM(
	ctx context.Context,
	dir string,
	rootfs string,
	firecracker string,
) (func(), *serialCapture, error) {
	sock := filepath.Join(dir, "firecracker.sock")
	logPath := filepath.Join(dir, "firecracker.log")
	// Firecracker v1.7 refuses to create the log file itself: the target must
	// already exist or logger initialization fails. Guest serial output goes
	// to the VMM's stdout, captured separately for output extraction.
	if err := os.WriteFile(logPath, nil, 0o644); err != nil {
		return nil, nil, configFault(fmt.Errorf("vm log: %w", err))
	}
	serial := newSerialCapture(r.MaxOutputBytes)
	cmd := exec.CommandContext(
		ctx,
		firecracker,
		"--api-sock", sock,
		"--log-path", logPath,
		"--level", "Info",
	)
	cmd.Stdout = serial
	cmd.Stderr = serial
	if r.Logf != nil {
		r.Logf("firecracker: launching microVM (rootfs %s)", rootfs)
	}
	if err := cmd.Start(); err != nil {
		return nil, nil, configFault(fmt.Errorf("firecracker launch: %w", err))
	}
	var stopOnce sync.Once
	stop := func() {
		stopOnce.Do(func() {
			_ = cmd.Process.Kill()
			_, _ = cmd.Process.Wait()
		})
	}
	if err := waitForSocket(ctx, sock); err != nil {
		stop()
		return nil, nil, err
	}
	if err := r.boot(ctx, sock, logPath); err != nil {
		stop()
		return nil, nil, err
	}
	return stop, serial, nil
}

// waitForSocket waits for the Firecracker control API socket to appear,
// surfacing an early VMM crash (missing KVM, bad kernel) as a config fault.
func waitForSocket(ctx context.Context, sock string) error {
	interval := 100 * time.Millisecond
	deadline := time.Now().Add(10 * time.Second)
	for {
		if _, err := os.Stat(sock); err == nil {
			return nil
		}
		if time.Now().After(deadline) {
			return configFault(errors.New("firecracker API socket did not appear"))
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-time.After(interval):
		}
	}
}

// boot drives the Firecracker control API: boot source, root drive, start.
func (r *FirecrackerRunner) boot(
	ctx context.Context,
	sock string,
	logPath string,
) error {
	client := &http.Client{
		Transport: &http.Transport{
			DialContext: func(ctx context.Context, _, _ string) (net.Conn, error) {
				return (&net.Dialer{}).DialContext(ctx, "unix", sock)
			},
		},
	}
	bootSource := fmt.Sprintf(
		`{"kernel_image_path": %q, "boot_args": "console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda rw"}`,
		r.Kernel,
	)
	if err := putAPI(ctx, client, "http://firecracker/boot-source", bootSource); err != nil {
		return configFault(fmt.Errorf("firecracker boot-source: %w", err))
	}
	drive := fmt.Sprintf(
		`{"drive_id": "rootfs", "path_on_host": %q, "is_root_device": true, "is_read_only": false}`,
		filepath.Join(filepath.Dir(sock), "rootfs.ext4"),
	)
	if err := putAPI(ctx, client, "http://firecracker/drives/rootfs", drive); err != nil {
		return configFault(fmt.Errorf("firecracker drive: %w", err))
	}
	memSize, err := sandbox.EffectiveMemoryMB(r.MemoryMB)
	if err != nil {
		return configFault(err)
	}
	if err := putAPI(ctx, client, "http://firecracker/machine-config",
		fmt.Sprintf(`{"mem_size_mib": %d, "vcpu_count": 1}`, memSize)); err != nil {
		return configFault(fmt.Errorf("firecracker machine-config: %w", err))
	}
	if err := putAPI(ctx, client, "http://firecracker/actions", `{"action_type": "InstanceStart"}`); err != nil {
		return configFault(fmt.Errorf("firecracker start: %w", err))
	}
	// The serial log appears once the VMM is serving; give it a moment to open.
	if r.Logf != nil {
		r.Logf("firecracker: microVM started; waiting for guest boot (log %s)", logPath)
	}
	return nil
}

func putAPI(ctx context.Context, client *http.Client, url, body string) error {
	request, err := http.NewRequestWithContext(ctx, http.MethodPut, url, strings.NewReader(body))
	if err != nil {
		return err
	}
	response, err := client.Do(request)
	if err != nil {
		return err
	}
	defer func() { _ = response.Body.Close() }()
	_, _ = io.Copy(io.Discard, response.Body)
	if response.StatusCode != http.StatusNoContent {
		return fmt.Errorf("api %s: status %d", url, response.StatusCode)
	}
	return nil
}

// buildRootfs renders a fresh ext4 image from the template directory with
// mke2fs -d. A clean image per run avoids journal replay and stale block
// state corrupting injected files.
func buildRootfs(ctx context.Context, mke2fs, rootdir, rootfs string) error {
	cmd := exec.CommandContext(ctx, mke2fs, "-q", "-d", rootdir, "-t", "ext4", rootfs, "64M")
	if out, err := cmd.CombinedOutput(); err != nil {
		if ctx.Err() != nil {
			return fmt.Errorf("mke2fs rootfs build interrupted: %w", ctx.Err())
		}
		return configFault(fmt.Errorf("mke2fs rootfs build: %w: %s", err, out))
	}
	return nil
}

// waitForMarker polls /forge-exit out of the (write-back) rootfs until the
// guest records it, the deadline passes, or the context is cancelled.
func (r *FirecrackerRunner) waitForMarker(
	ctx context.Context,
	debugfs string,
	rootfs string,
	serial *serialCapture,
	timeout time.Duration,
) (int, error) {
	interval := r.PollInterval
	if interval <= 0 {
		interval = 250 * time.Millisecond
	}
	for {
		select {
		case <-serial.overflow:
			return 0, serial.limitError()
		case <-ctx.Done():
			return 0, firecrackerContextError(ctx.Err(), timeout)
		case <-time.After(interval):
		}
		code, found, err := readMarkerWithRetry(ctx, debugfs, rootfs)
		if err != nil {
			if ctx.Err() != nil {
				return 0, firecrackerContextError(ctx.Err(), timeout)
			}
			return 0, configFault(fmt.Errorf("firecracker marker read: %w", err))
		}
		if found {
			return code, nil
		}
	}
}

// readMarkerWithRetry fetches the marker with a bounded retry: the guest is
// concurrently writing the live ext4 image, so transient debugfs errors must
// not escalate to a permanent config fault (review F6).
func readMarkerWithRetry(ctx context.Context, debugfs, rootfs string) (int, bool, error) {
	var lastErr error
	for attempt := 0; attempt < 3; attempt++ {
		code, found, err := readMarker(ctx, debugfs, rootfs)
		if err == nil {
			return code, found, nil
		}
		lastErr = err
		if attempt < 2 {
			select {
			case <-ctx.Done():
				return 0, false, ctx.Err()
			case <-time.After(100 * time.Millisecond):
			}
		}
	}
	return 0, false, lastErr
}

// readMarker fetches /forge-exit from the rootfs image; found=false while the
// guest has not yet written it.
func readMarker(ctx context.Context, debugfs, rootfs string) (int, bool, error) {
	cmd := exec.CommandContext(ctx, debugfs, "-R", "cat /forge-exit", rootfs)
	out, err := cmd.CombinedOutput()
	if ctx.Err() != nil {
		return 0, false, ctx.Err()
	}
	// debugfs reports a missing file on STDOUT with exit code 0, so the
	// not-found probe must run regardless of err.
	if bytes.Contains(out, []byte("File not found")) || bytes.Contains(out, []byte("couldn't find")) {
		return 0, false, nil
	}
	if err != nil {
		return 0, false, err
	}
	return parseMarkerText(string(out))
}

// parseMarkerText converts the /forge-exit marker content into an exit code.
// debugfs prefixes its output with a version banner, so the last non-empty
// line carries the marker value.
func parseMarkerText(raw string) (int, bool, error) {
	lines := strings.Fields(raw)
	if len(lines) == 0 {
		return 0, false, fmt.Errorf("marker is empty")
	}
	text := lines[len(lines)-1]
	code, err := strconv.Atoi(text)
	if err != nil {
		return 0, false, fmt.Errorf("marker %q is not an exit code", text)
	}
	return code, true, nil
}

// configFault wraps an infrastructure fault so the executor classifies it as
// a permanent configuration error.
func configFault(err error) error {
	return fmt.Errorf("sandbox firecracker: %w", err)
}
