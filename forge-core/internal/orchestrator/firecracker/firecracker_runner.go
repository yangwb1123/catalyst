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
	"time"
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
	// MemoryMB caps the microVM RAM; 0 uses 128.
	MemoryMB int
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
	firecracker := r.Binary
	if firecracker == "" {
		firecracker = "firecracker"
	}
	debugfs := r.DebugFS
	if debugfs == "" {
		debugfs = "debugfs"
	}
	if err := r.checkReady(firecracker, debugfs); err != nil {
		return "", 0, err
	}
	dir, err := os.MkdirTemp("", "forge-vm-*")
	if err != nil {
		return "", 0, fmt.Errorf("vm workspace: %w", err)
	}
	defer os.RemoveAll(dir)
	rootfs := filepath.Join(dir, "rootfs.ext4")
	if err := r.prepareWorkspace(dir, argv, stdin); err != nil {
		return "", 0, err
	}
	started := time.Now()
	runCtx := ctx
	if timeout > 0 {
		var cancel context.CancelFunc
		runCtx, cancel = context.WithTimeout(ctx, timeout)
		defer cancel()
	}
	stop, serialPath, err := r.launchVM(runCtx, dir, rootfs, firecracker)
	if err != nil {
		return "", 0, err
	}
	defer stop()
	exitCode, err := r.waitForMarker(runCtx, debugfs, rootfs, started, timeout)
	if err != nil {
		return "", 0, err
	}
	output := guestOutput(serialPath)
	if r.Logf != nil {
		r.Logf("firecracker: guest exit %d after %s", exitCode, time.Since(started).Round(time.Millisecond))
	}
	return output, exitCode, nil
}

// prepareWorkspace copies the template, injects the init script, and builds
// a fresh ext4 image from the copy.
func (r *FirecrackerRunner) prepareWorkspace(dir string, argv []string, stdin string) error {
	rootfs := filepath.Join(dir, "rootfs.ext4")
	rootdir := filepath.Join(dir, "root")
	if err := copyTree(r.RootDir, rootdir); err != nil {
		return fmt.Errorf("vm rootdir copy: %w", err)
	}
	if stdin != "" {
		if err := os.WriteFile(filepath.Join(rootdir, "forge-stdin"), []byte(stdin), 0o600); err != nil {
			return fmt.Errorf("vm stdin: %w", err)
		}
	}
	initScript := filepath.Join(rootdir, "init")
	if err := os.WriteFile(initScript, []byte(guestInitScript(argv, stdin)), 0o755); err != nil {
		return fmt.Errorf("vm init script: %w", err)
	}
	if err := buildRootfs(r.Mke2fs, rootdir, rootfs); err != nil {
		return err
	}
	return nil
}

// launchVM starts Firecracker, waits for its API socket, and drives the boot
// sequence. It returns a stop function (kill + reap) and the serial log path
// carrying guest stdout.
func (r *FirecrackerRunner) launchVM(
	ctx context.Context,
	dir string,
	rootfs string,
	firecracker string,
) (func(), string, error) {
	sock := filepath.Join(dir, "firecracker.sock")
	logPath := filepath.Join(dir, "firecracker.log")
	// Firecracker v1.7 refuses to create the log file itself: the target must
	// already exist or logger initialization fails. Guest serial output goes
	// to the VMM's stdout, captured separately for output extraction.
	if err := os.WriteFile(logPath, nil, 0o644); err != nil {
		return nil, "", configFault(fmt.Errorf("vm log: %w", err))
	}
	serialPath := filepath.Join(dir, "serial.log")
	serial, err := os.Create(serialPath)
	if err != nil {
		return nil, "", configFault(fmt.Errorf("vm serial: %w", err))
	}
	cmd := exec.Command(
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
		serial.Close()
		return nil, "", configFault(fmt.Errorf("firecracker launch: %w", err))
	}
	stop := func() {
		serial.Close()
		_ = cmd.Process.Kill()
		_, _ = cmd.Process.Wait()
	}
	if err := waitForSocket(ctx, sock); err != nil {
		stop()
		return nil, "", err
	}
	if err := r.boot(ctx, sock, logPath); err != nil {
		stop()
		return nil, "", err
	}
	return stop, serialPath, nil
}

