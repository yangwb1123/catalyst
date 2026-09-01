package firecracker

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"forgeos/forge-core/internal/execbound"
	"forgeos/forge-core/internal/orchestrator/sandbox"
)

// FirecrackerRunner executes agent commands inside a KVM-backed microVM.
// Guest PID 1 opens a private result file, drops the workload to uid/gid 65534
// with no-new-privs and no capabilities, then records status in a root-only
// directory. The host waits for the VMM to exit before reading those files via
// debugfs. Completion and status therefore never depend on workload-forgeable
// serial sentinels or a file the workload can open.
type FirecrackerRunner struct {
	// Binary is the firecracker executable (default "firecracker").
	Binary string
	// DebugFS is the debugfs executable (default "debugfs").
	DebugFS string
	// Kernel is the vmlinux.bin path; empty means the runner is not ready.
	Kernel string
	// RootDir is a trusted rootfs template directory (busybox layout: bin/,
	// etc/inittab, init) whose /bin/setpriv supports the profile emitted below.
	// Each run builds a fresh ext4 image with mke2fs -d.
	RootDir string
	// Mke2fs is the mke2fs executable (default "mke2fs").
	Mke2fs string
	// MemoryMB caps the microVM RAM; 0 uses the shared safe default.
	MemoryMB int
	// MaxOutputBytes bounds the exact workload result read; zero uses the
	// executor's 10 MiB default. Serial diagnostics have a separate fixed cap.
	MaxOutputBytes int
	// Logf receives runner diagnostics; nil disables them.
	Logf func(format string, args ...any)
}

const guestPrivilegeProbe = `'uid_ok=0
gid_ok=0
groups_ok=0
inh_ok=0
prm_ok=0
eff_ok=0
bnd_ok=0
amb_ok=0
nnp_ok=0
while read -r key a b c d; do
  case "$key" in
    Uid:) [ "$a $b $c $d" = "65534 65534 65534 65534" ] || exit 1; uid_ok=1 ;;
    Gid:) [ "$a $b $c $d" = "65534 65534 65534 65534" ] || exit 1; gid_ok=1 ;;
    Groups:) [ -z "$a" ] || exit 1; groups_ok=1 ;;
    CapInh:) [ "$a" = "0000000000000000" ] || exit 1; inh_ok=1 ;;
    CapPrm:) [ "$a" = "0000000000000000" ] || exit 1; prm_ok=1 ;;
    CapEff:) [ "$a" = "0000000000000000" ] || exit 1; eff_ok=1 ;;
    CapBnd:) [ "$a" = "0000000000000000" ] || exit 1; bnd_ok=1 ;;
    CapAmb:) [ "$a" = "0000000000000000" ] || exit 1; amb_ok=1 ;;
    NoNewPrivs:) [ "$a" = "1" ] || exit 1; nnp_ok=1 ;;
  esac
done < /proc/self/status
[ "$uid_ok$gid_ok$groups_ok$inh_ok$prm_ok$eff_ok$bnd_ok$amb_ok$nnp_ok" = "111111111" ]'`

