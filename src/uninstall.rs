use std::error::Error;
use std::path::PathBuf;
use std::process::Command;


pub fn uninstall(package_name: &str, current_dir: &PathBuf, cache_dir: &PathBuf) -> Result<(), Box<dyn Error + Send + Sync>> {
    let node_modules = current_dir.join("node_modules");
    let package_dir = node_modules.join(package_name);
    if package_dir.exists() {
        std::fs::remove_dir_all(package_dir)?;
    }
    //get package version from package.json
    let package_json = current_dir.join("package.json");
    let package_json_str = std::fs::read_to_string(&package_json)?;
    let package_json_value: serde_json::Value = serde_json::from_str(&package_json_str)?;
    let dependencies = package_json_value["dependencies"].as_object().unwrap();
    //if package in package.json get version and remove from cache
    if dependencies.contains_key(package_name) {
        let package_version = dependencies[package_name].as_str().unwrap();
        let package_cache_dir = cache_dir.join(package_name).join(package_version);
        if package_cache_dir.exists() {
            std::fs::remove_dir_all(package_cache_dir)?;
        }
    }
    else
    {
        //Find folders, then check their names to see if it contains the package name
        let cache_dir_contents = std::fs::read_dir(cache_dir)?;
        for entry in cache_dir_contents {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let entry_name = entry_path.file_name().unwrap().to_str().unwrap();
                if entry_name.contains(package_name) {
                    std::fs::remove_dir_all(entry_path)?;
                }
            }
        }
    }

    remove_from_package_json(package_name, current_dir)?;
    Ok(())
}

fn remove_from_package_json(package_name: &str, current_dir: &PathBuf) -> Result<(), Box<dyn Error + Send + Sync>> {
    let package_json = current_dir.join("package.json");
    let package_json_str = std::fs::read_to_string(&package_json)?;
    let package_json_value: serde_json::Value = serde_json::from_str(&package_json_str)?;
    let dependencies = package_json_value["dependencies"].as_object().unwrap();
    if dependencies.contains_key(package_name) {
        let mut package_json_value = package_json_value.clone();
        let dependencies = package_json_value["dependencies"].as_object_mut().unwrap();
        dependencies.remove(package_name);
        let package_json_str = serde_json::to_string_pretty(&package_json_value)?;
        std::fs::write(&package_json, package_json_str)?;
    }
    else
    {
        println!("Package {} is not installed", package_name);
    }
    Ok(())
}