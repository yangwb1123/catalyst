package appserver

import (
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
)

// TestAcceptedDeviceFabricAssemblyRechecksReviewEvidence exercises the route
// assembly boundary after the pure gate. A persisted candidate image cannot
// turn a Proposed ADR or a missing review claim into an accepted owner read.
// This remains a construction test: no production enrollment/heartbeat route
// is mounted and no lifecycle state is mutated.
func TestAcceptedDeviceFabricAssemblyRechecksReviewEvidence(t *testing.T) {
	owner := deviceidentity.Owner{Issuer: "https://issuer.example", Subject: "account-42", TenantID: "tenant-slate"}
	registryPath := writeLifecycleRegistrySourceFile(t, owner, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		lifecycleRegistrySourceState(t, owner, "device-a", "runner-a", 1),
	})
	cases := []struct {
		name   string
		mutate func(*devicefabricgate.Request)
		want   string
	}{
		{
			name: "adr_0114_proposed",
			mutate: func(request *devicefabricgate.Request) {
				request.ADR0114 = devicefabricgate.Decision{Status: "proposed"}
			},
			want: "adr_0114_not_accepted",
		},
		{
			name: "adr_0114_planning_only",
			mutate: func(request *devicefabricgate.Request) {
				request.ADR0114.PlanningOnly = true
			},
			want: "adr_0114_planning_only",
		},
		{
			name: "owner_approval_evidence_missing",
			mutate: func(request *devicefabricgate.Request) {
				request.Evidence.OwnerApprovalAndRevocation = false
			},
			want: "owner_approval_or_revocation_missing",
		},
		{
			name: "heartbeat_cas_evidence_missing",
			mutate: func(request *devicefabricgate.Request) {
				request.Evidence.HeartbeatCASAndFreshness = false
			},
			want: "heartbeat_cas_or_freshness_missing",
		},
		{
			name: "inventory_scope_evidence_missing",
			mutate: func(request *devicefabricgate.Request) {
				request.Evidence.InventoryOwnerScope = false
			},
			want: "inventory_owner_scope_missing",
		},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			activation := acceptedInventoryActivation()
			test.mutate(&activation)
			_, err := newAuthenticatedSessionRoutesWithDeviceFabricActivation(
				nil, nil, ptrDeviceFabricRequest(activation), registryPath, filepath.Join(t.TempDir(), "client-view.json"),
			)
			if err == nil || !strings.Contains(err.Error(), "device fabric activation blocked") ||
				!strings.Contains(err.Error(), test.want) {
				t.Fatalf("activation error=%v, want blocked by %s", err, test.want)
			}
		})
	}
}