// guestInitScript renders the trusted PID-1 boundary. Redirections are opened
// by root before setpriv executes, but the workload cannot reopen the 0700
// result directory or create the status file written after it exits.
func guestInitScript(argv []string, stdin, resultDir string) string {
	var quoted []string
	for _, arg := range argv {
		quoted = append(quoted, "'"+strings.ReplaceAll(arg, "'", `'"'"'`)+"'")
	}
	command := strings.Join(quoted, " ")
	stdinRedirect := "< /dev/null"
	if stdin != "" {
		stdinRedirect = "< /forge-stdin"
	}
	launcher := "/bin/setpriv --reuid=65534 --regid=65534 --clear-groups " +
		"--inh-caps=-all --ambient-caps=-all --bounding-set=-all " +
		"--no-new-privs -- /bin/env -i PATH=/bin:/sbin:/usr/bin:/usr/sbin"
	return fmt.Sprintf(`#!/bin/sh
umask 077
FORGE_RESULT=%s
rm -rf "$FORGE_RESULT"
mkdir -m 700 "$FORGE_RESULT" || { poweroff -f; exit 1; }
write_status() {
  printf '%%s\n' "$1" > "$FORGE_RESULT/status.tmp" || { poweroff -f; exit 1; }
  chmod 600 "$FORGE_RESULT/status.tmp" || { poweroff -f; exit 1; }
  mv "$FORGE_RESULT/status.tmp" "$FORGE_RESULT/status" || { poweroff -f; exit 1; }
  sync
  poweroff -f
  exit 0
}
mount -t proc none /proc || write_status "infra:mount-proc"
mount -t sysfs none /sys || write_status "infra:mount-sysfs"
mount -t devtmpfs devtmpfs /dev || write_status "infra:mount-dev"
PRIVILEGE_PROBE=%s
%s /bin/sh -c "$PRIVILEGE_PROBE" || write_status "infra:privilege-drop"
: > "$FORGE_RESULT/output" || write_status "infra:output-open"
chmod 600 "$FORGE_RESULT/output" || write_status "infra:output-mode"
%s %s %s > "$FORGE_RESULT/output" 2>&1
code=$?
write_status "exit:$code"
`, resultDir, guestPrivilegeProbe, launcher, launcher, command, stdinRedirect)
}

// Run executes argv inside a fresh microVM. It returns the exact private
// result-file bytes and the trusted PID-1 exit record. A nil error with a
// non-zero code means the command ran and failed; infrastructure failures
// return an error for the caller to classify.
func (r *FirecrackerRunner) Run(
	ctx context.Context,
	argv []string,
	stdin string,
	timeout time.Duration,
) (output string, exitCode int, runErr error) {
	if err := r.validateRunRequest(argv, stdin); err != nil {
		return "", 0, err
	}
	started := time.Now()
	runCtx, cancel := firecrackerRunContext(ctx, timeout)
	defer cancel()
	if err := runCtx.Err(); err != nil {
		return "", 0, firecrackerContextError(err, timeout)
	}
	firecracker, debugfs, mke2fs := r.toolBinaries()
	if err := r.checkReady(firecracker, debugfs, mke2fs); err != nil {
		return "", 0, err
	}
	resultDir, err := newGuestResultDir()
	if err != nil {
		return "", 0, configFault(fmt.Errorf("guest result capability: %w", err))
	}
	dir, err := os.MkdirTemp("", "forge-vm-*")
	if err != nil {
		return "", 0, fmt.Errorf("vm workspace: %w", err)
	}
	defer func() { _ = os.RemoveAll(dir) }()
	rootfs := filepath.Join(dir, "rootfs.ext4")
	if err := r.prepareWorkspace(runCtx, dir, argv, stdin, resultDir, mke2fs); err != nil {
		return "", 0, err
	}
	vm, serial, err := r.launchVM(runCtx, dir, rootfs, firecracker)
	if err != nil {
		return "", 0, err
	}
	defer func() { runErr = errors.Join(runErr, vm.stop()) }()
	if err := r.waitForVM(runCtx, vm, serial, timeout); err != nil {
		return "", 0, err
	}
	output, exitCode, err = r.readGuestResult(runCtx, debugfs, rootfs, dir, resultDir)
	if err != nil {
		return "", 0, err
	}
	if r.Logf != nil {
		r.Logf("firecracker: guest exit %d after %s", exitCode, time.Since(started).Round(time.Millisecond))
	}
	return output, exitCode, nil
}

func (r *FirecrackerRunner) validateRunRequest(argv []string, stdin string) error {
	if err := execbound.ValidateStdinSize(len(stdin)); err != nil {
		return err
	}
	if len(argv) == 0 {
		return configFault(errors.New("empty guest argv"))
	}
	if err := (execbound.Options{MaxOutputBytes: r.MaxOutputBytes}).Validate(); err != nil {
		return configFault(err)
	}
	return nil
}

