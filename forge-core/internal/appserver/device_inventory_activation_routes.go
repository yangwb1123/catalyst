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
	leaseRegistryFiles ...string,
) (http.Handler, error) {
	return newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthority(
		client, profiles, activation, lifecycleRegistryFile, clientInstanceSessionViewFile,
		nil, leaseRegistryFiles...,
	)
}

// newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthority
// is the explicit assembly seam for the future Runner effect boundary. The
// ordinary helper above remains authority-free so existing inventory and
// admission callers cannot accidentally mount this route.
func newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthority(
	client *runtimebridge.Client,
	profiles *executionprofile.Catalog,
	activation *devicefabricgate.Request,
	lifecycleRegistryFile string,
	clientInstanceSessionViewFile string,
	runnerAuthority *devicefabricgate.RunnerAuthorityConfig,
	leaseRegistryFiles ...string,
) (http.Handler, error) {
	return newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthorityWithBackend(
		client, profiles, activation, lifecycleRegistryFile, clientInstanceSessionViewFile,
		runnerAuthority, nil, leaseRegistryFiles...,
	)
}

// newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthorityWithBackend
// is the production assembly seam that binds Runner preview boundaries to the
// durable Runtime Run projection. The compatibility constructor above keeps
// value-only focused tests explicit; Run supplies the backend here.
func newAuthenticatedSessionRoutesWithDeviceFabricActivationAndRunnerAuthorityWithBackend(
	client *runtimebridge.Client,
	profiles *executionprofile.Catalog,
	activation *devicefabricgate.Request,
	lifecycleRegistryFile string,
	clientInstanceSessionViewFile string,
	runnerAuthority *devicefabricgate.RunnerAuthorityConfig,
	runnerBackend conversationBackend,
	leaseRegistryFiles ...string,
) (http.Handler, error) {
	leaseRegistryFile := ""
	policyRegistryFile := ""
	switch len(leaseRegistryFiles) {
	case 0:
	case 1:
		leaseRegistryFile = leaseRegistryFiles[0]
	case 2:
		leaseRegistryFile, policyRegistryFile = leaseRegistryFiles[0], leaseRegistryFiles[1]
	default:
		return nil, fmt.Errorf("device execution lease/policy registry files may be supplied at most once each")
	}
	if activation == nil {
		if lifecycleRegistryFile != "" {
			return nil, fmt.Errorf("device inventory lifecycle registry file requires an enabled activation")
		}
		if clientInstanceSessionViewFile != "" {
			return nil, fmt.Errorf("device client-instance session view file requires an enabled activation")
		}
		if leaseRegistryFile != "" {
			return nil, fmt.Errorf("device execution lease registry file requires an enabled activation")
		}
		if policyRegistryFile != "" {
			return nil, fmt.Errorf("device execution policy registry file requires an enabled activation")
		}
		if runnerAuthority != nil && runnerAuthority.Enabled {
			return nil, fmt.Errorf("Runner execution authority requires an enabled activation")
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
		if leaseRegistryFile != "" {
			return nil, fmt.Errorf("device execution lease registry file requires an enabled activation")
		}
		if policyRegistryFile != "" {
			return nil, fmt.Errorf("device execution policy registry file requires an enabled activation")
		}
		if runnerAuthority != nil && runnerAuthority.Enabled {
			return nil, fmt.Errorf("Runner execution authority requires EXECUTE activation")
		}
		return newAuthenticatedSessionRoutes(client, profiles), nil
	}
	if !decision.Allowed {
		return nil, fmt.Errorf("device fabric activation blocked (%s): %v", decision.Mode, decision.Reasons)
	}
	if decision.Mode != devicefabricgate.ModeInventory && decision.Mode != devicefabricgate.ModeObserve && decision.Mode != devicefabricgate.ModeExecute {
		return nil, fmt.Errorf("device fabric route assembly requires inventory, observe, or execute activation (got %s)", decision.Mode)
	}
	if leaseRegistryFile != "" {
		if decision.Mode != devicefabricgate.ModeExecute {
			return nil, fmt.Errorf("device execution lease registry file requires EXECUTE activation")
		}
		if err := validatePrivateFilePath(leaseRegistryFile); err != nil {
			return nil, fmt.Errorf("device execution lease registry file: %w", err)
		}
	}
	if policyRegistryFile != "" {
		if decision.Mode != devicefabricgate.ModeExecute {
			return nil, fmt.Errorf("device execution policy registry file requires EXECUTE activation")
		}
		if err := validatePrivateFilePath(policyRegistryFile); err != nil {
			return nil, fmt.Errorf("device execution policy registry file: %w", err)
		}
	}
	if runnerAuthority != nil && runnerAuthority.Enabled {
		if decision.Mode != devicefabricgate.ModeExecute {
			return nil, fmt.Errorf("Runner execution authority requires EXECUTE activation")
		}
		if leaseRegistryFile == "" {
			return nil, fmt.Errorf("Runner execution authority requires an execution lease registry file")
		}
		gate := devicefabricgate.EvaluateRunnerExecution(devicefabricgate.RunnerExecutionGateRequest{
			Activation: *activation, Authority: *runnerAuthority,
		})
		if !gate.Allowed {
			return nil, fmt.Errorf("Runner execution authority blocked (%s): %v", gate.Mode, gate.Reasons)
		}
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
	// EXECUTE keeps consent and pending-intent admission reviewable. The
	// preflight handlers below remain pure projections. A separate lease file
	// may explicitly add the fenced scheduler-lease claim; even that route has
	// no Runner transport, Run creation, command authorization, or dispatch.
	var executeDeviceObservation http.Handler
	var executeRunnerReceipt http.Handler
	var executeRunnerReceiptHistory http.Handler
	var executeRunnerReconciliation http.Handler
	var executeAttemptLeasePreflight http.Handler
	var executeDispatchPlanPreview http.Handler
	var executeRunnerExecutionIntentPreview http.Handler
	var executeReconciliation http.Handler
	var executeSchedulerSelection http.Handler
	var executeSchedulerLease http.Handler
	var executeSchedulerLeaseRenewal http.Handler
	var executeSchedulerLeaseRelease http.Handler
	var executeRunnerDispatchAdmission http.Handler
	var executeRunnerTransportAdmission http.Handler
	var executeRunnerExecutionBoundary http.Handler
	var executeRunnerAttemptBoundary http.Handler
	var executeRunExecutionEvidence http.Handler
	var policySource devicePlacementPolicyReadSource
	if policyRegistryFile != "" {
		// The policy image is a read-only owner-bound join.  Mounting it on
		// preview as well as lease keeps the candidate shown to clients equal
		// to the policy that a later fenced claim would evaluate.
		policySource = newPersistedPlacementPolicyFileSource(policyRegistryFile)
	}
	if decision.Mode == devicefabricgate.ModeExecute {
		executeDeviceObservation = newSessionDeviceObservationRoutes()
		executeRunnerReceipt = newSessionRunnerReceiptObservationRoutes()
		executeRunnerReceiptHistory = newSessionRunnerReceiptHistoryRoutes()
		executeRunnerReconciliation = newSessionRunnerReconciliationProjectionRoutes()
		executeAttemptLeasePreflight = newRunAttemptLeaseDispatchPreflightRoutes()
		executeDispatchPlanPreview = newRunnerDispatchPlanPreviewRoutes()
		executeRunnerExecutionIntentPreview = newRunnerExecutionIntentPreviewRoutesWithBackend(runnerBackend)
		executeReconciliation = newExecutionReconciliationPreviewRoutes()
		executeRunExecutionEvidence = newRunExecutionEvidencePreviewRoutes()
		executeSchedulerSelection = newSchedulerSelectionPreviewRoutes(&schedulerSelectionPreviewConfig{
			Enabled:      true,
			Source:       source,
			PolicySource: policySource,
			Now:          activatedDeviceFabricObservationClock,
			Backend:      runnerBackend,
		})
		if leaseRegistryFile != "" {
			executeSchedulerLease = newSchedulerSelectionLeaseRoutes(&schedulerSelectionLeaseConfig{
				Enabled: true, Source: source, Now: activatedDeviceFabricObservationClock,
				PolicySource: policySource, RegistryPath: leaseRegistryFile, Backend: runnerBackend,
			})
			executeSchedulerLeaseRenewal = newSchedulerSelectionLeaseRenewalRoutes(&schedulerSelectionLeaseConfig{
				Enabled: true, Now: activatedDeviceFabricObservationClock, RegistryPath: leaseRegistryFile,
				Backend: runnerBackend,
			})
			executeSchedulerLeaseRelease = newSchedulerSelectionLeaseReleaseRoutes(&schedulerSelectionLeaseConfig{
				Enabled: true, Now: activatedDeviceFabricObservationClock, RegistryPath: leaseRegistryFile,
			})
			executeRunnerDispatchAdmission = newRunnerDispatchAdmissionRoutes(&runnerDispatchAdmissionConfig{
				Enabled: true, Now: activatedDeviceFabricObservationClock, RegistryPath: leaseRegistryFile,
				Backend: runnerBackend,
			})
			executeRunnerTransportAdmission = newRunnerTransportAdmissionRoutes(&runnerTransportAdmissionConfig{
				Enabled: true, Now: activatedDeviceFabricObservationClock, RegistryPath: leaseRegistryFile,
				Backend: runnerBackend,
			})
			if runnerAuthority != nil && runnerAuthority.Enabled {
				executeRunnerExecutionBoundary = newRunnerExecutionBoundaryRoutes(&runnerExecutionBoundaryConfig{
					Enabled: true, Now: activatedDeviceFabricObservationClock, RegistryPath: leaseRegistryFile,
					Activation: *activation, Authority: *runnerAuthority, Backend: runnerBackend,
				})
				executeRunnerAttemptBoundary = newRunnerAttemptBoundaryRoutes(&runnerAttemptBoundaryConfig{
					Enabled: true, Now: activatedDeviceFabricObservationClock, RegistryPath: leaseRegistryFile,
					Activation: *activation, Authority: *runnerAuthority, Backend: runnerBackend,
				})
			}
		}
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
		case schedulerSelectionLeasePath:
			if executeSchedulerLease != nil {
				executeSchedulerLease.ServeHTTP(w, r)
				return
			}
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
		case schedulerSelectionLeaseRenewalPath:
			if executeSchedulerLeaseRenewal != nil {
				executeSchedulerLeaseRenewal.ServeHTTP(w, r)
				return
			}
			writeJSON(w, r, http.StatusNotFound, notFoundBody)
		case schedulerSelectionLeaseReleasePath:
			if executeSchedulerLeaseRelease != nil {
				executeSchedulerLeaseRelease.ServeHTTP(w, r)
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
				if _, _, ok := sessionRunnerReceiptHistoryPathIDs(r.URL.EscapedPath()); ok {
					executeRunnerReceiptHistory.ServeHTTP(w, r)
					return
				}
				if _, _, ok := sessionRunnerReconciliationProjectionPathIDs(r.URL.EscapedPath()); ok {
					executeRunnerReconciliation.ServeHTTP(w, r)
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
				if _, _, ok := runnerExecutionIntentPreviewPathIDs(r.URL.EscapedPath()); ok {
					executeRunnerExecutionIntentPreview.ServeHTTP(w, r)
					return
				}
				if _, _, ok := runnerDispatchAdmissionPathIDs(r.URL.EscapedPath()); ok {
					if executeRunnerDispatchAdmission != nil {
						executeRunnerDispatchAdmission.ServeHTTP(w, r)
						return
					}
					writeJSON(w, r, http.StatusNotFound, notFoundBody)
					return
				}
				if _, _, ok := runnerTransportAdmissionPathIDs(r.URL.EscapedPath()); ok {
					if executeRunnerTransportAdmission != nil {
						executeRunnerTransportAdmission.ServeHTTP(w, r)
						return
					}
					writeJSON(w, r, http.StatusNotFound, notFoundBody)
					return
				}
				if _, _, ok := runnerExecutionBoundaryPathIDs(r.URL.EscapedPath()); ok {
					if executeRunnerExecutionBoundary != nil {
						executeRunnerExecutionBoundary.ServeHTTP(w, r)
						return
					}
					writeJSON(w, r, http.StatusNotFound, notFoundBody)
					return
				}
				if _, _, ok := runnerAttemptBoundaryPathIDs(r.URL.EscapedPath()); ok {
					if executeRunnerAttemptBoundary != nil {
						executeRunnerAttemptBoundary.ServeHTTP(w, r)
						return
					}
					writeJSON(w, r, http.StatusNotFound, notFoundBody)
					return
				}
				if _, _, ok := executionReconciliationPreviewPathIDs(r.URL.EscapedPath()); ok {
					executeReconciliation.ServeHTTP(w, r)
					return
				}
				if _, _, ok := runExecutionEvidencePreviewPathIDs(r.URL.EscapedPath()); ok {
					executeRunExecutionEvidence.ServeHTTP(w, r)
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
