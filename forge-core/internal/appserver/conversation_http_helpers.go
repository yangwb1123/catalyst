package appserver

import (
	"bytes"
	"encoding/json"
	"errors"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"io"
	"mime"
	"net/http"
	"net/url"
	"strconv"
	"strings"
	"unicode/utf8"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/runtimebridge"
)

func conversationOwner(r *http.Request) (model.Owner, bool) {
	principal, ok := authn.PrincipalFromContext(r.Context())
	if !ok {
		return model.Owner{}, false
	}
	owner := model.Owner{Issuer: principal.Issuer, Subject: principal.Subject, TenantID: principal.TenantID}
	if strings.TrimSpace(owner.Issuer) == "" || len(owner.Issuer) > 2048 ||
		strings.TrimSpace(owner.Subject) == "" || len(owner.Subject) > 255 ||
		strings.TrimSpace(owner.TenantID) == "" || len(owner.TenantID) > 256 ||
		strings.ContainsAny(owner.Issuer+owner.Subject+owner.TenantID, "\x00\r\n") {
		return model.Owner{}, false
	}
	return owner, true
}

func requestIdempotencyKey(r *http.Request) (string, error) {
	values := r.Header.Values("Idempotency-Key")
	if len(values) != 1 {
		return "", errConversationJSON
	}
	key := values[0]
	if key == "" || strings.TrimSpace(key) != key || len(key) > idempotencyKeyMaxBytes || strings.ContainsAny(key, "\r\n\x00") {
		return "", errConversationJSON
	}
	return key, nil
}

func readStrictConversationJSON(w http.ResponseWriter, r *http.Request, destination any) ([]byte, error) {
	contentTypes := r.Header.Values("Content-Type")
	if len(contentTypes) != 1 {
		return nil, errConversationContentType
	}
	mediaType, parameters, err := mime.ParseMediaType(contentTypes[0])
	if err != nil || !strings.EqualFold(mediaType, "application/json") {
		return nil, errConversationContentType
	}
	for name, value := range parameters {
		if name != "charset" || !strings.EqualFold(value, "utf-8") {
			return nil, errConversationContentType
		}
	}
	limited := http.MaxBytesReader(w, r.Body, conversationBodyMaxBytes)
	body, err := io.ReadAll(limited)
	if err != nil {
		var maxBytes *http.MaxBytesError
		if errors.As(err, &maxBytes) {
			return nil, errConversationBodyTooBig
		}
		return nil, errConversationJSON
	}
	if !utf8.Valid(body) || rejectConversationDuplicateKeys(body) != nil {
		return nil, errConversationJSON
	}
	decoder := json.NewDecoder(bytes.NewReader(body))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(destination); err != nil {
		return nil, errConversationJSON
	}
	if err := decoder.Decode(new(any)); !errors.Is(err, io.EOF) {
		return nil, errConversationJSON
	}
	return body, nil
}

func rejectConversationDuplicateKeys(body []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(body))
	decoder.UseNumber()
	first, err := decoder.Token()
	if err != nil {
		return err
	}
	if delimiter, ok := first.(json.Delim); !ok || delimiter != '{' {
		return errors.New("JSON request must be an object")
	}
	if err := walkConversationJSON(decoder, first, 1); err != nil {
		return err
	}
	if _, err := decoder.Token(); !errors.Is(err, io.EOF) {
		return errors.New("trailing JSON token")
	}
	return nil
}

func walkConversationJSON(decoder *json.Decoder, token json.Token, depth int) error {
	if depth > conversationJSONMaxDepth {
		return errors.New("JSON nesting limit exceeded")
	}
	delimiter, ok := token.(json.Delim)
	if !ok {
		return nil
	}
	switch delimiter {
	case '{':
		seen := make(map[string]struct{})
		for decoder.More() {
			keyToken, err := decoder.Token()
			if err != nil {
				return err
			}
			key, ok := keyToken.(string)
			if !ok {
				return errors.New("invalid object key")
			}
			if _, found := seen[key]; found {
				return errors.New("duplicate object key")
			}
			seen[key] = struct{}{}
			value, err := decoder.Token()
			if err != nil {
				return err
			}
			if err := walkConversationJSON(decoder, value, depth+1); err != nil {
				return err
			}
		}
		return consumeConversationDelimiter(decoder, '}')
	case '[':
		for decoder.More() {
			value, err := decoder.Token()
			if err != nil {
				return err
			}
			if err := walkConversationJSON(decoder, value, depth+1); err != nil {
				return err
			}
		}
		return consumeConversationDelimiter(decoder, ']')
	default:
		return errors.New("invalid JSON delimiter")
	}
}

func consumeConversationDelimiter(decoder *json.Decoder, expected json.Delim) error {
	closing, err := decoder.Token()
	if err != nil || closing != expected {
		return errors.New("invalid JSON delimiter")
	}
	return nil
}

func hasExactRequiredFields(body []byte, required ...string) bool {
	var object map[string]json.RawMessage
	if err := json.Unmarshal(body, &object); err != nil || object == nil || len(object) != len(required) {
		return false
	}
	for _, field := range required {
		value, ok := object[field]
		if !ok || len(value) == 0 || bytes.Equal(bytes.TrimSpace(value), []byte("null")) {
			return false
		}
	}
	return true
}

