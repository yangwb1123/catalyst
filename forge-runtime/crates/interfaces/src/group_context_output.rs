use std::io::{self, Write};

use serde::Serialize;

use crate::runtime_domain::{
    Conversation, GroupContextMember, GroupContextPayload, GroupContextPolicy, GroupContextPrompt,
    GroupContextProvenance, GroupContextSlice, GroupContextStats, SessionGroup,
};

#[derive(Debug, Serialize)]
pub struct GroupContextView {
    pub v: u16,
    pub payload: GroupContextPayloadView,
    pub slice_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct GroupContextPayloadView {
    pub policy: GroupContextPolicy,
    pub group: SessionGroup,
    pub members: Vec<GroupContextMember>,
    pub conversations: Vec<GroupContextConversationView>,
    pub stats: GroupContextStats,
}

#[derive(Debug, Serialize)]
pub struct GroupContextConversationView {
    pub conversation: Conversation,
    pub provenance: GroupContextProvenance,
    pub prompts: Vec<GroupContextPromptView>,
    pub omitted_prompt_count: usize,
}

#[derive(Debug, Serialize)]
pub struct GroupContextPromptView {
    pub id: String,
    pub role: String,
    pub created_at_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
    pub original_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
    pub truncated: bool,
}

impl GroupContextView {
    pub fn from_slice(slice: GroupContextSlice, include_content: bool) -> Self {
        let GroupContextSlice {
            v,
            payload,
            slice_sha256,
        } = slice;
        let GroupContextPayload {
            policy,
            group,
            members,
            conversations,
            stats,
        } = payload;
        let conversations = conversations
            .into_iter()
            .map(|item| GroupContextConversationView {
                conversation: item.conversation,
                provenance: item.provenance,
                prompts: item
                    .prompts
                    .into_iter()
                    .map(|prompt| prompt_view(prompt, include_content))
                    .collect(),
                omitted_prompt_count: item.omitted_prompt_count,
            })
            .collect();
        Self {
            v,
            payload: GroupContextPayloadView {
                policy,
                group,
                members,
                conversations,
                stats,
            },
            slice_sha256,
        }
    }
}

pub fn write_group_context(
    context: &GroupContextView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    let payload = &context.payload;
    writeln!(
        writer,
        "group context {} — {}",
        payload.group.id,
        terminal_text(&payload.group.name)
    )?;
    writeln!(
        writer,
        "{} member(s) · {} conversation(s) · {} prompt(s) · {} content byte(s)",
        payload.stats.member_count,
        payload.stats.conversation_count,
        payload.stats.prompt_count,
        payload.stats.content_bytes
    )?;
    writeln!(
        writer,
        "{} conversation(s) omitted · {} prompt(s) omitted · {} prompt(s) truncated",
        payload.stats.omitted_conversation_count,
        payload.stats.omitted_prompt_count,
        payload.stats.truncated_prompt_count
    )?;
    writeln!(writer, "sha256 {}", context.slice_sha256)?;
    for member in &payload.members {
        writeln!(
            writer,
            "member {} — {} ({})",
            member.project_id,
            terminal_text(&member.project_name),
            terminal_text(&member.role)
        )?;
    }
    for conversation in &payload.conversations {
        write_conversation(conversation, writer)?;
    }
    Ok(())
}

fn prompt_view(prompt: GroupContextPrompt, include_content: bool) -> GroupContextPromptView {
    GroupContextPromptView {
        id: prompt.id,
        role: prompt.role,
        created_at_ms: prompt.created_at_ms,
        excerpt: include_content.then_some(prompt.excerpt),
        original_bytes: prompt.original_bytes,
        content_sha256: include_content.then_some(prompt.content_sha256),
        truncated: prompt.truncated,
    }
}

fn write_conversation(
    context: &GroupContextConversationView,
    writer: &mut impl Write,
) -> Result<(), io::Error> {
    writeln!(
        writer,
        "session {} — {} [{}]",
        context.conversation.id,
        terminal_text(&context.conversation.title),
        provenance_label(&context.provenance)
    )?;
    if context.omitted_prompt_count > 0 {
        writeln!(
            writer,
            "  {} earlier prompt(s) omitted",
            context.omitted_prompt_count
        )?;
    }
    for prompt in &context.prompts {
        write_prompt(prompt, writer)?;
    }
    Ok(())
}

fn write_prompt(prompt: &GroupContextPromptView, writer: &mut impl Write) -> Result<(), io::Error> {
    write!(
        writer,
        "  prompt {} — {} — {} byte(s)",
        prompt.id, prompt.role, prompt.original_bytes
    )?;
    if let Some(hash) = &prompt.content_sha256 {
        write!(writer, " — sha256 {hash}")?;
    }
    if prompt.truncated {
        write!(writer, " — truncated")?;
    }
    writeln!(writer)?;
    if let Some(excerpt) = &prompt.excerpt {
        writeln!(writer, "    {}", terminal_text(excerpt))?;
    }
    Ok(())
}

fn provenance_label(provenance: &GroupContextProvenance) -> String {
    match provenance {
        GroupContextProvenance::Group { group_id } => format!("group:{group_id}"),
        GroupContextProvenance::Project { project_id, role } => {
            format!("project:{project_id}, role:{}", terminal_text(role))
        }
    }
}

fn terminal_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{1b}' => escaped.push_str("\\x1b"),
            '\u{2028}' => escaped.push_str("\\u{2028}"),
            '\u{2029}' => escaped.push_str("\\u{2029}"),
            value if is_bidi_control(value) => escaped.extend(value.escape_unicode()),
            value if value.is_control() => escaped.extend(value.escape_default()),
            value => escaped.push(value),
        }
    }
    escaped
}

fn is_bidi_control(value: char) -> bool {
    matches!(
        value,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

#[cfg(test)]
mod tests {
    use super::terminal_text;

    #[test]
    fn human_text_escapes_layout_and_terminal_controls() {
        assert_eq!(
            terminal_text("中文\nline\t\u{1b}[2J\u{2028}\u{202e}"),
            "中文\\nline\\t\\x1b[2J\\u{2028}\\u{202e}"
        );
    }
}
