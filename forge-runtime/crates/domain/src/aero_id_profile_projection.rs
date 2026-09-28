//! Pure consumer for the bounded Aero-ID profile projection.
//!
//! The values are caller-supplied declarations.  This module does not call
//! Aero-ID, inspect a token, persist a profile, or grant Forge authority.

use std::collections::HashSet;

use serde::{Deserialize, de, de::DeserializeSeed};

pub const SCHEMA_VERSION: &str = "forge.aero-id-profile-projection/v1";
pub const EVALUATION_MODE: &str = "pure_projection_only";
pub const SOURCE: &str = "aero-id";
pub const NOTICE: &str = "This is a caller-supplied Aero-ID profile/membership projection. Owner, account, profile, membership, consistency, and status values are unverified; it grants no Forge authorization, device, reservation, scheduling, dispatch, or execution authority.";
pub const MAX_BYTES: usize = 256 * 1024;
pub const MAX_MEMBERSHIPS: usize = 128;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    pub issuer: String,
    pub subject: String,
    pub tenant_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub account_id: String,
    pub display_name: String,
    pub avatar_url: String,
    pub locale: String,
    pub timezone: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Membership {
    pub source: String,
    pub scope_type: String,
    pub scope_id: String,
    pub role: String,
    pub status: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The frozen projection names each independently denied authority capability."
)]
pub struct Authority {
    pub identity_verified: bool,
    pub profile_authoritative: bool,
    pub membership_authoritative: bool,
    pub authorization_granted: bool,
    pub execution_authorized: bool,
    pub reservation_created: bool,
    pub dispatch_performed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The shared wire retains separate unverified and partial-observation flags."
)]
pub struct Projection {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub source: String,
    pub owner_declaration: Owner,
    pub owner_declaration_unverified: bool,
    pub profile_attributes_unverified: bool,
    pub membership_attributes_unverified: bool,
    pub profile: Profile,
    pub memberships: Vec<Membership>,
    pub consistency: String,
    pub partial: bool,
    pub notice: String,
    pub authority: Authority,
}

/// Decode and validate only the supplied projection bytes.
///
/// # Errors
///
/// Rejects empty or oversized input, malformed or duplicate-key JSON, unknown
/// fields, and projections that violate the envelope or membership constraints.
pub fn decode(bytes: &[u8]) -> Result<Projection, String> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("aero-id profile projection size is invalid".into());
    }
    reject_duplicate_keys(bytes)
        .map_err(|error| format!("aero-id profile projection JSON is invalid: {error}"))?;
    let projection: Projection = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode aero-id profile projection: {error}"))?;
    validate(&projection)?;
    Ok(projection)
}

/// Reject duplicate keys at every object depth before serde materializes maps
/// or structs. `serde_json` otherwise keeps the last value for a duplicate
/// key, which would make the Rust consumer less strict than the Go contract
/// decoder for the same caller-supplied projection.
fn reject_duplicate_keys(bytes: &[u8]) -> Result<(), serde_json::Error> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    UniqueJson.deserialize(&mut decoder)?;
    decoder.end()
}

struct UniqueJson;

impl<'de> serde::de::DeserializeSeed<'de> for UniqueJson {
    type Value = ();

    fn deserialize<D: de::Deserializer<'de>>(self, decoder: D) -> Result<(), D::Error> {
        decoder.deserialize_any(self)
    }
}

impl<'de> de::Visitor<'de> for UniqueJson {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence.next_element_seed(UniqueJson)?.is_some() {}
        Ok(())
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut object: A) -> Result<(), A::Error> {
        let mut keys = HashSet::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            object.next_value_seed(UniqueJson)?;
        }
        Ok(())
    }
}

/// Validate the versioned shape and deterministic membership ordering.
///
/// # Errors
///
/// Rejects invalid envelope, authority, consistency, owner, profile, or membership
/// declarations, including excessive, duplicate, and unsorted memberships.
pub fn validate(projection: &Projection) -> Result<(), String> {
    if projection.schema_version != SCHEMA_VERSION
        || projection.evaluation_mode != EVALUATION_MODE
        || projection.source != SOURCE
        || projection.notice != NOTICE
    {
        return Err("aero-id profile projection envelope is invalid".into());
    }
    if !projection.owner_declaration_unverified
        || !projection.profile_attributes_unverified
        || !projection.membership_attributes_unverified
        || projection.authority != Authority::default()
    {
        return Err("aero-id profile projection claims authority".into());
    }
    if !matches!(
        projection.consistency.as_str(),
        "eventual" | "bounded" | "strong"
    ) {
        return Err("aero-id profile projection consistency is invalid".into());
    }
    validate_owner(&projection.owner_declaration)?;
    validate_profile(&projection.profile)?;
    if projection.memberships.len() > MAX_MEMBERSHIPS {
        return Err("aero-id profile projection has too many memberships".into());
    }
    let mut previous: Option<(&str, &str, &str, &str, &str)> = None;
    for (index, membership) in projection.memberships.iter().enumerate() {
        validate_membership(membership).map_err(|error| format!("membership {index}: {error}"))?;
        let key = (
            membership.source.as_str(),
            membership.scope_type.as_str(),
            membership.scope_id.as_str(),
            membership.role.as_str(),
            membership.status.as_str(),
        );
        if previous.is_some_and(|old| old >= key) {
            return Err("aero-id profile projection memberships are not sorted".into());
        }
        previous = Some(key);
    }
    Ok(())
}

