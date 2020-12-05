use std::env;
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
    process_root_config(&root_config);
}

fn load_file(path: &path::Path) -> Result<String, io::Error> {
    let mut file = fs::File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

#[derive(Debug, serde::Deserialize)]
struct RootConfig {
    workdir: Option<String>,
    auth: serde_json::Value,
    configurations: Vec<Configuration>,
}

// #[derive(Debug, serde::Deserialize)]
// struct Authentication {
//     key: Option<String>,
// }

fn process_root_config(root_config: &RootConfig) {
    // Ensure working directory exists and move to it.
    if let Some(workdir) = &root_config.workdir {
        let workdir = path::Path::new(&workdir);
        if !workdir.is_dir() {
            if let Err(e) = fs::create_dir_all(&workdir) {
                eprintln!("Failed to create working directory: {}.", e);
                return
            }
        }
        if let Err(e) = env::set_current_dir(&workdir) {
            eprintln!("Failed to move to working directory: {}.", e);
            return
        }
    }
    // Process each configuration.
    for config in &root_config.configurations {
        process_config(config);
    }
}

#[derive(Debug, serde::Deserialize)]
struct Configuration {
    name: String,
    mirrors: Vec<Mirror>,
}

#[derive(Debug, serde::Deserialize)]
struct Mirror {
    src: String,
    dest: String,
}

fn process_config(config: &Configuration) {
    println!("Processing config {}.", config.name);
    // Move into the configuration directory.
    let mut config_path = match env::current_dir() {
        Ok(pb) => pb,
        Err(e) => { eprintln!("Current directory is not available: {}.", e); return }
    };
    config_path.push(&config.name);
    if !config_path.is_dir() {
        if let Err(e) = fs::create_dir_all(&config_path) {
            eprintln!("Failed to create working directory: {}.", e);
            return
        }
    }
    if let Err(e) = env::set_current_dir(&config_path) {
        eprintln!("Failed to move to working directory: {}.", e);
        return
    }
    // Mirror each repository in the configuration.
    for mirror in &config.mirrors {
        mirror_repo(&mirror.src, &mirror.dest);
    }
}

fn mirror_repo(src_url: &str, dest_url: &str) -> bool {
    clone(src_url)
}

fn run_git_command(args: Vec<&str>) -> bool {
    let mut command = process::Command::new("git");
    command.args(&args);
    match command.status() {
        Ok(status) => status.success(),
        Err(e) => { eprintln!("Failed to run Git: {}", e); false }
    }
}

fn clone(url: &str) -> bool {
    run_git_command(vec!("clone", "--mirror", url))
}
