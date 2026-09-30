use super::{Command, DeviceCommand, DeviceInventoryCommand, DevicePlacementCommand, parse_tokens};

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

#[path = "device_args/client_tests.rs"]
mod client_tests;
#[path = "device_args/inventory_tests.rs"]
mod inventory_tests;
#[path = "device_args/placement_tests.rs"]
mod placement_tests;
#[path = "device_args/run_tests.rs"]
mod run_tests;
#[path = "device_args/runner_tests.rs"]
mod runner_tests;
