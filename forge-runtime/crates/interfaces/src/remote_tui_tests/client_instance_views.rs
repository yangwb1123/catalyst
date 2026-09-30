use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};

use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[path = "client_instance_views/authenticated_reads.rs"]
mod authenticated_reads;
#[path = "client_instance_views/convergence.rs"]
mod convergence;
#[path = "client_instance_views/refresh_retention.rs"]
mod refresh_retention;
#[path = "client_instance_views/refresh_visibility.rs"]
mod refresh_visibility;
#[path = "client_instance_views/revocation.rs"]
mod revocation;
