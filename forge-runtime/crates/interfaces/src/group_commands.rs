use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub enum GroupCommand {
    Analysis(GroupAnalysisCommand),
    Create {
        name: String,
    },
    Add {
        group_id: String,
        project: PathBuf,
        role: String,
    },
    Context {
        group_id: String,
        include_content: bool,
        max_bytes: usize,
    },
    Execution(GroupExecutionCommand),
    Run(GroupRunCommand),
    List,
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupAnalysisCommand {
    Prepare {
        group_run_id: String,
        model: Option<String>,
        max_output_tokens: u32,
    },
    Send {
        analysis_id: String,
        confirm_off_machine: bool,
        include_result: bool,
    },
    Show {
        analysis_id: String,
        include_result: bool,
    },
    List {
        group_run_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupExecutionCommand {
    Start {
        group_run_id: String,
    },
    Show {
        execution_id: String,
    },
    List {
        group_run_id: Option<String>,
        limit: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum GroupRunCommand {
    Prepare {
        group_id: String,
        include_content: bool,
        max_bytes: usize,
    },
    Show {
        run_id: String,
        include_content: bool,
    },
    List {
        group_id: Option<String>,
        limit: usize,
    },
}
