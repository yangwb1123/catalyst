package deviceplacement

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
)

const commandUsage = `usage:
  forge device-placement dry-run --input FILE|-

Reads one bounded caller-supplied JSON declaration set. FILE|- is read locally;
the evaluator performs no discovery, API/network access, persistence, reservation,
dispatch, or execution. Every inventory and security attribute is unverified.
`

// Command implements the nested offline-only dry-run command.
func Command(args []string, stdin io.Reader, stdout, stderr io.Writer) int {
	if len(args) == 1 && (args[0] == "--help" || args[0] == "-h") {
		_, _ = io.WriteString(stderr, commandUsage)
		return 0
	}
	if len(args) == 0 || args[0] != "dry-run" {
		return commandFailure(stderr, 2, "expected dry-run")
	}
	if len(args) == 2 && (args[1] == "--help" || args[1] == "-h") {
		_, _ = io.WriteString(stderr, commandUsage)
		return 0
	}
	input, err := parseInputFlag(args[1:])
	if err != nil {
		return commandFailure(stderr, 2, "expected --input FILE|-")
	}
	requestReader, closeInput, err := openInput(input, stdin)
	if err != nil {
		return commandFailure(stderr, 1, "cannot read bounded input")
	}
	request, decodeErr := Decode(requestReader)
	closeErr := closeInput()
	if decodeErr != nil || closeErr != nil {
		return commandFailure(stderr, 1, "invalid dry-run input")
	}
	result, err := Evaluate(request)
	if err != nil {
		return commandFailure(stderr, 1, "invalid dry-run request")
	}
	encoded, err := Marshal(result)
	if err != nil {
		return commandFailure(stderr, 1, "cannot encode dry-run result")
	}
	written, err := stdout.Write(encoded)
	if err != nil || written != len(encoded) {
		return commandFailure(stderr, 1, "cannot write dry-run result")
	}
	return 0
}

func parseInputFlag(args []string) (string, error) {
	value, supplied := "", false
	flags := flag.NewFlagSet("device-placement dry-run", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	flags.Func("input", "bounded request JSON file or - for stdin", func(input string) error {
		if supplied || input == "" {
			return errInvalidRequest
		}
		supplied, value = true, input
		return nil
	})
	if err := flags.Parse(args); err != nil || flags.NArg() != 0 || !supplied {
		return "", errInvalidRequest
	}
	return value, nil
}

func openInput(path string, stdin io.Reader) (io.Reader, func() error, error) {
	if path == "-" {
		if stdin == nil {
			return nil, func() error { return nil }, errInvalidRequest
		}
		return stdin, func() error { return nil }, nil
	}
	pathBefore, err := os.Lstat(path)
	if err != nil || !pathBefore.Mode().IsRegular() {
		return nil, func() error { return nil }, errInvalidRequest
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, func() error { return nil }, errInvalidRequest
	}
	before, err := file.Stat()
	pathInfo, pathErr := os.Lstat(path)
	if err != nil || pathErr != nil || !before.Mode().IsRegular() || !pathInfo.Mode().IsRegular() ||
		!os.SameFile(pathBefore, before) || !os.SameFile(before, pathInfo) {
		_ = file.Close()
		return nil, func() error { return nil }, errInvalidRequest
	}
	reader := &stableFileReader{file: file, path: path, before: before}
	return reader, reader.Close, nil
}

type stableFileReader struct {
	file   *os.File
	path   string
	before os.FileInfo
}

func (reader *stableFileReader) Read(buffer []byte) (int, error) {
	return reader.file.Read(buffer)
}

func (reader *stableFileReader) Close() error {
	after, statErr := reader.file.Stat()
	pathInfo, pathErr := os.Lstat(reader.path)
	closeErr := reader.file.Close()
	if statErr != nil || pathErr != nil || !after.Mode().IsRegular() || !pathInfo.Mode().IsRegular() ||
		!os.SameFile(reader.before, after) || !os.SameFile(after, pathInfo) ||
		reader.before.Size() != after.Size() || !reader.before.ModTime().Equal(after.ModTime()) {
		return errInvalidRequest
	}
	return closeErr
}

// Marshal emits deterministic compact JSON with stable field order and no LF.
func Marshal(result Result) ([]byte, error) {
	if result.SchemaVersion != ResultSchemaVersion || result.EvaluationMode != EvaluationMode ||
		!result.OwnerDeclarationUnverified || !result.DeviceAttributesUnverified ||
		result.ExecutionAuthorized || result.ReservationCreated || result.DispatchPerformed ||
		result.EvaluatedAtMS <= 0 || result.Notice != resultNotice || !validOwner(result.Owner) ||
		len(result.DeviceResults) > MaxDevices {
		return nil, errInvalidRequest
	}
	if !validOutputDevices(result.DeviceResults) {
		return nil, errInvalidRequest
	}
	var buffer bytes.Buffer
	encoder := jsonEncoder(&buffer)
	if err := encoder.Encode(result); err != nil || buffer.Len() == 0 || buffer.Len() > MaxRequestBytes {
		return nil, errInvalidRequest
	}
	encoded := buffer.Bytes()
	if encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidRequest
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func commandFailure(stderr io.Writer, code int, message string) int {
	_, _ = fmt.Fprintf(stderr, "forge device-placement dry-run: %s\n", message)
	return code
}

func jsonEncoder(writer io.Writer) *json.Encoder {
	encoder := json.NewEncoder(writer)
	encoder.SetEscapeHTML(false)
	return encoder
}

func validOutputDevices(devices []DeviceResult) bool {
	lastID := ""
	for index, device := range devices {
		if !validDeviceID(device.DeviceID) || !device.AttributesUnverified ||
			device.MatchesRequirements != (len(device.ExclusionReasons) == 0) {
			return false
		}
		if index > 0 && device.DeviceID <= lastID {
			return false
		}
		lastID = device.DeviceID
		for reasonIndex, reason := range device.ExclusionReasons {
			if !validToken(reason) || reasonIndex > 0 && device.ExclusionReasons[reasonIndex-1] >= reason {
				return false
			}
		}
	}
	return true
}
