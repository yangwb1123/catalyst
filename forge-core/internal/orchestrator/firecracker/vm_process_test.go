package firecracker

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

var errTestDiagnosticWriter = errors.New("diagnostic writer failed")

type failingDiagnosticWriter struct{}

func (failingDiagnosticWriter) Write(_ []byte) (int, error) {
	return 0, errTestDiagnosticWriter
}

func TestVMProcessDirectExitResults(t *testing.T) {
	for _, test := range []struct {
		mode    string
		wantErr bool
	}{{mode: "exit-zero"}, {mode: "exit-seven", wantErr: true}} {
		t.Run(test.mode, func(t *testing.T) {
			vm := launchPortableVMFixture(t, test.mode, io.Discard)
			err := vm.result()
			if (err != nil) != test.wantErr {
				t.Fatalf("result error = %v, want error %v", err, test.wantErr)
			}
		})
	}
}

func TestVMProcessDiagnosticWriterFailureIsReported(t *testing.T) {
	vm := launchPortableVMFixture(t, "write-output", failingDiagnosticWriter{})
	if err := vm.result(); !errors.Is(err, errTestDiagnosticWriter) {
		t.Fatalf("diagnostic writer error = %v", err)
	}
}

func TestVMProcessStopReportsCompletedFailures(t *testing.T) {
	t.Run("abnormal exit", func(t *testing.T) {
		vm := launchPortableVMFixture(t, "exit-seven", io.Discard)
		<-vm.exited
		if err := vm.stop(); !containsExitStatus(err) {
			t.Fatalf("stop abnormal-exit error = %v", err)
		}
	})
	t.Run("diagnostic writer", func(t *testing.T) {
		vm := launchPortableVMFixture(t, "write-output", failingDiagnosticWriter{})
		<-vm.exited
		<-vm.drained
		if err := vm.stop(); !errors.Is(err, errTestDiagnosticWriter) {
			t.Fatalf("stop diagnostic error = %v", err)
		}
	})
}

func TestWaitForVMCancellationStopsDirectProcess(t *testing.T) {
	serial := newSerialCapture(1024)
	vm := launchPortableVMFixture(t, "sleep", serial)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	err := (&FirecrackerRunner{}).waitForVM(ctx, vm, serial, 0)
	if !errors.Is(err, context.Canceled) {
		t.Fatalf("wait cancellation = %v", err)
	}
	if err := vm.stop(); err != nil {
		t.Fatalf("cancel cleanup = %v", err)
	}
}

func TestWaitForSocketReportsEarlyDirectExit(t *testing.T) {
	serial := newSerialCapture(1024)
	vm := launchPortableVMFixture(t, "exit-seven", serial)
	err := waitForSocket(context.Background(), filepath.Join(t.TempDir(), "missing.sock"), vm, serial)
	if err == nil || !containsExitStatus(err) {
		t.Fatalf("early exit error = %v", err)
	}
}

func containsExitStatus(err error) bool {
	var exitErr interface{ ExitCode() int }
	return errors.As(err, &exitErr) && exitErr.ExitCode() == 7
}

func launchPortableVMFixture(t *testing.T, mode string, diagnostics io.Writer) *vmProcess {
	t.Helper()
	command := exec.Command(os.Args[0], "-test.run=^TestVMProcessPortableHelper$")
	command.Env = append(os.Environ(), "FORGE_VM_PORTABLE_HELPER="+mode)
	vm, err := launchVMProcess(command, diagnostics)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = vm.stop() })
	return vm
}

func TestVMProcessPortableHelper(t *testing.T) {
	switch os.Getenv("FORGE_VM_PORTABLE_HELPER") {
	case "exit-zero":
		return
	case "exit-seven":
		os.Exit(7)
	case "write-output":
		_, _ = fmt.Fprint(os.Stdout, "diagnostic")
	case "sleep":
		time.Sleep(30 * time.Second)
	default:
		return
	}
}
