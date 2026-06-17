mod automation;
mod auth;
mod cli;
mod config_host;
mod identity;
mod inspect;
mod paths;
mod role;

use cli::{dispatch, exit_code_for_error, print_human_error};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = dispatch(args) {
        print_human_error(&error);
        std::process::exit(exit_code_for_error(&error));
    }
}
