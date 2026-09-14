package runtimebridge

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"os/exec"
)

func (c *Client) call(ctx context.Context, payload request) (json.RawMessage, error) {
	return c.callVersion(ctx, payload, protocolVersion, maxRequestBytes)
}

func (c *Client) callWrite(ctx context.Context, payload request) (json.RawMessage, error) {
	return c.callVersion(ctx, payload, writeProtocolVersion, maxWriteRequestBytes)
}

func (c *Client) callVersion(ctx context.Context, payload request, version string, maximum int) (json.RawMessage, error) {
	if ctx == nil {
		return nil, &Error{Code: "invalid_context"}
	}
	requestID, err := newRequestID()
	if err != nil {
		return nil, &Error{Code: "runtime_unavailable"}
	}
	payload.APIVersion = version
	payload.RequestID = requestID
	input, err := json.Marshal(payload)
	if err != nil || len(input)+1 > maximum {
		return nil, &Error{Code: "request_too_large"}
	}
	input = append(input, '\n')
	framed, err := c.run(ctx, input)
	if err != nil {
		return nil, err
	}
	return decodeEnvelopeVersion(framed, requestID, version)
}

func (c *Client) run(ctx context.Context, input []byte) ([]byte, error) {
	callCtx, cancel := context.WithTimeout(ctx, c.timeout)
	defer cancel()
	command := exec.CommandContext(callCtx, c.executable, "--runtime-rpc", "--database", c.database)
	command.WaitDelay = childWaitDelay
	command.Env = []string{}
	command.Stdin = bytes.NewReader(input)
	output := &limitedBuffer{limit: maxResponseBytes}
	command.Stdout = output
	command.Stderr = io.Discard
	if err := command.Run(); err != nil || output.exceeded {
		if errors.Is(callCtx.Err(), context.DeadlineExceeded) {
			return nil, &Error{Code: "runtime_timeout"}
		}
		if ctx.Err() != nil {
			return nil, &Error{Code: "request_cancelled"}
		}
		if output.exceeded {
			return nil, &Error{Code: "response_too_large"}
		}
		return nil, &Error{Code: "runtime_unavailable"}
	}
	framed, err := unframeResponse(output.Bytes())
	if err != nil {
		return nil, &Error{Code: "invalid_runtime_response"}
	}
	return framed, nil
}

func decodeEnvelope(framed []byte, requestID string) (json.RawMessage, error) {
	return decodeEnvelopeVersion(framed, requestID, protocolVersion)
}

func decodeEnvelopeVersion(framed []byte, requestID, apiVersion string) (json.RawMessage, error) {
	var response envelope
	if err := decodeStrict(framed, &response); err != nil ||
		response.APIVersion != apiVersion || response.RequestID != requestID {
		return nil, &Error{Code: "invalid_runtime_response"}
	}
	if !response.OK {
		if !validErrorEnvelope(framed, response) {
			return nil, &Error{Code: "invalid_runtime_response"}
		}
		return nil, &Error{Code: stableRuntimeErrorCode(response.Error.Code)}
	}
	if response.Error != nil || requireObjectFieldSet(framed, "api_version", "request_id", "ok", "result") != nil {
		return nil, &Error{Code: "invalid_runtime_response"}
	}
	return response.Result, nil
}

func stableRuntimeErrorCode(code string) string {
	switch code {
	case "not_found", "conflict", "query_failed", "write_rejected",
		"invalid_owned_conversation_request", "invalid_owned_conversation_import_request",
		"invalid_owned_conversation_import", "invalid_owned_prompt_request",
		"invalid_project_execution_consent_request",
		"invalid_pending_run_intent_request",
		"invalid_prompt_request", "invalid_limit", "invalid_cursor",
		"storage_unavailable", "storage_corrupt", "invalid_owned_request":
		return code
	default:
		return "runtime_query_failed"
	}
}

func validErrorEnvelope(data []byte, response envelope) bool {
	if response.Error == nil || requireObjectFieldSet(data, "api_version", "request_id", "ok", "error") != nil {
		return false
	}
	var root map[string]json.RawMessage
	if json.Unmarshal(data, &root) != nil || requireObjectFieldSet(root["error"], "code", "message") != nil {
		return false
	}
	return response.Error.Code != "" && response.Error.Message != ""
}

func newRequestID() (string, error) {
	var value [16]byte
	if _, err := rand.Read(value[:]); err != nil {
		return "", err
	}
	return "go-" + hex.EncodeToString(value[:]), nil
}
