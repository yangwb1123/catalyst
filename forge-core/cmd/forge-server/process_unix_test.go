//go:build linux && !android

package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"net"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"testing"
	"time"

	"forgeos/forge-core/internal/appserver"
)

func TestForgeServerProcessContentionReuseAndInterrupt(t *testing.T) {
	binary := buildServerCommand(t)
	stateDir := processStateDir(t)
	first, firstOutput, firstError := startServerCommand(t, binary, stateDir, "127.0.0.1:0")
	t.Cleanup(func() { stopUnwaited(first) })
	ready := readProcessReady(t, firstOutput, firstError.String)
	assertProcessHealth(t, ready.Listen)
	second := exec.Command(binary, "--state-dir", stateDir, "--listen", "127.0.0.1:0")
	secondOutput, err := second.CombinedOutput()
	if err == nil || !strings.Contains(string(secondOutput), "already running") {
		t.Fatalf("contending process = %v, output=%q", err, secondOutput)
	}
	interruptAndWait(t, first, firstError)
	third, thirdOutput, thirdError := startServerCommand(t, binary, stateDir, "127.0.0.1:0")
	t.Cleanup(func() { stopUnwaited(third) })
	assertProcessHealth(t, readProcessReady(t, thirdOutput, thirdError.String).Listen)
	interruptAndWait(t, third, thirdError)
}

func TestForgeServerProcessCancelsWithBlockedStartupOutput(t *testing.T) {
	binary := buildServerCommand(t)
	stateDir := processStateDir(t)
	address := reserveAddress(t)
	reader, writer := saturatedPipe(t)
	defer func() { _ = reader.Close() }()
	command := exec.Command(binary, "--state-dir", stateDir, "--listen", address)
	var stderr bytes.Buffer
	command.Stdout = writer
	command.Stderr = &stderr
	if err := command.Start(); err != nil {
		t.Fatal(err)
	}
	_ = writer.Close()
	t.Cleanup(func() { stopUnwaited(command) })
	waitForBoundPort(t, address)
	assertHTTPUnavailable(t, "http://"+address+appserver.HealthPath)
	interruptAndWait(t, command, &stderr)
}

func TestForgeServerProcessAnnouncementFailureDoesNotServe(t *testing.T) {
	binary := buildServerCommand(t)
	address := reserveAddress(t)
	readOnlyOutput, err := os.Open(os.DevNull)
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = readOnlyOutput.Close() }()
	command := exec.Command(binary, "--state-dir", processStateDir(t), "--listen", address)
	var stderr bytes.Buffer
	command.Stdout = readOnlyOutput
	command.Stderr = &stderr
	err = command.Run()
	if err == nil || !strings.Contains(stderr.String(), "announce server readiness") {
		t.Fatalf("announcement failure = %v, stderr=%q", err, stderr.String())
	}
	assertHTTPUnavailable(t, "http://"+address+appserver.HealthPath)
}

func buildServerCommand(t *testing.T) string {
	t.Helper()
	binary := filepath.Join(t.TempDir(), "forge-server")
	command := exec.Command("go", "build", "-o", binary, ".")
	command.Env = append(os.Environ(), "GOFLAGS=-buildvcs=false")
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("build forge-server: %v, output=%q", err, output)
	}
	return binary
}

func processStateDir(t *testing.T) string {
	t.Helper()
	parent := t.TempDir()
	if err := os.Chmod(parent, 0o700); err != nil {
		t.Fatal(err)
	}
	return filepath.Join(parent, "state")
}

func startServerCommand(t *testing.T, binary, stateDir, address string) (*exec.Cmd, *bufio.Reader, *bytes.Buffer) {
	t.Helper()
	command := exec.Command(binary, "--state-dir", stateDir, "--listen", address)
	stdout, err := command.StdoutPipe()
	if err != nil {
		t.Fatal(err)
	}
	stderr := &bytes.Buffer{}
	command.Stderr = stderr
	if err := command.Start(); err != nil {
		t.Fatal(err)
	}
	return command, bufio.NewReader(stdout), stderr
}

func readProcessReady(t *testing.T, reader *bufio.Reader, stderr func() string) appserver.Ready {
	t.Helper()
	line := make(chan []byte, 1)
	go func() {
		data, _ := reader.ReadBytes('\n')
		line <- data
	}()
	select {
	case data := <-line:
		var ready appserver.Ready
		if err := json.Unmarshal(data, &ready); err != nil {
			t.Fatalf("ready receipt %q: %v, stderr=%q", data, err, stderr())
		}
		return ready
	case <-time.After(5 * time.Second):
		t.Fatalf("ready timeout, stderr=%q", stderr())
		return appserver.Ready{}
	}
}

func assertProcessHealth(t *testing.T, listen string) {
	t.Helper()
	client := &http.Client{Transport: &http.Transport{Proxy: nil}, Timeout: 2 * time.Second}
	response, err := client.Get(listen + appserver.HealthPath)
	if err != nil {
		t.Fatal(err)
	}
	_ = response.Body.Close()
	if response.StatusCode != http.StatusOK {
		t.Fatalf("health status = %d", response.StatusCode)
	}
}

func interruptAndWait(t *testing.T, command *exec.Cmd, stderr *bytes.Buffer) {
	t.Helper()
	if err := command.Process.Signal(os.Interrupt); err != nil {
		t.Fatal(err)
	}
	if err := waitProcess(command); err != nil {
		t.Fatalf("process exit: %v, stderr=%q", err, stderr.String())
	}
}

func waitProcess(command *exec.Cmd) error {
	result := make(chan error, 1)
	go func() { result <- command.Wait() }()
	select {
	case err := <-result:
		return err
	case <-time.After(6 * time.Second):
		_ = command.Process.Kill()
		return <-result
	}
}

func stopUnwaited(command *exec.Cmd) {
	if command.Process != nil && command.ProcessState == nil {
		_ = command.Process.Kill()
		_ = command.Wait()
	}
}

func reserveAddress(t *testing.T) string {
	t.Helper()
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	address := listener.Addr().String()
	if err := listener.Close(); err != nil {
		t.Fatal(err)
	}
	return address
}

func saturatedPipe(t *testing.T) (*os.File, *os.File) {
	t.Helper()
	reader, writer, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}
	descriptor := int(writer.Fd())
	if err := syscall.SetNonblock(descriptor, true); err != nil {
		t.Fatal(err)
	}
	fillNonblocking(t, descriptor, make([]byte, 4096))
	fillNonblocking(t, descriptor, []byte{'x'})
	if err := syscall.SetNonblock(descriptor, false); err != nil {
		t.Fatal(err)
	}
	return reader, writer
}

func fillNonblocking(t *testing.T, descriptor int, data []byte) {
	t.Helper()
	for {
		_, err := syscall.Write(descriptor, data)
		if errors.Is(err, syscall.EAGAIN) || errors.Is(err, syscall.EWOULDBLOCK) {
			return
		}
		if err != nil {
			t.Fatal(err)
		}
	}
}

func waitForBoundPort(t *testing.T, address string) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		connection, err := net.DialTimeout("tcp", address, 50*time.Millisecond)
		if err == nil {
			_ = connection.Close()
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatal("server did not bind before timeout")
}

func assertHTTPUnavailable(t *testing.T, endpoint string) {
	t.Helper()
	client := &http.Client{Transport: &http.Transport{Proxy: nil}, Timeout: 200 * time.Millisecond}
	response, err := client.Get(endpoint)
	if response != nil {
		_ = response.Body.Close()
	}
	if err == nil {
		t.Fatal("server responded before a successful startup announcement")
	}
}
