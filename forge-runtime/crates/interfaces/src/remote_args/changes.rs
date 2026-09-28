use std::collections::VecDeque;

use crate::args::{Command, next_value, require_empty, usage};

use super::{
    RemoteCommand, parse_bounded_u64, parse_bounded_usize, parse_instance_id,
    parse_safe_integer_u64,
};

pub(super) fn parse_changes(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    match tokens.pop_front().as_deref() {
        Some("list") => parse_list(tokens),
        Some("watch") => parse_watch(tokens),
        Some("stream") => parse_stream(tokens),
        Some(value) => Err(format!(
            "unknown remote changes command '{value}'\n\n{}",
            usage()
        )),
        None => Err(format!("remote changes command is required\n\n{}", usage())),
    }
}

fn parse_list(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut after_cursor = None;
    let mut instance_id = None;
    let mut instance_view = None;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--after-cursor" if after_cursor.is_none() => {
                after_cursor = Some(parse_safe_integer_u64(
                    &next_value(tokens, "--after-cursor")?,
                    "--after-cursor",
                )?);
            }
            "--instance" if instance_id.is_none() => {
                instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote changes list option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote changes list --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::ChangesList {
        after_cursor,
        instance_id,
        instance_view,
    }))
}

fn parse_watch(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let WatchOptions {
        after_cursor,
        polls,
        min_delay_ms,
        max_delay_ms,
        instance_id,
        instance_view,
        ..
    } = WatchOptions::parse(tokens)?;
    require_empty(tokens)?;
    if max_delay_ms < min_delay_ms {
        return Err(format!(
            "--max-delay-ms must be greater than or equal to --min-delay-ms\n\n{}",
            usage()
        ));
    }
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote changes watch --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::ChangesWatch {
        after_cursor,
        polls,
        min_delay_ms,
        max_delay_ms,
        instance_id,
        instance_view,
    }))
}

fn parse_stream(tokens: &mut VecDeque<String>) -> Result<Command, String> {
    let mut after_cursor = None;
    let mut wait_ms = 5_000;
    let mut instance_id = None;
    let mut instance_view = None;
    let mut after_cursor_seen = false;
    let mut wait_seen = false;
    while let Some(option) = tokens.pop_front() {
        match option.as_str() {
            "--after-cursor" if !after_cursor_seen => {
                after_cursor = Some(parse_safe_integer_u64(
                    &next_value(tokens, "--after-cursor")?,
                    "--after-cursor",
                )?);
                after_cursor_seen = true;
            }
            "--wait-ms" if !wait_seen => {
                wait_ms =
                    parse_bounded_u64(&next_value(tokens, "--wait-ms")?, 0, 10_000, "--wait-ms")?;
                wait_seen = true;
            }
            "--instance" if instance_id.is_none() => {
                instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if instance_view.is_none() => {
                instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote changes stream option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
    }
    require_empty(tokens)?;
    if instance_view.is_some() && instance_id.is_none() {
        return Err(format!(
            "remote changes stream --instance-view requires --instance\n\n{}",
            usage()
        ));
    }
    Ok(Command::Remote(RemoteCommand::ChangesStream {
        after_cursor,
        wait_ms,
        instance_id,
        instance_view,
    }))
}

struct WatchOptions {
    after_cursor: Option<u64>,
    polls: usize,
    min_delay_ms: u64,
    max_delay_ms: u64,
    instance_id: Option<String>,
    instance_view: Option<String>,
    polls_seen: bool,
    min_delay_seen: bool,
    max_delay_seen: bool,
}

impl WatchOptions {
    fn parse(tokens: &mut VecDeque<String>) -> Result<Self, String> {
        let mut options = Self {
            after_cursor: None,
            polls: 8,
            min_delay_ms: 250,
            max_delay_ms: 5_000,
            instance_id: None,
            instance_view: None,
            polls_seen: false,
            min_delay_seen: false,
            max_delay_seen: false,
        };
        while let Some(option) = tokens.pop_front() {
            options.parse_option(&option, tokens)?;
        }
        Ok(options)
    }

    fn parse_option(&mut self, option: &str, tokens: &mut VecDeque<String>) -> Result<(), String> {
        match option {
            "--after-cursor" if self.after_cursor.is_none() => {
                self.after_cursor = Some(parse_safe_integer_u64(
                    &next_value(tokens, "--after-cursor")?,
                    "--after-cursor",
                )?);
            }
            "--polls" if !self.polls_seen => {
                self.polls =
                    parse_bounded_usize(&next_value(tokens, "--polls")?, 1, 64, "--polls")?;
                self.polls_seen = true;
            }
            "--min-delay-ms" if !self.min_delay_seen => {
                self.min_delay_ms = parse_bounded_u64(
                    &next_value(tokens, "--min-delay-ms")?,
                    0,
                    10_000,
                    "--min-delay-ms",
                )?;
                self.min_delay_seen = true;
            }
            "--max-delay-ms" if !self.max_delay_seen => {
                self.max_delay_ms = parse_bounded_u64(
                    &next_value(tokens, "--max-delay-ms")?,
                    0,
                    60_000,
                    "--max-delay-ms",
                )?;
                self.max_delay_seen = true;
            }
            "--instance" if self.instance_id.is_none() => {
                self.instance_id = Some(parse_instance_id(tokens)?);
            }
            "--instance-view" if self.instance_view.is_none() => {
                self.instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            _ => {
                return Err(format!(
                    "invalid remote changes watch option '{option}'\n\n{}",
                    usage()
                ));
            }
        }
        Ok(())
    }
}
