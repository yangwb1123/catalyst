package firecracker

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"time"

	"forgeos/forge-core/internal/execbound"
	"forgeos/forge-core/internal/orchestrator/sandbox"
)

const (
	guestStatusLimit       = 64
	debugfsDiagnosticLimit = 64 << 10
	debugfsTimeout         = 10 * time.Second
)

func (r *FirecrackerRunner) readGuestResult(
	ctx context.Context, debugfs, rootfs, dir, resultDir string,
) (string, int, error) {
	statusPath, err := dumpGuestFile(
		ctx, debugfs, rootfs, dir, resultDir+"/status", "guest-status",
	)
	if err != nil {
		return "", 0, resultReadError(ctx, "status dump", err)
	}
	status, err := readBoundedRegularFile(statusPath, guestStatusLimit)
	if err != nil {
		return "", 0, resultReadError(ctx, "status read", err)
	}
	exitCode, err := parseGuestStatus(status)
	if err != nil {
		return "", 0, configFault(err)
	}
	outputPath, err := dumpGuestFile(
		ctx, debugfs, rootfs, dir, resultDir+"/output", "guest-output",
	)
	if err != nil {
		return "", 0, resultReadError(ctx, "output dump", err)
	}
	output, err := readBoundedRegularFile(outputPath, r.outputLimit())
	if err != nil {
		return "", 0, resultReadError(ctx, "output read", err)
	}
	return string(output), exitCode, nil
}

func newGuestResultDir() (string, error) {
	var token [16]byte
	if _, err := rand.Read(token[:]); err != nil {
		return "", err
	}
	return "/forge-result-" + hex.EncodeToString(token[:]), nil
}

func (r *FirecrackerRunner) outputLimit() int {
	if r.MaxOutputBytes > 0 {
		return r.MaxOutputBytes
	}
	return sandbox.DefaultMaxOutputBytes
}

func resultReadError(ctx context.Context, subject string, err error) error {
	if ctx.Err() != nil {
		return ctx.Err()
	}
	if errors.Is(err, context.Canceled) || errors.Is(err, context.DeadlineExceeded) {
		return err
	}
	var limitErr *sandbox.OutputLimitError
	if errors.As(err, &limitErr) {
		return err
	}
	return configFault(fmt.Errorf("guest %s: %w", subject, err))
}

// dumpGuestFile runs only after VMM exit. Its fixed guest paths and fixed
// basename are interpreted by debugfs; the host destination is relative to a
// private working directory, so no temp-path quoting enters debugfs syntax.
func dumpGuestFile(
	ctx context.Context,
	debugfs, rootfs, dir, guestPath, hostName string,
) (string, error) {
	if !validGuestResultPath(guestPath) ||
		(hostName != "guest-status" && hostName != "guest-output") {
		return "", errors.New("invalid debugfs result path")
	}
	hostPath := filepath.Join(dir, hostName)
	if _, err := os.Lstat(hostPath); !os.IsNotExist(err) {
		return "", fmt.Errorf("dump destination is not fresh")
	}
	command := "dump " + guestPath + " " + hostName
	result := execbound.Run(ctx, []string{debugfs, "-R", command, rootfs},
		execbound.Options{Timeout: debugfsTimeout, MaxOutputBytes: debugfsDiagnosticLimit},
		execbound.CaptureCombined,
		execbound.Spec{Dir: dir, Env: []string{"LANG=C", "LC_ALL=C"}},
	)
	if result.CtxErr != nil {
		return "", result.CtxErr
	}
	if result.Err != nil {
		return "", fmt.Errorf("debugfs: %w: %q", result.Err, result.Rendered())
	}
	if result.CountOverflow || result.DrainIncomplete || result.Total > int64(result.Retained) {
		return "", errors.New("debugfs diagnostics were incomplete")
	}
	return hostPath, nil
}

func validGuestResultPath(path string) bool {
	const prefix = "/forge-result-"
	if len(path) != len(prefix)+32+len("/status") &&
		len(path) != len(prefix)+32+len("/output") {
		return false
	}
	tokenEnd := len(prefix) + 32
	if _, err := hex.DecodeString(path[len(prefix):tokenEnd]); err != nil {
		return false
	}
	suffix := path[tokenEnd:]
	return suffix == "/status" || suffix == "/output"
}

func readBoundedRegularFile(path string, limit int) ([]byte, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer func() { _ = file.Close() }()
	info, err := file.Stat()
	if err != nil {
		return nil, err
	}
	if !info.Mode().IsRegular() || info.Size() < 0 {
		return nil, fmt.Errorf("result is not a bounded regular file")
	}
	if info.Size() > int64(limit) {
		return nil, &sandbox.OutputLimitError{Limit: int64(limit), Total: info.Size()}
	}
	content, err := io.ReadAll(io.LimitReader(file, info.Size()))
	if err != nil {
		return nil, err
	}
	final, err := file.Stat()
	if err != nil || final.Size() != info.Size() || int64(len(content)) != info.Size() {
		return nil, errors.New("result file changed or was incompletely read")
	}
	return content, nil
}

func parseGuestStatus(raw []byte) (int, error) {
	knownInfrastructure := map[string]string{
		"infra:mount-proc\n":     "proc mount failed",
		"infra:mount-sysfs\n":    "sysfs mount failed",
		"infra:mount-dev\n":      "devtmpfs mount failed",
		"infra:privilege-drop\n": "privilege drop failed",
		"infra:output-open\n":    "result output open failed",
		"infra:output-mode\n":    "result output mode failed",
	}
	if detail, ok := knownInfrastructure[string(raw)]; ok {
		return 0, fmt.Errorf("guest init infrastructure failure: %s", detail)
	}
	if !bytes.HasPrefix(raw, []byte("exit:")) || len(raw) < len("exit:0\n") || raw[len(raw)-1] != '\n' {
		return 0, errors.New("guest status is not canonical")
	}
	digits := raw[len("exit:") : len(raw)-1]
	if len(digits) > 3 || (len(digits) > 1 && digits[0] == '0') || bytes.Contains(digits, []byte("\n")) {
		return 0, errors.New("guest exit status is not canonical")
	}
	for _, digit := range digits {
		if digit < '0' || digit > '9' {
			return 0, errors.New("guest exit status is not numeric")
		}
	}
	code, err := strconv.Atoi(string(digits))
	if err != nil || code > 255 {
		return 0, errors.New("guest exit status is outside 0..255")
	}
	return code, nil
}
