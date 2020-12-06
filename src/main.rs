use std::fs;
use std::io;
use std::io::prelude::*;
use std::path;
use std::process;

fn main() {
    let config_text = match load_file(&path::Path::new("mira.json")) {
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
    auth: serde_json::Value,
    configurations: Vec<Configuration>,
}

/// Authentication options, unused at the moment.
#[derive(Debug, serde::Deserialize)]
struct Authentication {
    key: Option<String>,
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
    // Ensure the repository is cloned.
    if !repo_path.exists() {
        if !clone(src_url, path, name) {
            return Ok(MirrorResult::CloneFailed)
        }
    }
    // Ensure the mirror remote is available.
    let remotes = match get_remotes(&repo_path) {
        Some(remotes) => remotes,
        None => return Ok(MirrorResult::RemotesError)
    };
    // Push to the mirror repo.
    if !push(&repo_path, dest_url) {
        return Ok(MirrorResult::PushFailed)
    }
    Ok(MirrorResult::Success)
}

/// Run a git mirror clone command.
fn clone(url: &str, path: &path::Path, name: &str) -> bool {
    let (success, _) = run_git_command_in(vec!("clone", "--mirror", url, name), path);
    success
}

/// Return a vector of remote names on success.
fn get_remotes(path: &path::Path) -> Option<Vec<String>> {
    let (success, stdout) = run_git_command_in(vec!("remote"), path);
    if !success || stdout.is_none() {
        return None
    }
    Some(stdout.unwrap().split_whitespace().map(|s| s.to_string()).collect())
}

/// Run a git mirror push command.
fn push(path: &path::Path, url: &str) -> bool {
    let (success, _) = run_git_command_in(vec!("push", "--mirror", url), path);
    success
}

/// Run a git command with supplied arguments, return true on successful completion.
fn run_git_command(args: Vec<&str>) -> (bool, Option<String>) {
    let mut command = process::Command::new("git");
    command.args(&args);
    match command.output() {
        Ok(output) => {
            let success = output.status.success();
            let stdout = String::from_utf8(output.stdout).ok();
            (success, stdout)
        }
        Err(e) => { eprintln!("Failed to run Git: {}", e); (false, None) }
    }
}

/// Call `run_git_command` but with a work directory specified.
fn run_git_command_in(args: Vec<&str>, path: &path::Path) -> (bool, Option<String>) {
    let path = match path.to_str() {
        Some(path) => path,
        None => { eprintln!("Invalid path: {:?}", path); return (false, None) }
    };
    let mut full_args = vec!("-C", path);
    full_args.extend(args.clone());
    run_git_command(full_args)
}
