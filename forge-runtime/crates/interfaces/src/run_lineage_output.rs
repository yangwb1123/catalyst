use std::io::{self, Write};

use serde::Serialize;

use crate::runtime_domain::RunLineageRecord;

#[derive(Debug, Serialize)]
pub(crate) struct RunLineageView {
    pub(crate) run_id: String,
    pub(crate) recorded: bool,
    pub(crate) lineage: Option<RunLineageRecord>,
    pub(crate) scope: &'static str,
    pub(crate) content_included: bool,
}

impl RunLineageView {
    pub(crate) fn new(run_id: &str, lineage: Option<RunLineageRecord>) -> Self {
        Self {
            run_id: run_id.into(),
            recorded: lineage.is_some(),
            lineage,
            scope: "direct_parent_only",
            content_included: false,
        }
    }
}

pub(crate) fn write(view: &RunLineageView, writer: &mut impl Write) -> Result<(), io::Error> {
    let run_id = crate::group_context_output::terminal_text(&view.run_id);
    if let Some(lineage) = &view.lineage {
        let parent = crate::group_context_output::terminal_text(&lineage.parent_run_id);
        return writeln!(
            writer,
            "run {run_id} branches directly from {parent} at root input"
        );
    }
    writeln!(writer, "run {run_id} has no recorded direct-parent lineage")
}
