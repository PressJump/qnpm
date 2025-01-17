use std::error::Error;
use std::path::PathBuf;
use std::process::Command;


pub fn remove(package_name: &str, current_dir: &PathBuf) -> Result<(), Box<dyn Error + Send + Sync>> {
    let node_modules = current_dir.join("node_modules");
    let package_dir = node_modules.join(package_name);
    if !package_dir.exists() {
        println!("Package not in node_modules, nothing to remove");
        return Ok(());
    }
    std::fs::remove_dir_all(package_dir)?;
    Ok(())
}