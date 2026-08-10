use super::{ArtifactEvidenceContractError, invalid};

const SECONDS_PER_DAY: i64 = 86_400;

pub(super) fn unix_millis_floor(value: &str) -> Result<i64, ArtifactEvidenceContractError> {
    let parsed = parse_timestamp(value)?;
    let days = days_since_epoch(parsed.year, parsed.month, parsed.day)?;
    let local_seconds = days
        .checked_mul(SECONDS_PER_DAY)
        .and_then(|seconds| seconds.checked_add(i64::from(parsed.second_of_day)))
        .ok_or_else(|| invalid("artifact created_at is outside the timestamp range"))?;
    let unix_seconds = local_seconds
        .checked_sub(i64::from(parsed.offset_seconds))
        .ok_or_else(|| invalid("artifact created_at is outside the timestamp range"))?;
    let millis = unix_seconds
        .checked_mul(1_000)
        .and_then(|seconds| seconds.checked_add(i64::from(parsed.nanosecond / 1_000_000)))
        .ok_or_else(|| invalid("artifact created_at is outside the timestamp range"))?;
    if millis < 0 {
        return Err(invalid(
            "artifact created_at must not precede the Unix epoch",
        ));
    }
    Ok(millis)
}

#[derive(Clone, Copy)]
struct ParsedTimestamp {
    year: i32,
    month: u8,
    day: u8,
    second_of_day: u32,
    nanosecond: u32,
    offset_seconds: i32,
}

fn parse_timestamp(value: &str) -> Result<ParsedTimestamp, ArtifactEvidenceContractError> {
    let bytes = value.as_bytes();
    if bytes.len() < 20 || !fixed_separators(bytes) {
        return Err(invalid("artifact created_at must be strict RFC3339Nano"));
    }
    let year = i32::try_from(parse_digits(bytes, 0, 4)?)
        .map_err(|_| invalid("artifact created_at year is invalid"))?;
    let month = u8::try_from(parse_digits(bytes, 5, 2)?)
        .map_err(|_| invalid("artifact created_at month is invalid"))?;
    let day = u8::try_from(parse_digits(bytes, 8, 2)?)
        .map_err(|_| invalid("artifact created_at day is invalid"))?;
    validate_date(year, month, day)?;
    let hour = parse_digits(bytes, 11, 2)?;
    let minute = parse_digits(bytes, 14, 2)?;
    let second = parse_digits(bytes, 17, 2)?;
    if hour > 23 || minute > 59 || second > 59 {
        return Err(invalid("artifact created_at clock time is invalid"));
    }
    let (nanosecond, timezone_index) = parse_fraction(bytes)?;
    let offset_seconds = parse_offset(bytes, timezone_index)?;
    Ok(ParsedTimestamp {
        year,
        month,
        day,
        second_of_day: hour * 3_600 + minute * 60 + second,
        nanosecond,
        offset_seconds,
    })
}

fn fixed_separators(bytes: &[u8]) -> bool {
    bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes.get(10) == Some(&b'T')
        && bytes.get(13) == Some(&b':')
        && bytes.get(16) == Some(&b':')
}

fn parse_fraction(bytes: &[u8]) -> Result<(u32, usize), ArtifactEvidenceContractError> {
    if bytes[19] != b'.' {
        return Ok((0, 19));
    }
    let timezone_index = bytes[20..]
        .iter()
        .position(|byte| matches!(byte, b'Z' | b'+' | b'-'))
        .map(|index| index + 20)
        .ok_or_else(|| invalid("artifact created_at timezone is missing"))?;
    let digits = &bytes[20..timezone_index];
    if digits.is_empty() || digits.len() > 9 || !digits.iter().all(u8::is_ascii_digit) {
        return Err(invalid("artifact created_at fraction is invalid"));
    }
    let mut nanoseconds = parse_digits(bytes, 20, digits.len())?;
    for _ in digits.len()..9 {
        nanoseconds *= 10;
    }
    Ok((nanoseconds, timezone_index))
}

fn parse_offset(bytes: &[u8], index: usize) -> Result<i32, ArtifactEvidenceContractError> {
    match bytes.get(index) {
        Some(b'Z') if index + 1 == bytes.len() => Ok(0),
        Some(sign @ (b'+' | b'-')) if index + 6 == bytes.len() && bytes[index + 3] == b':' => {
            let hour = parse_digits(bytes, index + 1, 2)?;
            let minute = parse_digits(bytes, index + 4, 2)?;
            if hour > 23 || minute > 59 {
                return Err(invalid("artifact created_at offset is invalid"));
            }
            let seconds = i32::try_from(hour * 3_600 + minute * 60)
                .map_err(|_| invalid("artifact created_at offset is invalid"))?;
            Ok(if *sign == b'-' { -seconds } else { seconds })
        }
        _ => Err(invalid("artifact created_at timezone is invalid")),
    }
}

fn parse_digits(
    bytes: &[u8],
    start: usize,
    length: usize,
) -> Result<u32, ArtifactEvidenceContractError> {
    let digits = bytes
        .get(start..start + length)
        .ok_or_else(|| invalid("artifact created_at is truncated"))?;
    if !digits.iter().all(u8::is_ascii_digit) {
        return Err(invalid("artifact created_at contains a nondigit"));
    }
    Ok(digits
        .iter()
        .fold(0_u32, |value, digit| value * 10 + u32::from(*digit - b'0')))
}

fn validate_date(year: i32, month: u8, day: u8) -> Result<(), ArtifactEvidenceContractError> {
    if year < 1 || !(1..=12).contains(&month) {
        return Err(invalid("artifact created_at date is invalid"));
    }
    let maximum = days_in_month(year, month);
    if day == 0 || day > maximum {
        return Err(invalid("artifact created_at date is invalid"));
    }
    Ok(())
}

fn days_since_epoch(year: i32, month: u8, day: u8) -> Result<i64, ArtifactEvidenceContractError> {
    let mut days = 0_i64;
    if year >= 1970 {
        for prior_year in 1970..year {
            days += if leap_year(prior_year) { 366 } else { 365 };
        }
    } else {
        for prior_year in year..1970 {
            days -= if leap_year(prior_year) { 366 } else { 365 };
        }
    }
    for prior_month in 1..month {
        days += i64::from(days_in_month(year, prior_month));
    }
    days.checked_add(i64::from(day) - 1)
        .ok_or_else(|| invalid("artifact created_at date is outside the supported range"))
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
