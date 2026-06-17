mod assignments;
mod automation;
mod automation_policy;
mod cli;
mod config_host;
mod enrollment;
mod fs_atomic;
mod identity;
mod inspect;
mod paths;
mod provision;
mod registry_cmd;
mod registry_image;
mod role;

use cli::{dispatch, exit_code_for_error, print_human_error};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = dispatch(args) {
        print_human_error(&error);
        std::process::exit(exit_code_for_error(&error));
    }
}
