package appserver

import (
	"context"
	"fmt"
	"net/http"
	"time"

	"forgeos/forge-core/internal/devicefabricgate"
	"forgeos/forge-core/internal/deviceplacement"
	"forgeos/forge-core/internal/executionprofile"
	"forgeos/forge-core/internal/runtimebridge"
)

// newAuthenticatedSessionRoutesWithDeviceFabricActivation is the sole
// production assembly seam for owner-scoped device inventory, registry
// placement preflight, optional client-instance observation views, and the
// explicitly accepted execution-admission surface. The ordinary constructor
// remains unchanged and therefore keeps those paths at 404. A caller must
// pass the same accepted activation request already validated by Config and
// an owner-private lifecycle image; this helper evaluates again at the route
// boundary so an accidental future caller cannot bypass the gate.
func newAuthenticatedSessionRoutesWithDeviceFabricActivation(
	client *runtimebridge.Client,
	profiles *executionprofile.Catalog,
	activation *devicefabricgate.Request,
	lifecycleRegistryFile string,
	clientInstanceSessionViewFile string,
) (http.Handler, error) {
	if activation == nil {
		if lifecycleRegistryFile != "" {
			return nil, fmt.Errorf("device inventory lifecycle registry file requires an enabled activation")
		}
		if clientInstanceSessionViewFile != "" {
			return nil, fmt.Errorf("device client-instance session view file requires an enabled activation")
		}
		return newAuthenticatedSessionRoutes(client, profiles), nil
	}
	decision := devicefabricgate.Evaluate(*activation)
	if decision.Mode == devicefabricgate.ModeOff {
		if lifecycleRegistryFile != "" {
			return nil, fmt.Errorf("device inventory lifecycle registry file requires an enabled activation")
		}
		if clientInstanceSessionViewFile != "" {
			return nil, fmt.Errorf("device client-instance session view file requires an enabled activation")
		}
		return newAuthenticatedSessionRoutes(client, profiles), nil
	}
	if !decision.Allowed {
		return nil, fmt.Errorf("device fabric activation blocked (%s): %v", decision.Mode, decision.Reasons)
	}
	if decision.Mode != devicefabricgate.ModeInventory && decision.Mode != devicefabricgate.ModeObserve && decision.Mode != devicefabricgate.ModeExecute {
		return nil, fmt.Errorf("device fabric route assembly requires inventory, observe, or execute activation (got %s)", decision.Mode)
	}
	if lifecycleRegistryFile == "" {
		return nil, fmt.Errorf("accepted device fabric activation requires a lifecycle registry file")
	}
	if err := validatePrivateFilePath(lifecycleRegistryFile); err != nil {
		return nil, fmt.Errorf("device inventory lifecycle registry file: %w", err)
	}
	if clientInstanceSessionViewFile != "" {
		if err := validatePrivateFilePath(clientInstanceSessionViewFile); err != nil {
			return nil, fmt.Errorf("device client-instance session view file: %w", err)
		}
	}

	sessions := newAuthenticatedSessionRoutes(client, profiles)
	source := newActivatedPersistedLifecycleRegistryFileSetReadSource(lifecycleRegistryFile)
	v1 := newDeviceInventoryReadCandidateRoutes(&deviceInventoryReadCandidateConfig{
		Enabled: true,
		Source:  source,
	})
	v2 := newDeviceInventoryReadCandidateV2Routes(&deviceInventoryReadCandidateV2Config{
		Enabled: true,
		Source:  source,
	})
	registryPlacement := newDevicePlacementRegistryCandidateRoutes(&devicePlacementRegistryCandidateConfig{
		Enabled: true,
		Source:  source,
		Now:     activatedDeviceFabricObservationClock,
	})
	lifecycleRegistry := newActivatedLifecycleRegistryReadRoutes(lifecycleRegistryFile)
	var clientInstanceSession http.Handler
	var clientInstanceResource http.Handler
	if clientInstanceSessionViewFile != "" {
		clientInstanceSource := newActivatedClientInstanceSessionViewFileSource(clientInstanceSessionViewFile)
		clientInstanceSession = newClientInstanceSessionViewCandidateRoutes(&clientInstanceSessionViewCandidateConfig{
			Enabled: true,
			Source:  clientInstanceSource,
		})
		inventoryReader := persistedLifecycleRegistryInventoryStateReader{
			source: newActivatedPersistedLifecycleRegistryFileSetReadSource(lifecycleRegistryFile),
		}
		clientInstanceResource = newClientInstanceResourceViewCandidateRoutes(&clientInstanceResourceViewCandidateConfig{
			Enabled: true,
			Source: activatedClientInstanceResourceViewSource{
				inventory: inventoryReader,
				clients:   clientInstanceSource,
			},
		})
	}
	// EXECUTE is deliberately an admission-only production assembly. It
	// exposes the already owner-scoped consent and pending-intent handlers so
	// clients can submit an explicit, reviewable intent. The additional
	// preflight handlers below are pure binding projections; no Runner
	// transport, target selection, lease, Run creation, or dispatch route is
	// composed here.
	var executeDeviceObservation http.Handler
	var executeRunnerReceipt http.Handler
	var executeAttemptLeasePreflight http.Handler
	var executeDispatchPlanPreview http.Handler
	var executeReconciliation http.Handler
	var executeSchedulerSelection http.Handler
	if decision.Mode == devicefabricgate.ModeExecute {
		executeDeviceObservation = newSessionDeviceObservationRoutes()
		executeRunnerReceipt = newSessionRunnerReceiptObservationRoutes()
		executeAttemptLeasePreflight = newRunAttemptLeaseDispatchPreflightRoutes()
		executeDispatchPlanPreview = newRunnerDispatchPlanPreviewRoutes()
		executeReconciliation = newExecutionReconciliationPreviewRoutes()
		executeSchedulerSelection = newSchedulerSelectionPreviewRoutes(&schedulerSelectionPreviewConfig{
			Enabled: true,
			Source:  source,
			Now:     activatedDeviceFabricObservationClock,
		})
	}
	fabricRoutes := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.EscapedPath() {
		case lifecycleRegistryCandidatePath:
			lifecycleRegistry.ServeHTTP(w, r)
		case deviceInventoryReadCandidatePath:
			v1.ServeHTTP(w, r)
		case deviceInventoryReadCandidateV2Path:
			v2.ServeHTTP(w, r)
		case devicePlacementRegistryCandidatePath:
			registryPlacement.ServeHTTP(w, r)
		case clientInstanceSessionViewCandidatePath:
			if clientInstanceSession != nil {
				clientInstanceSession.ServeHTTP(w, r)
				return
			}
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
		case clientInstanceResourceViewCandidatePath:
			if clientInstanceResource != nil {
				clientInstanceResource.ServeHTTP(w, r)
				return
			}
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
		case schedulerSelectionPreviewPath:
			if decision.Mode == devicefabricgate.ModeExecute {
				executeSchedulerSelection.ServeHTTP(w, r)
				return
			}
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
		default:
			if decision.Mode == devicefabricgate.ModeExecute {
				if _, _, ok := sessionDeviceObservationPathIDs(r.URL.EscapedPath()); ok {
					executeDeviceObservation.ServeHTTP(w, r)
					return
				}
				if _, _, ok := sessionRunnerReceiptObservationPathIDs(r.URL.EscapedPath()); ok {
					executeRunnerReceipt.ServeHTTP(w, r)
					return
				}
				if _, _, ok := runAttemptLeaseDispatchPreflightPathIDs(r.URL.EscapedPath()); ok {
					executeAttemptLeasePreflight.ServeHTTP(w, r)
					return
				}
				if _, _, ok := runnerDispatchPlanPreviewPathIDs(r.URL.EscapedPath()); ok {
					executeDispatchPlanPreview.ServeHTTP(w, r)
					return
				}
				if _, _, ok := executionReconciliationPreviewPathIDs(r.URL.EscapedPath()); ok {
					executeReconciliation.ServeHTTP(w, r)
					return
				}
			}
			sessions.ServeHTTP(w, r)
		}
	})
	var assembled http.Handler = fabricRoutes
	if decision.Mode == devicefabricgate.ModeExecute {
		var executionBackend conversationBackend
		if client != nil {
			executionBackend = client
		}
		assembled = executionSurface{
			sessions:  fabricRoutes,
			execution: newExecutionRoutes(executionBackend, profiles),
		}
	}
	return assembled, nil
}

func activatedDeviceFabricObservationClock(ctx context.Context) (int64, error) {
	if ctx == nil {
		return 0, fmt.Errorf("device fabric observation clock requires a context")
	}
	if err := ctx.Err(); err != nil {
		return 0, err
	}
	now := time.Now().UnixMilli()
	if now <= 0 || now > deviceplacement.MaxSafeIntegerMS {
		return 0, fmt.Errorf("device fabric observation time is outside the JSON-safe range")
	}
	return now, nil
}
