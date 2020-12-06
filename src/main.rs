//! Mira -- Git mirrors from a JSON config file.

use std::fs;
use std::io;
use std::io::prelude::*;
use std::path;
use std::process;

const MIRROR_REMOTE_NAME: &str = "mirror";

fn main() {
    let matches = clap::App::new("Mira")
        .setting(clap::AppSettings::ArgRequiredElseHelp)
        .arg(clap::Arg::with_name("config")
             .short("c").long("config").takes_value(true).required(true))
        .get_matches();
    let config_file = matches.value_of("config").unwrap();
    let config_text = match load_file(&path::Path::new(config_file)) {
        Ok(content) => content,
        Err(e) => { eprintln!("{:?}", e); process::exit(1) }
    };
    let root_config: RootConfig = match serde_json::from_str(&config_text) {
        Ok(config) => config,
        Err(e) => { eprintln!("{:?}", e); process::exit(1) }
    };
    process::exit(if process_root_config(&root_config) { 0 } else { 1 });
}

fn load_file(path: &path::Path) -> Result<String, io::Error> {
    let mut file = fs::File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

/// Configuration file.
#[derive(Debug, serde::Deserialize)]
struct RootConfig {
    workspace: String,
    configurations: Vec<Configuration>,
}

/// Server configuration.
#[derive(Debug, serde::Deserialize)]
struct Configuration {
    name: String,
    mirrors: Vec<Mirror>,
}

/// Mirror configuration.
#[derive(Debug, serde::Deserialize)]
struct Mirror {
    name: String,
    src: String,
    dest: String,
}

/// Process the Mira configuration file, return true on complete success.
fn process_root_config(root_config: &RootConfig) -> bool {
    // Ensure working directory exists and move to it.
    let workspace = path::Path::new(&root_config.workspace);
    if !workspace.is_dir() {
        if let Err(e) = fs::create_dir_all(&workspace) {
            eprintln!("Failed to create workspace directory: {}.", e);
            return false
        }
    }
    // Process each configuration, even if some of them fail.
    let mut complete_success = true;
    for config in &root_config.configurations {
        if let Err(e) = process_config(config, workspace) {
            eprintln!("An error occured with configuration {}: {}", config.name, e);
            complete_success = false;
        }
    }
    complete_success
}

/// Result of a mirror operation.
enum MirrorResult {
    Success,
    CloneFailed,
    FetchFailed,
    RemotesError,
    PushFailed,
}

/// Process mirrors of this server configuration.
///
/// If an IO error is met when preparing for the mirroring, this function returns early with this
/// error. After that, all mirrors in `config` are processed, and the function returns true only if
/// every mirror completes succesfully.
fn process_config(config: &Configuration, workspace: &path::Path) -> Result<bool, io::Error> {
    println!("Processing config {}.", config.name);
    // Move into the configuration directory.
    let mut config_path = workspace.to_path_buf();
    config_path.push(&config.name);
    if !config_path.is_dir() {
        fs::create_dir_all(&config_path)?;
    }
    // Mirror each repository in the configuration.
    let mut complete_success = true;
    for mirror in &config.mirrors {
        match mirror_repo(&mirror.name, &mirror.src, &mirror.dest, &config_path) {
            Ok(MirrorResult::Success) => { println!("{} mirrored successfully.", mirror.name); },
            Ok(MirrorResult::CloneFailed) => {
                println!("Failed to clone {}.", mirror.name);
                complete_success = false;
            },
            Ok(MirrorResult::FetchFailed) => {
                println!("Failed to fetch changes for {}.", mirror.name);
                complete_success = false;
            },
            Ok(MirrorResult::RemotesError) => {
                println!("Failed to process remotes for {}.", mirror.name);
                complete_success = false;
            },
            Ok(MirrorResult::PushFailed) => {
                println!("Failed to push {}.", mirror.name);
                complete_success = false;
            },
            Err(e) => {
                eprintln!("An error occured during {} mirroring: {}", mirror.name, e);
                complete_success = false;
            }
        }
    }
    Ok(complete_success)
}

/// Mirror a repository from `src_url` to `dest_url`.
///
/// This function assumes that the current work directory is the workspace,
/// so that a directory named `name` can be used to clone and/or push from.
fn mirror_repo(
    name: &str,
    src_url: &str,
    dest_url: &str,
    path: &path::Path
) -> Result<MirrorResult, io::Error> {
    let mut repo_path = path.to_path_buf();
    repo_path.push(name);
    // Ensure the repository is cloned and up to date.
    if !repo_path.exists() {
        if let Some(e) = check_git_return(&clone(src_url, path, name), MirrorResult::CloneFailed) {
            return Ok(e)
        }
    } else {
        if let Some(e) = check_git_return(&fetch(&repo_path), MirrorResult::FetchFailed) {
            return Ok(e)
        }
    }
    // Ensure the mirror remote is available.
    let remotes = match get_remotes(&repo_path) {
        Some(remotes) => remotes,
        None => return Ok(MirrorResult::RemotesError)
    };
    if !remotes.contains(&MIRROR_REMOTE_NAME.to_string()) {
        if let Some(e) = check_git_return(
            &add_mirror_remote(&repo_path, dest_url),
            MirrorResult::RemotesError
        ) {
            return Ok(e)
        }
    }
    // Push to the mirror repo.
    if let Some(e) = check_git_return(&push(&repo_path), MirrorResult::PushFailed) {
        return Ok(e)
    }
    Ok(MirrorResult::Success)
}

/// Common type for wrappers around Git commands: success and optional stdout.
type GitCmdReturn = (bool, Option<String>);

/// Check a GitCmdReturn.
///
/// Print errors if the command failed and return `Some(on_error)`, or return None if the command
/// completed successfully.
fn check_git_return(cmd_return: &GitCmdReturn, on_error: MirrorResult) -> Option<MirrorResult> {
    match cmd_return {
        (false, output_opt) => {
            if let Some(output) = output_opt {
                eprintln!("Git output:\n{}", output);
            }
            Some(on_error)
        }
        _ => None
    }
}

/// Run a git mirror clone command.
fn clone(url: &str, path: &path::Path, name: &str) -> GitCmdReturn {
    let args = vec!("clone", "--mirror", url, name);
    run_git_command_in(args, path)
}

/// Update a local repository.
fn fetch(path: &path::Path) -> GitCmdReturn {
    run_git_command_in(vec!("fetch"), path)
}

/// Return a vector of remote names on success.
fn get_remotes(path: &path::Path) -> Option<Vec<String>> {
    let (success, stdout) = run_git_command_in(vec!("remote"), path);
    if !success {
        return None
    }
    stdout.and_then(|s| Some(s.split_whitespace().map(|ss| ss.to_string()).collect()))
}

/// Set the mirror remote `url` in the repository at `path`.
fn add_mirror_remote(path: &path::Path, url: &str) -> GitCmdReturn {
    let args = vec!("remote", "add", MIRROR_REMOTE_NAME, url);
    run_git_command_in(args, path)
}

/// Run a git mirror push command.
fn push(path: &path::Path) -> GitCmdReturn {
    let args = vec!("push", "--mirror", MIRROR_REMOTE_NAME);
    run_git_command_in(args, path)
}

/// Run a git command with supplied arguments, return true on successful completion.
fn run_git_command(args: Vec<&str>) -> GitCmdReturn {
    let mut command = process::Command::new("git");
    command.args(&args);
    match command.output() {
        Ok(output) => {
            let success = output.status.success();
            let text = String::from_utf8(
                if success { output.stdout } else { output.stderr }
            ).ok();
            (success, text)
        }
        Err(e) => { eprintln!("Failed to run Git: {}", e); (false, None) }
    }
}

/// Call `run_git_command` but with a work directory specified.
fn run_git_command_in(args: Vec<&str>, path: &path::Path) -> GitCmdReturn {
    let path = match path.to_str() {
        Some(path) => path,
        None => { eprintln!("Invalid path: {:?}", path); return (false, None) }
    };
    let mut full_args = vec!("-C", path);
    full_args.extend(args.clone());
    run_git_command(full_args)
}