func validConversationScopeShape(body []byte) bool {
	var object map[string]json.RawMessage
	var request struct {
		Scope json.RawMessage `json:"scope"`
	}
	if json.Unmarshal(body, &request) != nil || len(request.Scope) == 0 || json.Unmarshal(request.Scope, &object) != nil || object == nil {
		return false
	}
	var scope model.ConversationScope
	if json.Unmarshal(request.Scope, &scope) != nil {
		return false
	}
	switch scope.Kind {
	case "global":
		return scope.ID == "" && exactJSONKeys(object, "kind")
	case "project", "group":
		return strings.TrimSpace(scope.ID) != "" && len(scope.ID) <= conversationIDMaxBytes && exactJSONKeys(object, "kind", "id")
	default:
		return false
	}
}

func exactJSONKeys(object map[string]json.RawMessage, keys ...string) bool {
	if len(object) != len(keys) {
		return false
	}
	for _, key := range keys {
		if _, ok := object[key]; !ok {
			return false
		}
	}
	return true
}

func parseConversationQuery(r *http.Request, allowed ...string) (url.Values, error) {
	if r.URL.ForceQuery {
		return nil, errConversationJSON
	}
	query, err := url.ParseQuery(r.URL.RawQuery)
	if err != nil {
		return nil, errConversationJSON
	}
	allowedSet := make(map[string]struct{}, len(allowed))
	for _, key := range allowed {
		allowedSet[key] = struct{}{}
	}
	for key, values := range query {
		if _, ok := allowedSet[key]; !ok || len(values) != 1 {
			return nil, errConversationJSON
		}
	}
	return query, nil
}

func requestHasBody(r *http.Request) bool {
	return r.ContentLength > 0 || len(r.TransferEncoding) > 0 || r.Body != nil && r.Body != http.NoBody
}

func parseConversationLimit(query url.Values) (int, error) {
	values, ok := query["limit"]
	if !ok {
		return conversationPageDefault, nil
	}
	value, err := parseUnsignedDecimal(values[0])
	if err != nil || value < 1 || value > conversationPageMax {
		return 0, errConversationJSON
	}
	return int(value), nil
}

func parseUnsignedDecimal(value string) (uint64, error) {
	if value == "" {
		return 0, errors.New("empty decimal")
	}
	for _, character := range value {
		if character < '0' || character > '9' {
			return 0, errors.New("non-decimal value")
		}
	}
	return strconv.ParseUint(value, 10, 64)
}

func writeConversationRequestError(w http.ResponseWriter, r *http.Request, err error) {
	switch {
	case errors.Is(err, errConversationContentType):
		writeConversationError(w, r, http.StatusUnsupportedMediaType, "unsupported_media_type", "content type must be application/json")
	case errors.Is(err, errConversationBodyTooBig):
		writeConversationError(w, r, http.StatusRequestEntityTooLarge, "request_too_large", "request body exceeds the allowed size")
	default:
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "request is invalid")
	}
}

func writeConversationMethodError(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Allow", "GET, POST")
	writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
}

func writeConversationBackendUnavailable(w http.ResponseWriter, r *http.Request) {
	writeConversationError(w, r, http.StatusServiceUnavailable, "conversation_service_unavailable", "conversation service is unavailable")
}

func writeConversationBackendError(w http.ResponseWriter, r *http.Request, err error) {
	var bridgeErr *runtimebridge.Error
	if !errors.As(err, &bridgeErr) || bridgeErr == nil {
		writeConversationError(w, r, http.StatusBadGateway, "conversation_service_error", "conversation service request failed")
		return
	}
	switch bridgeErr.Code {
	case "invalid_owned_conversation_request", "invalid_owned_conversation_import_request",
		"invalid_owned_conversation_import", "invalid_owned_prompt_request", "invalid_owned_request",
		"invalid_prompt_request", "invalid_limit", "invalid_cursor", "invalid_conversation_id":
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "request is invalid")
	case "not_found":
		writeConversationError(w, r, http.StatusNotFound, "not_found", "conversation was not found")
	case "conflict":
		writeConversationError(w, r, http.StatusConflict, "conflict", "conversation changed; refresh before retrying")
	case "storage_unavailable":
		writeConversationBackendUnavailable(w, r)
	case "storage_corrupt":
		writeConversationError(w, r, http.StatusServiceUnavailable, "conversation_service_unavailable", "conversation service is unavailable")
	case "runtime_timeout":
		writeConversationError(w, r, http.StatusGatewayTimeout, "conversation_service_timeout", "conversation service request timed out")
	case "runtime_unavailable", "request_cancelled", "invalid_configuration":
		writeConversationBackendUnavailable(w, r)
	default:
		writeConversationError(w, r, http.StatusBadGateway, "conversation_service_error", "conversation service request failed")
	}
}

func writeConversationJSON(w http.ResponseWriter, r *http.Request, status int, value any) {
	body, err := json.Marshal(value)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "request could not be completed")
		return
	}
	writeJSON(w, r, status, append(body, '\n'))
}

func writeConversationError(w http.ResponseWriter, r *http.Request, status int, code, message string) {
	body, err := json.Marshal(conversationErrorResponse{APIVersion: APIVersion, Code: code, Message: message})
	if err != nil {
		body = []byte(`{"api_version":"forgeos.app-server/v1","code":"internal_error","message":"request could not be completed"}`)
	}
	writeJSON(w, r, status, append(body, '\n'))
}

func conversationErrorBody(code, message string) []byte {
	body, err := json.Marshal(conversationErrorResponse{APIVersion: APIVersion, Code: code, Message: message})
	if err != nil {
		return []byte(`{"api_version":"forgeos.app-server/v1","code":"internal_error","message":"request could not be completed"}`)
	}
	return append(body, '\n')
}
