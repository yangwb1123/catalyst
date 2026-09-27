use std::collections::VecDeque;

use crate::args::{Command, next_value, require_empty, usage};

use super::{RemoteCommand, parse_bounded_usize, parse_instance_id, parse_safe_integer_u64};

pub(super) fn parse_pending_run_intents(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_pending_run_intent_list(tokens),
        Some("submit") => parse_pending_run_intent_submit(tokens),
        Some("timeline") => parse_pending_run_intent_timeline(tokens),
        Some(value) => Err(format!(
            "unknown remote run-intents command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!(
            "remote run-intents command is required\n\n{}",
            usage()
        )),
    }
}

fn parse_pending_run_intent_submit(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote run-intents submit")?;
    if tokens
        .front()
        .is_none_or(|value| value != "--expected-version")
    {
        return Err(format!(
            "remote run-intent submit requires --expected-version from the session list\n\n{}",
            usage()
        ));
    }
    tokens.pop_front();
    let version = next_value(tokens, "--expected-version")?;
    let expected_version = version
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0 && *value <= 9_007_199_254_740_991)
        .ok_or_else(|| format!("invalid --expected-version '{version}'\n\n{}", usage()))?;
    let (instance_id, instance_view) = parse_submit_instance_options(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote run-intents submit --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    if tokens.is_empty() {
        return Err(format!(
            "pending Run-intent content is required\n\n{}",
            usage()
        ));
    }
    let content_tokens = tokens.drain(..).collect::<Vec<_>>();
    if content_tokens.first().is_some_and(|token| token == "-") && content_tokens.len() != 1 {
        return Err(format!(
            "remote stdin pending Run-intent marker '-' must be the only content token\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::PendingRunIntentSubmit {
        conversation_id,
        expected_version,
        content: content_tokens.join(" "),
        instance_id,
        instance_view,
    }))
}

fn parse_pending_run_intent_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote run-intents list")?;
    let IntentListOptions {
        limit,
        before_submitted_at_ms,
        before_intent_id,
        instance_id,
        instance_view,
        ..
    } = IntentListOptions::parse(tokens)?;
    require_empty(tokens)?;
    if before_submitted_at_ms.is_some() != before_intent_id.is_some() {
        return Err(format!(
            "remote run-intents list cursor requires both --before-submitted-at-ms and --before-intent-id\n\n{}",
            usage()
        ));
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote run-intents list --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::PendingRunIntentsList {
        conversation_id,
        limit,
        before_submitted_at_ms,
        before_intent_id,
        instance_id,
        instance_view,
    }))
}

fn parse_pending_run_intent_timeline(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote run-intents timeline")?;
    let intent_id = next_value(tokens, "remote run-intents timeline")?;
    let IntentTimelineOptions {
        after_sequence,
        limit,
        instance_id,
        instance_view,
        ..
    } = IntentTimelineOptions::parse(tokens)?;
    require_empty(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote run-intents timeline --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::PendingRunIntentTimeline {
        conversation_id,
        intent_id,
        after_sequence,
        limit,
        instance_id,
        instance_view,
    }))
}

struct IntentListOptions {
    limit: usize,
    limit_seen: bool,
    before_submitted_at_ms: Option<u64>,
    before_intent_id: Option<String>,
    instance_id: Option<String>,
    instance_view: Option<String>,
}

impl IntentListOptions {
    fn parse(tokens: &mut VecDeque<String>) -> Result<Self, String> {
        let mut options = Self {
            limit: 25,
            limit_seen: false,
            before_submitted_at_ms: None,
            before_intent_id: None,
            instance_id: None,
            instance_view: None,
        };
        while let Some(option) = tokens.pop_front() {
            options.parse_option(&option, tokens)?;
        }
        Ok(options)
    }

    fn parse_option(&mut self, option: &str, tokens: &mut VecDeque<String>) -> Result<(), String> {
        match option {
            "--limit" if !self.limit_seen => {
                self.limit =
                    parse_bounded_usize(&next_value(tokens, "--limit")?, 1, 25, "--limit")?;
                self.limit_seen = true;
            }
            "--before-submitted-at-ms" if self.before_submitted_at_ms.is_none() => {
                self.before_submitted_at_ms = Some(parse_safe_integer_u64(
                    &next_value(tokens, "--before-submitted-at-ms")?,
                    "--before-submitted-at-ms",
                )?);
            }
            "--before-intent-id" if self.before_intent_id.is_none() => {
                self.before_intent_id = Some(next_value(tokens, "--before-intent-id")?);
            }
            "--instance" if self.instance_id.is_none() => {
                self.instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if self.instance_view.is_none() => {
                self.instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote run-intents list option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
        Ok(())
    }
}

struct IntentTimelineOptions {
    after_sequence: u64,
    limit: usize,
    after_sequence_seen: bool,
    limit_seen: bool,
    instance_id: Option<String>,
    instance_view: Option<String>,
}

impl IntentTimelineOptions {
    fn parse(tokens: &mut VecDeque<String>) -> Result<Self, String> {
        let mut options = Self {
            after_sequence: 0,
            limit: 25,
            after_sequence_seen: false,
            limit_seen: false,
            instance_id: None,
            instance_view: None,
        };
        while let Some(option) = tokens.pop_front() {
            options.parse_option(&option, tokens)?;
        }
        Ok(options)
    }

    fn parse_option(&mut self, option: &str, tokens: &mut VecDeque<String>) -> Result<(), String> {
        match option {
            "--after-sequence" if !self.after_sequence_seen => {
                self.after_sequence = parse_safe_integer_u64(
                    &next_value(tokens, "--after-sequence")?,
                    "--after-sequence",
                )?;
                self.after_sequence_seen = true;
            }
            "--limit" if !self.limit_seen => {
                self.limit =
                    parse_bounded_usize(&next_value(tokens, "--limit")?, 1, 25, "--limit")?;
                self.limit_seen = true;
            }
            "--instance" if self.instance_id.is_none() => {
                self.instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if self.instance_view.is_none() => {
                self.instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote run-intents timeline option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
        Ok(())
    }
}

fn parse_submit_instance_options(
    tokens: &mut VecDeque<String>,
) -> Result<(Option<String>, Option<String>), String> {
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.front().map(String::as_str) {
        match option {
            "--instance" => {
                if instance_id.is_some() {
                    return Err(format!(
                        "invalid remote run-intents submit option '--instance'\n\n{}",
                        usage()
                    ));
                }
                tokens.pop_front();
                instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" => {
                if instance_view.is_some() {
                    return Err(format!(
                        "invalid remote run-intents submit option '--instance-view'\n\n{}",
                        usage()
                    ));
                }
                tokens.pop_front();
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => break,
        }
    }
    Ok((instance_id, instance_view))
}
