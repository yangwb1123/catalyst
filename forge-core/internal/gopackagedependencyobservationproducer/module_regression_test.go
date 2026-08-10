package gopackagedependencyobservationproducer

import (
	"context"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/gitworktreesource"
)

func TestProduceRejectsPseudoModuleDirectives(t *testing.T) {
	for name, goMod := range map[string]string{
		"block comment":   "/*\nmodule example.com/false\n*/\n",
		"directive block": "require (\nmodule example.com/false\n)\n",
	} {
		t.Run(name, func(t *testing.T) {
			root, environment := producerFixture(t)
			writeProducerFile(t, root, "go.mod", goMod)
			production, err := produceWith(
				context.Background(), root, ".", "run-false-module", environment,
				func() time.Time { return time.UnixMilli(1_700_000_000_123) },
				gitworktreesource.Capture, gitworktreesource.ReadRegularFiles,
			)
			if err == nil || production != nil || !strings.Contains(err.Error(), "selected go.mod") {
				t.Fatalf("production = %v, error = %v", production, err)
			}
		})
	}
}
