use std::error::Error;
use std::env;
use std::path::PathBuf;
use std::collections::HashMap;
mod config;
use config::Config;
use std::time::Instant;
mod add;
mod init;
use std::path::Path;
use std::sync::Arc;
mod run;
use run::run_script;
mod packageconfig;
mod errors;
mod remove;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let start: Instant = Instant::now();
    
    let mut args_iter = env::args().skip(1);
    let command = match args_iter.next() {
        Some(cmd) => cmd,
        None => {
            println!("Usage: qnpm <command> [options] [package_name]");
            return Ok(());
        }
    };

    // Directly jump to match if command is neither 'config' nor 'add'.
    // In the future, this should be changed to a dynamic solution although this would slow it down.
    // Maybe a dictionary of commands and their requirement to loading configuration files.

    if command != "config" && command != "add" {
        let elapsed = start.elapsed().as_secs_f64();
        println!("Elapsed: {:.8?}", elapsed);
        goto_match(&command, args_iter.collect(), start, PathBuf::from("node_modules")).await;
        return Ok(());
    }

    // Only perform the necessary operations for 'config' and 'add' commands
    let current_dir = env::current_dir()?;
    let config_path = current_dir.join("package_manager_config.json");
    let mut config = Config::load(&config_path)?;

    if command == "config" {
        if let Some(new_cache_dir) = parse_config_args(args_iter) {
            config.cache_dir = PathBuf::from(new_cache_dir);
            config.save(&config_path)?;
            println!("Cache directory updated to: {}", config.cache_dir.display());
        }
    } else {
        let elapsed = start.elapsed().as_secs_f64();
        println!("Elapsed: {:.8?}", elapsed);
        goto_match(&command, args_iter.collect(), start, config.cache_dir).await;
    }

    Ok(())
}

fn parse_config_args(mut args_iter: impl Iterator<Item = String>) -> Option<String> {
    let mut cache_dir = None;
    while let Some(arg) = args_iter.next() {
        if arg == "--cachedir" {
            cache_dir = args_iter.next();
            break;
        }
    }
    cache_dir
}

async fn goto_match(command: &str, args: Vec<String>, start: Instant, cache_dir: PathBuf) {
    let current_dir = match env::current_dir() {
        Ok(dir) => dir,
        Err(_) => {
            eprintln!("Error: Unable to determine the current directory");
            return;
        }
    };

    match command {
        "add" => {
            create_required_dirs_and_files(&current_dir, &cache_dir).await;

            let package_names: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            if let Err(e) = add::add_and_install_packages(&package_names, Arc::new(current_dir), Arc::new(cache_dir)).await {
                eprintln!("Error: {}", e);
            }
        },
        "remove" => {
            if args.is_empty() {
                println!("Usage: qnpm remove <package_name, ...>");
                return;
            }

            if let Err(e) = remove::remove_package(&args[0], &current_dir) {
                eprintln!("Error: {}", e);
            }
        }
        "run" => {
            if args.is_empty() {
                println!("Usage: qnpm run <script_name>");
                return;
            }

            let package_json_path = current_dir.join("package.json");
            if !package_json_path.exists() {
                println!("package.json not found");
                return;
            }

            if let Err(e) = run_script(&package_json_path, &args[0]) {
                eprintln!("Error: {}", e);
            }
        }
        "init" => {
            if let Err(e) = init::initialize_node(&current_dir) {
                eprintln!("Error: {}", e);
            }
        },
        _ => println!("Command not found"),
    }
    
    let elapsed = start.elapsed().as_secs_f64();
    println!("Elapsed: {:.8} seconds", elapsed);
}

async fn create_required_dirs_and_files(current_dir: &PathBuf, cache_dir: &PathBuf) {
    let package_json_path = current_dir.join("package.json");
    let package_lock_json_path = current_dir.join("package-lock.json");
    let node_modules_path = current_dir.join("node_modules");
    let cache_node_modules_path = cache_dir.join("node_modules");

    if !package_json_path.exists() {
        init::create_bare_package_json(current_dir);
    }
    if !node_modules_path.exists() {
        std::fs::create_dir_all(node_modules_path).unwrap_or_else(|_| eprintln!("Error: Unable to create node_modules directory"));
    }
    if !cache_node_modules_path.exists() {
        std::fs::create_dir_all(cache_node_modules_path).unwrap_or_else(|_| eprintln!("Error: Unable to create cache node_modules directory"));
    }
    if !package_lock_json_path.exists() {
        init::create_bare_package_lock_json(current_dir);
    }
}