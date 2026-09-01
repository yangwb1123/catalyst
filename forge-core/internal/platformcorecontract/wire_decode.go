package platformcorecontract

import contractwire "forgeos/forge-core/internal/platformcorecontract/internal/wireprofile"

func parseStrictJSON(data []byte, maximum int) (any, error) {
	return contractwire.ParseStrictJSON(data, maximum)
}
