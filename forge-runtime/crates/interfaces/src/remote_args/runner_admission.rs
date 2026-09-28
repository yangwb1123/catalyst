use std::collections::VecDeque;

use crate::args::{Command, next_value, usage};

use super::RemoteCommand;

pub(super) enum Kind {
    Dispatch,
    Transport,
}

#[derive(Default)]
struct Options {
    input: Option<String>,
    instance_id: Option<String>,
    instance_view: Option<String>,
}

pub(super) fn parse(tokens: &mut VecDeque<String>, kind: Kind) -> Result<Command, String> {
    let (input, instance_id, instance_view) = parse_options(tokens, kind.label())?;
    Ok(Command::Remote(kind.command(
        input,
        instance_id,
        instance_view,
    )))
}

fn parse_options(
    tokens: &mut VecDeque<String>,
    label: &str,
) -> Result<(String, Option<String>, Option<String>), String> {
    let mut options = Options::default();
    while let Some(option) = tokens.pop_front() {
        options.accept(&option, tokens, label)?;
    }
    options.finish(label)
}

impl Options {
    fn accept(
        &mut self,
        option: &str,
        tokens: &mut VecDeque<String>,
        label: &str,
    ) -> Result<(), String> {
        match option {
            "--input" if self.input.is_none() => {
                self.input = Some(next_value(tokens, "--input")?);
            }
            "--input" => return Err("--input was specified more than once".into()),
            "--instance" if self.instance_id.is_none() => {
                let value = next_value(tokens, "--instance")?;
                crate::client_instance_session_scope::validate_instance_id(&value).map_err(
                    |error| format!("invalid --instance '{value}': {error}\n\n{}", usage()),
                )?;
                self.instance_id = Some(value);
            }
            "--instance" => return Err("--instance was specified more than once".into()),
            "--instance-view" if self.instance_view.is_none() => {
                self.instance_view = Some(next_value(tokens, "--instance-view")?);
            }
            "--instance-view" => {
                return Err("--instance-view was specified more than once".into());
            }
            value => {
                return Err(format!(
                    "invalid remote {label} option '{value}'\n\n{}",
                    usage()
                ));
            }
        }
        Ok(())
    }

    fn finish(self, label: &str) -> Result<(String, Option<String>, Option<String>), String> {
        let input = self
            .input
            .ok_or_else(|| format!("remote {label} requires --input FILE|-\n\n{}", usage()))?;
        if input.trim().is_empty() {
            return Err("--input requires a non-empty FILE|- value".into());
        }
        if self.instance_view.is_some() && self.instance_id.is_none() {
            return Err(format!(
                "remote {label} --instance-view requires --instance\n\n{}",
                usage()
            ));
        }
        Ok((input, self.instance_id, self.instance_view))
    }
}

impl Kind {
    fn command(
        self,
        input: String,
        instance_id: Option<String>,
        instance_view: Option<String>,
    ) -> RemoteCommand {
        match self {
            Kind::Dispatch => RemoteCommand::RunnerDispatchAdmissionPreview {
                input,
                instance_id,
                instance_view,
            },
            Kind::Transport => RemoteCommand::RunnerTransportAdmissionPreview {
                input,
                instance_id,
                instance_view,
            },
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Dispatch => "Runner dispatch admission",
            Self::Transport => "Runner transport admission",
        }
    }
}
