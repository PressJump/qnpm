use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

pub async fn remove_packages_from_local_modules(
    package_names: &[&str],
    current_dir: Arc<PathBuf>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    //get folder 
}