// checkReady verifies the runner's prerequisites: kernel, rootfs template,
// both binaries, and a usable KVM device.
func (r *FirecrackerRunner) checkReady(firecracker, debugfs string) error {
	if r.Kernel == "" || r.RootDir == "" {
		return configFault(errors.New("firecracker runner: kernel and rootdir must be configured"))
	}
	mke2fs := r.Mke2fs
	if mke2fs == "" {
		mke2fs = "mke2fs"
	}
	for _, path := range []string{r.Kernel, r.RootDir, firecracker, debugfs, mke2fs} {
		if _, err := os.Stat(path); err != nil {
			return configFault(fmt.Errorf("firecracker runner: %s unavailable: %w", path, err))
		}
	}
	if _, err := os.Stat("/dev/kvm"); err != nil {
		return configFault(errors.New("firecracker runner: /dev/kvm unavailable; host execution refused"))
	}
	return nil
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
	memSize := r.MemoryMB
	if memSize <= 0 {
		memSize = 128
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
	defer response.Body.Close()
	_, _ = io.Copy(io.Discard, response.Body)
	if response.StatusCode != http.StatusNoContent {
		return fmt.Errorf("api %s: status %d", url, response.StatusCode)
	}
	return nil
}

// buildRootfs renders a fresh ext4 image from the template directory with
// mke2fs -d. A clean image per run avoids journal replay and stale block
// state corrupting injected files.
func buildRootfs(mke2fs, rootdir, rootfs string) error {
	cmd := exec.Command(mke2fs, "-q", "-d", rootdir, "-t", "ext4", rootfs, "64M")
	if out, err := cmd.CombinedOutput(); err != nil {
		return configFault(fmt.Errorf("mke2fs rootfs build: %w: %s", err, out))
	}
	return nil
}

// copyTree recursively copies a template directory, preserving symlinks and
// executable bits (busybox layouts use bin/ symlinks into the applet binary).
func copyTree(from, to string) error {
	return filepath.Walk(from, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(from, path)
		if err != nil {
			return err
		}
		target := filepath.Join(to, rel)
		if info.IsDir() {
			return os.MkdirAll(target, info.Mode().Perm())
		}
		if info.Mode()&os.ModeSymlink != 0 {
			link, err := os.Readlink(path)
			if err != nil {
				return err
			}
			return os.Symlink(link, target)
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		return os.WriteFile(target, data, info.Mode().Perm())
	})
}

// waitForMarker polls /forge-exit out of the (write-back) rootfs until the
// guest records it, the deadline passes, or the context is cancelled.
func (r *FirecrackerRunner) waitForMarker(
	ctx context.Context,
	debugfs string,
	rootfs string,
	started time.Time,
	timeout time.Duration,
) (int, error) {
	interval := r.PollInterval
	if interval <= 0 {
		interval = 250 * time.Millisecond
	}
	for {
		select {
		case <-ctx.Done():
			if timeout > 0 && time.Since(started) >= timeout {
				return 0, fmt.Errorf("firecracker guest timed out after %s", timeout)
			}
			return 0, ctx.Err()
		case <-time.After(interval):
		}
		code, found, err := readMarkerWithRetry(debugfs, rootfs)
		if err != nil {
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
func readMarkerWithRetry(debugfs, rootfs string) (int, bool, error) {
	var lastErr error
	for attempt := 0; attempt < 3; attempt++ {
		code, found, err := readMarker(debugfs, rootfs)
		if err == nil {
			return code, found, nil
		}
		lastErr = err
		time.Sleep(100 * time.Millisecond)
	}
	return 0, false, lastErr
}

// readMarker fetches /forge-exit from the rootfs image; found=false while the
// guest has not yet written it.
func readMarker(debugfs, rootfs string) (int, bool, error) {
	cmd := exec.Command(debugfs, "-R", "cat /forge-exit", rootfs)
	out, err := cmd.CombinedOutput()
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

// guestOutput extracts guest stdout from the Firecracker serial log: the
// section between the FORGE-GUEST-START and FORGE-GUEST-DONE markers. The
// log is read with a bound so a runaway guest cannot OOM the host
// (review F3).
func guestOutput(logPath string) string {
	file, err := os.Open(logPath)
	if err != nil {
		return ""
	}
	defer file.Close()
	bounded := make([]byte, 64*1024*1024+1)
	n, _ := file.Read(bounded)
	log := string(bounded[:n])
	start := strings.Index(log, "FORGE-GUEST-START")
	if start < 0 {
		return ""
	}
	end := strings.Index(log[start:], "FORGE-GUEST-DONE")
	if end < 0 {
		end = len(log) - start
	}
	section := log[start+len("FORGE-GUEST-START") : start+end]
	lines := strings.Split(section, "\n")
	var kept []string
	for _, line := range lines {
		// Strip ONLY a leading kernel timestamp ("[    0.123456] ..."),
		// never an arbitrary "] " inside guest output (Stage review F2:
		// "LEFT] RIGHT" must survive verbatim).
		kept = append(kept, stripKernelTimestamp(line))
	}
	return strings.TrimSpace(strings.Join(kept, "\n"))
}

// configFault wraps an infrastructure fault so the executor classifies it as
// a permanent configuration error.
func configFault(err error) error {
	return fmt.Errorf("sandbox firecracker: %w", err)
}
