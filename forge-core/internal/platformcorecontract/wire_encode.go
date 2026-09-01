package platformcorecontract

import contractwire "forgeos/forge-core/internal/platformcorecontract/internal/wireprofile"

func canonicalJSON(value any, maximum int) ([]byte, error) {
	return contractwire.CanonicalJSON(value, maximum)
}
