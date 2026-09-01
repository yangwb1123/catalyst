package main

import (
	"fmt"

	"forgeos/forge-core/internal/persist"
)

func validateResumeResourceProgress(cp persist.Checkpoint, phaseLimit int) error {
	switch {
	case cp.PhaseIndex < 0 || cp.PhaseIndex > phaseLimit:
		return fmt.Errorf("phase_index %d outside executable range [0,%d]",
			cp.PhaseIndex, phaseLimit)
	case cp.PhaseIndex > 0 && cp.AgentCalls == 0:
		return fmt.Errorf("phase_index %d is unreachable with zero agent_calls for an Evolve workflow",
			cp.PhaseIndex)
	case cp.LoopBacks > cp.AgentCalls:
		return fmt.Errorf("loop_backs %d exceeds recorded agent_calls %d",
			cp.LoopBacks, cp.AgentCalls)
	case cp.SpentUsdMicros < 0:
		return fmt.Errorf("spent_usd_micros %d must be non-negative", cp.SpentUsdMicros)
	default:
		return nil
	}
}
