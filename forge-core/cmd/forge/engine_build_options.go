package main

// engineBuildSettings carries optional per-run observers into the shared
// run/evolve engine assembly without widening every default test call.
type engineBuildSettings struct {
	runID                string
	finalDispatchObserve func(phase, model string)
}

type engineBuildOption func(*engineBuildSettings)

func resolveEngineBuildSettings(options []engineBuildOption) engineBuildSettings {
	var settings engineBuildSettings
	for _, option := range options {
		if option != nil {
			option(&settings)
		}
	}
	return settings
}

func withEngineRunID(runID string) engineBuildOption {
	return func(settings *engineBuildSettings) {
		settings.runID = runID
	}
}

func withFinalDispatchObserver(observer func(phase, model string)) engineBuildOption {
	return func(settings *engineBuildSettings) {
		settings.finalDispatchObserve = observer
	}
}

func (settings engineBuildSettings) dispatchObserver(provenance *artifactProvenance) func(string) {
	if settings.finalDispatchObserve == nil {
		return nil
	}
	return func(phase string) {
		settings.finalDispatchObserve(phase, provenance.modelFor(phase))
	}
}