/// Require an exact owner tuple match.  The tuple is still unverified data;
/// this only prevents a confused-deputy display across owners.
///
/// # Errors
///
/// Rejects an invalid expected owner or a projection declaring a different owner.
pub fn bind_owner(projection: &Projection, expected: &Owner) -> Result<(), String> {
    validate_owner(expected)?;
    if projection.owner_declaration != *expected {
        return Err("aero-id profile projection owner does not match".into());
    }
    Ok(())
}

fn validate_owner(owner: &Owner) -> Result<(), String> {
    for (name, value) in [
        ("issuer", owner.issuer.as_str()),
        ("subject", owner.subject.as_str()),
        ("tenant_id", owner.tenant_id.as_str()),
    ] {
        if !valid_text(value, 256, false) {
            return Err(format!("aero-id profile owner {name} is invalid"));
        }
    }
    Ok(())
}

fn validate_profile(profile: &Profile) -> Result<(), String> {
    if !valid_text(&profile.account_id, 128, false)
        || !valid_text(&profile.display_name, 512, true)
        || !valid_text(&profile.avatar_url, 2048, true)
        || !valid_text(&profile.locale, 128, true)
        || !valid_text(&profile.timezone, 128, true)
    {
        return Err("aero-id profile fields are invalid".into());
    }
    Ok(())
}

fn validate_membership(membership: &Membership) -> Result<(), String> {
    if !valid_text(&membership.source, 128, false)
        || !valid_text(&membership.scope_type, 128, false)
        || !valid_text(&membership.scope_id, 256, false)
        || !valid_text(&membership.role, 128, false)
        || !valid_text(&membership.status, 128, false)
    {
        return Err("aero-id membership fields are invalid".into());
    }
    Ok(())
}

fn valid_text(value: &str, max: usize, allow_empty: bool) -> bool {
    value.len() <= max
        && value == value.trim()
        && (allow_empty || !value.is_empty())
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!(
        "../../../../docs/contracts/fixtures/forge-aero-id-profile-projection-v1.json"
    );

    #[test]
    fn fixture_is_a_pure_owner_bound_projection() {
        let projection = decode(FIXTURE.as_bytes()).expect("projection fixture");
        bind_owner(&projection, &projection.owner_declaration).expect("same owner");
        let mut foreign = projection.owner_declaration.clone();
        foreign.tenant_id = "tenant-foreign".into();
        assert!(bind_owner(&projection, &foreign).is_err());
        assert_eq!(projection.authority, Authority::default());
    }

    #[test]
    fn fixture_mutations_fail_closed() {
        let cases = [
            (
                "unknown",
                FIXTURE.replace(
                    "{\n  \"schema_version\"",
                    "{\n  \"unexpected\": true,\n  \"schema_version\"",
                ),
            ),
            (
                "authority",
                FIXTURE.replace(
                    "\"authorization_granted\": false",
                    "\"authorization_granted\": true",
                ),
            ),
            (
                "unsorted",
                FIXTURE.replace("\"source\": \"aero-im\"", "\"source\": \"z-aero-im\""),
            ),
        ];
        for (name, value) in cases {
            assert!(
                decode(value.as_bytes()).is_err(),
                "{name} mutation accepted"
            );
        }
    }

    #[test]
    fn decoder_rejects_duplicate_keys_at_any_object_depth() {
        let duplicate_root = FIXTURE.replacen(
            "\"source\": \"aero-id\",",
            "\"source\": \"aero-id\",\n  \"source\": \"aero-id\",",
            1,
        );
        assert!(decode(duplicate_root.as_bytes()).is_err());

        let duplicate_nested = FIXTURE.replacen(
            "\"account_id\": \"11111111-1111-4111-8111-111111111111\",",
            "\"account_id\": \"11111111-1111-4111-8111-111111111111\",\n      \"account_id\": \"11111111-1111-4111-8111-111111111111\",",
            1,
        );
        assert!(decode(duplicate_nested.as_bytes()).is_err());
    }
}
