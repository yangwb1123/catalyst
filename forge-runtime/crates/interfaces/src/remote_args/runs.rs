use std::collections::VecDeque;

use crate::args::{Command, next_value, require_empty, usage};

use super::{RemoteCommand, parse_bounded_usize, parse_instance_id, parse_safe_integer_u64};

pub(super) fn parse_runs(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_run_list(tokens),
        Some("observed") => parse_run_observed(tokens),
        Some("timeline") => parse_run_timeline(tokens),
        Some(value) => Err(format!(
            "unknown remote runs command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("remote runs command is required\n\n{}", usage())),
    }
}

fn parse_run_observed(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote runs observed")?;
    let run_id = next_value(tokens, "remote runs observed")?;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--instance" | "--instance-id" if instance_id.is_none() => {
                instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote runs observed option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote runs observed --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::RunObserved {
        conversation_id,
        run_id,
        instance_id,
        instance_view,
    }))
}

fn parse_run_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote runs list")?;
    let RunListOptions {
        limit,
        before_created_at_ms,
        before_run_id,
        instance_id,
        instance_view,
        ..
    } = RunListOptions::parse(tokens)?;
    require_empty(tokens)?;
    if before_created_at_ms.is_some() != before_run_id.is_some() {
        return Err(format!(
            "remote runs list cursor requires both --before-created-at-ms and --before-run-id\n\n{}",
            usage()
        ));
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote runs list --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::RunsList {
        conversation_id,
        limit,
        before_created_at_ms,
        before_run_id,
        instance_id,
        instance_view,
    }))
}

fn parse_run_timeline(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let conversation_id = next_value(tokens, "remote runs timeline")?;
    let run_id = next_value(tokens, "remote runs timeline")?;
    let RunTimelineOptions {
        after_sequence,
        limit,
        after_sequence_seen,
        resume,
        instance_id,
        instance_view,
        ..
    } = RunTimelineOptions::parse(tokens)?;
    require_empty(tokens)?;
    if resume && after_sequence_seen {
        return Err(format!(
            "remote runs timeline cannot combine --resume with --after-sequence\n\n{}",
            usage()
        ));
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote runs timeline --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::RunTimeline {
        conversation_id,
        run_id,
        after_sequence,
        limit,
        resume,
        instance_id,
        instance_view,
    }))
}

struct RunListOptions {
    limit: usize,
    limit_seen: bool,
    before_created_at_ms: Option<u64>,
    before_run_id: Option<String>,
    instance_id: Option<String>,
    instance_view: Option<String>,
}

impl RunListOptions {
    fn parse(tokens: &mut VecDeque<String>) -> Result<Self, String> {
        let mut options = Self {
            limit: 25,
            limit_seen: false,
            before_created_at_ms: None,
            before_run_id: None,
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
            "--before-created-at-ms" if self.before_created_at_ms.is_none() => {
                self.before_created_at_ms = Some(parse_safe_integer_u64(
                    &next_value(tokens, "--before-created-at-ms")?,
                    "--before-created-at-ms",
                )?);
            }
            "--before-run-id" if self.before_run_id.is_none() => {
                self.before_run_id = Some(next_value(tokens, "--before-run-id")?);
            }
            "--instance" | "--instance-id" if self.instance_id.is_none() => {
                self.instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if self.instance_view.is_none() => {
                self.instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote runs list option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
        Ok(())
    }
}

struct RunTimelineOptions {
    after_sequence: u64,
    limit: usize,
    after_sequence_seen: bool,
    limit_seen: bool,
    resume: bool,
    instance_id: Option<String>,
    instance_view: Option<String>,
}

impl RunTimelineOptions {
    fn parse(tokens: &mut VecDeque<String>) -> Result<Self, String> {
        let mut options = Self {
            after_sequence: 0,
            limit: 128,
            after_sequence_seen: false,
            limit_seen: false,
            resume: false,
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
                    parse_bounded_usize(&next_value(tokens, "--limit")?, 1, 128, "--limit")?;
                self.limit_seen = true;
            }
            "--resume" if !self.resume => {
                self.resume = true;
            }
            "--instance" | "--instance-id" if self.instance_id.is_none() => {
                self.instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if self.instance_view.is_none() => {
                self.instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote runs timeline option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
        Ok(())
    }
}
