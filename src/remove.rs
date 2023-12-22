use std::error::Error;
use std::path::PathBuf;
use std::fs;
use super::packageconfig::check_package_status;
use super::packageconfig::PackageStatus;
use super::packageconfig::remove_from_json;

pub fn remove_package(
    package_name: &str, 
    current_dir: &PathBuf
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match check_package_status(package_name, "*", current_dir)? {
        PackageStatus::Installed | PackageStatus::Outdated | PackageStatus::FoundInNodeModules => {
            remove_from_json(&current_dir.join("package.json"), package_name)?;
            remove_from_json(&current_dir.join("package-lock.json"), package_name)?;
            remove_from_node_modules(package_name, current_dir)?;
            Ok(())
        },
        _ => Ok(()),
    }
}

pub fn remove_from_node_modules(
    package_name: &str, 
    current_dir: &PathBuf
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let node_modules_path = current_dir.join("node_modules").join(package_name);
    if node_modules_path.exists() {
        fs::remove_dir_all(node_modules_path)?;
    }
    Ok(())
}