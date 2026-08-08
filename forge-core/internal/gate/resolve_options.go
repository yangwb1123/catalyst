// resolve_options.go — the gate-config resolver: the pure lattice mapping the
// raw CLI/env surface (--timeout / --max-output-bytes flags on gate|check|
// accept; FORGE_GATE_TIMEOUT env everywhere) onto execbound.Options. Strict,
// deterministic, and fail-loud: garbage or negative values are hard errors
// that name the offending source and value — a silent default would either
// false-fail a legitimate slow suite or silently reintroduce the very hang
// this direction fixes. Every success path satisfies Options.Validate().
package gate

import (
	"fmt"
	"time"
)

// CLIInput is the raw, pre-resolution config surface of one invocation.
type CLIInput struct {
	TimeoutSet  bool          // --timeout EXPLICITLY passed (flagSet() precedent)
	Timeout     time.Duration // parsed flag value
	EnvTimeout  string        // raw $FORGE_GATE_TIMEOUT; "" = unset/empty → not consulted
	MaxBytesSet bool          // --max-output-bytes explicitly passed
	MaxBytes    int           // parsed flag value
}

// ResolveOptions implements the config lattice — flag > env > default:
//
//	--timeout > 0        → that deadline, knob "--timeout"
//	--timeout == 0       → Unbounded (the documented escape), knob "--timeout"
//	--timeout < 0        → ERROR (naming the value)
//	--timeout unset      → $FORGE_GATE_TIMEOUT consulted ("" = unset → default)
//	  env valid > 0      → that deadline, knob "FORGE_GATE_TIMEOUT"
//	  env "0"            → Unbounded (the documented escape), knob named
//	  env garbage / < 0  → ERROR (naming the variable AND value)
//	--max-output-bytes   → explicit cap; 0 → default; < 0 → ERROR
//	neither timeout source → 10m default, knob "" (nothing was configured)
//
// When the flag IS set, the env is NOT consulted at all (a garbage env is
// ignored in that case — the flag is load-bearing). Pure: no I/O, no wall
// clock, no process spawn. Options.Knob is set from whichever source won, so
// the honest timeout text names the load-bearing knob.
func ResolveOptions(in CLIInput) (Options, error) {
	opts := Options{}
	switch {
	case in.TimeoutSet && in.Timeout < 0:
		return opts, fmt.Errorf("invalid --timeout=%s: must be >= 0 (negative would silently disable the deadline)", in.Timeout)
	case in.TimeoutSet && in.Timeout == 0:
		opts.Unbounded = true
		opts.Knob = "--timeout"
	case in.TimeoutSet:
		opts.Timeout = in.Timeout
		opts.Knob = "--timeout"
	case in.EnvTimeout != "":
		// Strict parsing: whitespace/garbage/newlines all fail ParseDuration;
		// only "" (unset) skips the env entirely.
		d, err := time.ParseDuration(in.EnvTimeout)
		if err != nil {
			return opts, fmt.Errorf("invalid %s=%q: %v", EnvTimeout, in.EnvTimeout, err)
		}
		if d < 0 {
			return opts, fmt.Errorf("invalid %s=%q: must be >= 0 (negative would silently disable the deadline)", EnvTimeout, in.EnvTimeout)
		}
		if d == 0 {
			opts.Unbounded = true
		} else {
			opts.Timeout = d
		}
		opts.Knob = EnvTimeout
	}
	if in.MaxBytesSet {
		if in.MaxBytes < 0 {
			return opts, fmt.Errorf("invalid --max-output-bytes=%d: must be >= 0", in.MaxBytes)
		}
		opts.MaxOutputBytes = in.MaxBytes // 0 → execbound's safe default
	}
	if err := opts.Validate(); err != nil {
		return opts, err
	}
	return opts, nil
}