// prepareWorkspace copies the template, injects the init script, and builds
// a fresh ext4 image from the copy.
func (r *FirecrackerRunner) prepareWorkspace(
	ctx context.Context,
	dir string,
	argv []string,
	stdin string,
	resultDir string,
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
	if err := writeInjectedFile(
		initScript, []byte(guestInitScript(argv, stdin, resultDir)), 0o700,
	); err != nil {
		return fmt.Errorf("vm init script: %w", err)
	}
	if err := buildRootfs(ctx, mke2fs, rootdir, rootfs); err != nil {
		return err
	}
	return nil
}

// launchVM starts Firecracker, waits for its API socket, and drives the boot
// sequence. Workload output does not use this serial stream; it is bounded
// diagnostics only.
func (r *FirecrackerRunner) launchVM(
	ctx context.Context,
	dir string,
	rootfs string,
	firecracker string,
) (*vmProcess, *serialCapture, error) {
	sock := filepath.Join(dir, "firecracker.sock")
	logPath := filepath.Join(dir, "firecracker.log")
	// Firecracker v1.7 refuses to create the log file itself: the target must
	// already exist or logger initialization fails. VMM/guest-console output
	// goes to the bounded diagnostic capture, never to the result parser.
	if err := os.WriteFile(logPath, nil, 0o644); err != nil {
		return nil, nil, configFault(fmt.Errorf("vm log: %w", err))
	}
	serial := newSerialCapture(serialDiagnosticLimit)
	cmd := exec.Command(
		firecracker,
		"--api-sock", sock,
		"--log-path", logPath,
		"--level", "Info",
	)
	cmd.Env = []string{"LANG=C", "LC_ALL=C"}
	if r.Logf != nil {
		r.Logf("firecracker: launching microVM (rootfs %s)", rootfs)
	}
	vm, err := launchVMProcess(cmd, serial)
	if err != nil {
		return nil, nil, configFault(fmt.Errorf("firecracker launch: %w", err))
	}
	if err := waitForSocket(ctx, sock, vm, serial); err != nil {
		return nil, nil, errors.Join(err, vm.stop())
	}
	if err := r.boot(ctx, sock, logPath); err != nil {
		return nil, nil, errors.Join(err, vm.stop())
	}
	return vm, serial, nil
}

// waitForSocket waits for the Firecracker control API socket to appear,
// surfacing an early VMM crash (missing KVM, bad kernel) as a config fault.
func waitForSocket(
	ctx context.Context, sock string, vm *vmProcess, serial *serialCapture,
) error {
	ticker := time.NewTicker(100 * time.Millisecond)
	defer ticker.Stop()
	deadline := time.NewTimer(10 * time.Second)
	defer deadline.Stop()
	for {
		if _, err := os.Stat(sock); err == nil {
			return nil
		}
		select {
		case <-ctx.Done():
			return ctx.Err()
		case <-vm.exited:
			return configFault(vm.earlyExitError())
		case <-serial.overflow:
			return serial.limitError()
		case <-deadline.C:
			return configFault(errors.New("firecracker API socket did not appear"))
		case <-ticker.C:
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
	result := execbound.Run(ctx,
		[]string{mke2fs, "-q", "-d", rootdir, "-t", "ext4", rootfs, "64M"},
		execbound.Options{Unbounded: true, MaxOutputBytes: 64 << 10},
		execbound.CaptureCombined,
		execbound.Spec{Env: []string{"LANG=C", "LC_ALL=C"}},
	)
	if result.Err != nil {
		if ctx.Err() != nil {
			return fmt.Errorf("mke2fs rootfs build interrupted: %w", ctx.Err())
		}
		return configFault(fmt.Errorf("mke2fs rootfs build: %w: %q", result.Err, result.Rendered()))
	}
	if result.CountOverflow || result.DrainIncomplete || result.Total > int64(result.Retained) {
		return configFault(errors.New("mke2fs rootfs build diagnostics were incomplete"))
	}
	return nil
}

// configFault wraps an infrastructure fault so the executor classifies it as
// a permanent configuration error.
func configFault(err error) error {
	return fmt.Errorf("sandbox firecracker: %w", err)
}
