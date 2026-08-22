package main

import (
	"fmt"
	"sync"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/productsource"
	"forgeos/forge-core/internal/runlock"
)

const (
	outputChallengeTrailer = "\nFORGE_OUTPUT_CHALLENGE: %s\nFORGE_OUTPUT_BINDING_SHA256: %s\n"
)

type outputBindingWorkflowInfo struct {
	root, runID, workflowSHA string
	wf                       asset.Workflow
}

type outputBindingExecutionInfo struct {
	opts     runOpts
	policy   mode.Policy
	isClaude bool
}

type outputBindingEngineHooks struct {
	phaseStart       func(asset.Phase) error
	agentSpawn       func(asset.Phase) error
	phaseComplete    func(asset.Phase) error
	validateVerdict  func(asset.Phase, string) error
	workflowComplete func(asset.Workflow) error
}

func (runtime *outputBindingRuntime) engineHooks() outputBindingEngineHooks {
	if runtime == nil {
		return outputBindingEngineHooks{}
	}
	return outputBindingEngineHooks{
		phaseStart: runtime.phaseStart, agentSpawn: runtime.agentSpawn,
		phaseComplete: runtime.phaseComplete, validateVerdict: runtime.validateVerdict,
		workflowComplete: runtime.workflowComplete,
	}
}

type outputBindingAttempt struct {
	phase                    asset.Phase
	attempt                  int64
	challenge                string
	workflowSHA              string
	model                    string
	promptContext            string
	finalPrompt              string
	sourceBefore             productsource.Snapshot
	policy                   outputbinding.RuntimePolicyBinding
	artifactInputs           outputbinding.ArtifactManifest
	artifactInputPaths       []string
	artifactInputBlocks      []string
	fixedReleaseInputDigests map[string]string
	preflight                outputbinding.PreflightBinding
	claimed                  bool
	buildErr                 error
}

type outputBindingRuntime struct {
	mu               sync.Mutex
	root             string
	runID            string
	wf               asset.Workflow
	opts             runOpts
	policy           mode.Policy
	isClaude         bool
	workflowSHA      string
	priorEmits       func(string) []string
	outputPaths      func(asset.Phase) ([]string, error)
	validateOutputs  func(asset.Phase, outputbinding.ArtifactManifest) error
	attempts         map[string]int64
	challenges       map[string]bool
	bindings         map[string]bool
	journalHead      string
	pending          map[string]outputBindingAttempt
	accepted         map[string]outputbinding.AgentOutputReceipt
	acceptedSemantic map[string]string
	now              func() time.Time
	randBytes        func([]byte) error
}

func newOutputBindingRuntime(workflow outputBindingWorkflowInfo, execution outputBindingExecutionInfo,
	priorEmits func(string) []string, outputPaths ...func(asset.Phase) ([]string, error)) *outputBindingRuntime {
	if workflow.wf.OutputBindingContract != asset.OutputBindingContractLocalDigestV1 {
		return nil
	}
	if workflow.workflowSHA == "" {
		workflow.workflowSHA = checkpointWorkflowDigest(workflow.wf)
	}
	return &outputBindingRuntime{
		root: workflow.root, runID: workflow.runID, wf: workflow.wf,
		opts: execution.opts, policy: execution.policy, isClaude: execution.isClaude,
		workflowSHA: workflow.workflowSHA, priorEmits: priorEmits,
		outputPaths: firstOutputPathResolver(outputPaths),
		attempts:    map[string]int64{}, challenges: map[string]bool{}, bindings: map[string]bool{},
		pending:          map[string]outputBindingAttempt{},
		accepted:         map[string]outputbinding.AgentOutputReceipt{},
		acceptedSemantic: map[string]string{},
		now:              time.Now, randBytes: fillCryptoRandom,
	}
}

func (runtime *outputBindingRuntime) setOutputValidator(
	validate func(asset.Phase, outputbinding.ArtifactManifest) error,
) {
	if runtime != nil {
		runtime.validateOutputs = validate
	}
}

func firstOutputPathResolver(values []func(asset.Phase) ([]string, error)) func(asset.Phase) ([]string, error) {
	if len(values) == 0 || values[0] == nil {
		return func(phase asset.Phase) ([]string, error) { return append([]string(nil), phase.Emits...), nil }
	}
	return values[0]
}

func validateOutputBindingHost(wf asset.Workflow, opts runOpts) error {
	return validateOutputBindingHostSupport(wf, opts, runlock.Supported())
}

func validateOutputBindingHostSupport(wf asset.Workflow, opts runOpts, lockSupported bool) error {
	if wf.OutputBindingContract == asset.OutputBindingContractLocalDigestV1 &&
		opts.executor == "command" && !lockSupported {
		return fmt.Errorf("output binding: command execution requires a real cross-process repository run lock on this platform")
	}
	return nil
}
