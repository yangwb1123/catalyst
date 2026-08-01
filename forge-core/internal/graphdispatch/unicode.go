package graphdispatch

func validUnicodeEscapes(data []byte) bool {
	for position := 0; position < len(data); position++ {
		if data[position] != '"' {
			continue
		}
		end, valid := scanJSONString(data, position)
		if !valid {
			return false
		}
		position = end
	}
	return true
}

func scanJSONString(data []byte, start int) (int, bool) {
	for position := start + 1; position < len(data); position++ {
		switch data[position] {
		case '"':
			return position, true
		case '\\':
			position++
			if position >= len(data) {
				return 0, false
			}
			if data[position] == 'u' {
				end, valid := scanUnicodeEscape(data, position)
				if !valid {
					return 0, false
				}
				position = end
			}
		}
	}
	return 0, false
}

func scanUnicodeEscape(data []byte, marker int) (int, bool) {
	value, valid := fourHexDigits(data, marker+1)
	if !valid || (value >= 0xdc00 && value <= 0xdfff) {
		return 0, false
	}
	end := marker + 4
	if value < 0xd800 || value > 0xdbff {
		return end, true
	}
	if end+6 >= len(data) || data[end+1] != '\\' || data[end+2] != 'u' {
		return 0, false
	}
	low, valid := fourHexDigits(data, end+3)
	if !valid || low < 0xdc00 || low > 0xdfff {
		return 0, false
	}
	return end + 6, true
}

func fourHexDigits(data []byte, start int) (uint16, bool) {
	if start+4 > len(data) {
		return 0, false
	}
	var value uint16
	for _, character := range data[start : start+4] {
		digit, valid := hexDigit(character)
		if !valid {
			return 0, false
		}
		value = value*16 + uint16(digit)
	}
	return value, true
}

func hexDigit(character byte) (byte, bool) {
	switch {
	case character >= '0' && character <= '9':
		return character - '0', true
	case character >= 'a' && character <= 'f':
		return character - 'a' + 10, true
	case character >= 'A' && character <= 'F':
		return character - 'A' + 10, true
	default:
		return 0, false
	}
}
