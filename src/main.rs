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

    // Directly jump to match if command is neither 'config' nor 'add'
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
    match command {
        "add" => {
            let current_dir: PathBuf = env::current_dir().unwrap();
            // Extract package names from args if command is 'add'
            let package_names: Vec<String> = args.iter().cloned().collect();

            // if package.json doesn't exist, create it
            if !Path::new("package.json").exists() {
                init::create_bare_package_json(&current_dir);
            }

            if !Path::new("node_modules").exists() {
                std::fs::create_dir_all("node_modules").unwrap();
            }

            // Make sure cache_dir also has node_modules
            if !Path::new(&cache_dir.join("node_modules")).exists() {
                std::fs::create_dir_all(&cache_dir.join("node_modules")).unwrap();
            }

            // Wrap params in Arc and call the new function
            if let Err(e) = add::add_packages_with_dependencies_from_names(
                &package_names,
                Arc::new(current_dir),
                Arc::new(cache_dir),
            )
            .await
            {
                eprintln!("Error adding packages: {}", e);
            }
        },
        "remove" => 
        {
            let current_dir: PathBuf = env::current_dir().unwrap();

            if !Path::new("package.json").exists() {
                println!("package.json not found");
                return;
            }

            if args.is_empty() {
                println!("Usage: qnpm remove <package_name>");
                return;
            }

            //for loop package names and remove them
            for package_name in args {
                if let Err(e) = remove::remove(&package_name, &current_dir) {
                    eprintln!("Error removing package: {}", e);
                }
            }
        }
        "run" => {
            let current_dir: PathBuf = env::current_dir().unwrap();
            if args.is_empty() {
                println!("Usage: qnpm run <script_name>");
                return;
            }
            if !current_dir.join("package.json").exists() {
                println!("package.json not found");
                return;
            }
            let package_json_path: PathBuf = current_dir.join("package.json");
            run_script(&package_json_path, &args[0]).unwrap();
        }
        "init" => {
            let current_dir: PathBuf = env::current_dir().unwrap();
            init::initialize_node(&current_dir);
        },
        _ => println!("Command not found")
    }
    let elapsed = start.elapsed().as_secs_f64();
    println!("Elapsed: {:.8?}", elapsed);
}
