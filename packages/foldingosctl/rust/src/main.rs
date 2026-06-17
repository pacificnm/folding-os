mod assignments;
mod automation;
mod automation_policy;
mod boot_cmd;
mod cli;
mod config;
mod config_cmd;
mod config_host;
mod enrollment;
mod foldops_manifest;
mod fs_atomic;
mod identity;
mod inspect;
mod paths;
mod process;
mod provision;
mod registry_cmd;
mod registry_foldops_tools;
mod registry_image;
mod registry_import;
mod registry_poll;
mod role;
mod storage;

use cli::{dispatch, exit_code_for_error, print_human_error};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = dispatch(args) {
        print_human_error(&error);
        std::process::exit(exit_code_for_error(&error));
    }
}
