#[cfg(unix)]
use std::fs;
use std::{
    io::{BufRead, BufReader, Cursor, Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, Url, redirect::Policy};
use serde_json::{Value, json};

#[cfg(unix)]
use super::super::super::credentials::{CredentialStore, StoredCredential};
use super::{RemoteClient, run_with_io};

include!("helpers/sync.rs");
include!("helpers/prompt.rs");
include!("helpers/http.rs");
