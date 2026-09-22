package appserver

import (
	"bytes"
	"encoding/json"
	"net/http"
	"net/url"
	"reflect"
	"sort"
	"strings"

	"forgeos/forge-core/internal/authn"
	"forgeos/forge-core/internal/deviceplacement"
)

const sessionDeviceObservationPreviewScope = "forge:conversations:read"

func newSessionDeviceObservationRoutes() http.Handler {
	return authn.RequireScopes(
		http.HandlerFunc(serveSessionDeviceObservationPreview),
		sessionDeviceObservationPreviewScope,
	)
}

func serveSessionDeviceObservationPreview(w http.ResponseWriter, r *http.Request) {
	if r.URL.ForceQuery || r.URL.RawQuery != "" {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_query", "device observation preview does not accept query parameters")
		return
	}
	if r.Method != http.MethodPost {
		w.Header().Set("Allow", http.MethodPost)
		writeConversationError(w, r, http.StatusMethodNotAllowed, "method_not_allowed", "method not allowed")
		return
	}
	body, err := readStrictConversationJSON(w, r, new(any))
	if err != nil {
		writeConversationRequestError(w, r, err)
		return
	}
	request, err := deviceplacement.DecodeSessionPlacementObservationRequest(bytes.NewReader(body))
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "device observation request is invalid")
		return
	}
	conversationID, runID, ok := sessionDeviceObservationPathIDs(r.URL.EscapedPath())
	if !ok || request.ConversationID != conversationID || request.RunID != runID {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "request Conversation and Run must match the path")
		return
	}
	owner, ok := conversationOwner(r)
	if !ok {
		writeConversationError(w, r, http.StatusUnauthorized, "invalid_token", "authentication is required")
		return
	}
	if request.Owner.Issuer != owner.Issuer || request.Owner.Subject != owner.Subject || request.Owner.TenantID != owner.TenantID {
		writeConversationError(w, r, http.StatusForbidden, "owner_mismatch", "device observation owner must match the authenticated principal")
		return
	}
	declaredOwner := deviceplacement.Owner{Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID}
	if !sameSessionPlacementDevices(request) {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_binding", "placement devices must match the candidate declarations")
		return
	}
	placement, err := deviceplacement.ObserveSessionPlacement(request)
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "device observation request is invalid")
		return
	}
	inventory := append([]deviceplacement.SessionPlacementCandidate(nil), request.Candidates...)
	sort.Slice(inventory, func(left, right int) bool {
		return inventory[left].Device.DeviceID < inventory[right].Device.DeviceID
	})
	summary, err := deviceplacement.ObserveDeviceResourceSummary(deviceplacement.DeviceResourceSummaryRequest{
		Owner:     declaredOwner,
		Inventory: inventory,
		Placement: placement,
	})
	if err != nil {
		writeConversationError(w, r, http.StatusBadRequest, "invalid_request", "device observation declarations are inconsistent")
		return
	}
	envelope := deviceplacement.SessionDeviceObservation{
		SchemaVersion:              deviceplacement.SessionDeviceObservationSchemaVersion,
		EvaluationMode:             deviceplacement.EvaluationMode,
		Owner:                      declaredOwner,
		ConversationID:             conversationID,
		RunID:                      runID,
		EvaluatedAtMS:              placement.EvaluatedAtMS,
		OwnerDeclarationUnverified: true,
		Inventory: deviceplacement.SessionDeviceObservationInventory{
			SchemaVersion:              "forge.device-inventory-observation/v1",
			EvaluationMode:             deviceplacement.EvaluationMode,
			EvaluatedAtMS:              placement.EvaluatedAtMS,
			Owner:                      declaredOwner,
			OwnerDeclarationUnverified: true,
			InventoryUnverified:        true,
			Notice:                     deviceplacement.SessionDeviceObservationInventoryNotice,
			Devices:                    inventory,
		},
		PlacementObservation: placement,
		ResourceSummary:      summary,
		Authority:            deviceplacement.SessionPlacementAuthority{},
	}
	if err := deviceplacement.ValidateSessionDeviceObservation(envelope); err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "device observation could not be validated")
		return
	}
	encoded, err := json.Marshal(envelope)
	if err != nil {
		writeConversationError(w, r, http.StatusInternalServerError, "internal_error", "device observation could not be encoded")
		return
	}
	writeJSON(w, r, http.StatusOK, append(encoded, '\n'))
}

func sessionDeviceObservationPathIDs(path string) (string, string, bool) {
	conversationID, suffix, ok := conversationPathSuffix(path)
	if !ok {
		return "", "", false
	}
	parts := strings.Split(suffix, "/")
	if len(parts) != 4 || parts[0] != "runs" || parts[1] == "" || parts[2] != "device-observation" || parts[3] != "preview" {
		return "", "", false
	}
	runID, err := url.PathUnescape(parts[1])
	if err != nil || strings.ContainsAny(runID, "/\x00") || strings.TrimSpace(runID) == "" || len(runID) > conversationIDMaxBytes {
		return "", "", false
	}
	return conversationID, runID, true
}

func sameSessionPlacementDevices(request deviceplacement.SessionPlacementObservationRequest) bool {
	if len(request.Placement.Devices) != len(request.Candidates) {
		return false
	}
	byID := make(map[string]deviceplacement.Device, len(request.Candidates))
	for _, candidate := range request.Candidates {
		if _, exists := byID[candidate.Device.DeviceID]; exists {
			return false
		}
		byID[candidate.Device.DeviceID] = candidate.Device
	}
	for _, device := range request.Placement.Devices {
		candidate, exists := byID[device.DeviceID]
		if !exists || !reflect.DeepEqual(candidate, device) {
			return false
		}
	}
	return true
}
