use async_recursion::async_recursion;
use flate2::read::GzDecoder;
use reqwest;
use serde_json::Value;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tar::Archive;
use tar::EntryType;
use std::io::Cursor;

use super::packageconfig::add_to_package_json;
use super::packageconfig::add_to_package_lock_json;
use super::errors::{DownloadError, AddCommandError};

async fn get_pkg_tarball_url(package_name: &str) -> Result<String, AddCommandError> {
    let url = format!("https://registry.npmjs.org/{}", package_name);
    let package_metadata = reqwest::get(&url)
        .await
        .map_err(|e| AddCommandError::FailedToRetrievePackageData(e))?
        .json::<Value>()
        .await
        .map_err(|e| AddCommandError::FailedToParsePackageMeta(e))?;
    if let Some(latest_version) = package_metadata["dist-tags"]["latest"].as_str() {
        if let Some(tarball_url) =
            package_metadata["versions"][latest_version]["dist"]["tarball"].as_str()
        {
            return Ok(tarball_url.to_string());
        }
    }
    Err(AddCommandError::NoValidTarballUrl(package_name.to_string()))
}

#[async_recursion]
pub async fn add_packages_with_dependencies(
    package_names: &[&str],
    current_dir: Arc<PathBuf>,
    cache_dir: Arc<PathBuf>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut tasks = Vec::new();

    for &package_name in package_names {
        let current_dir_clone = Arc::clone(&current_dir);
        let cache_dir_clone = Arc::clone(&cache_dir);
        let package_name_owned = package_name.to_owned(); // Cloning the package name

        let task = tokio::spawn(async move {
            let tarball_url = get_pkg_tarball_url(&package_name_owned).await?;
            let file_name = tarball_url.rsplit('/').next()
                .ok_or_else(|| DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Failed to extract file name from URL")))?;

            let package_name = file_name.splitn(2, ".tgz").next().ok_or_else(|| DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Invalid file name format")))?;
            let package_path = cache_dir_clone.join(format!("node_modules/{}", package_name));

            let local_package_path = current_dir_clone.join(format!("node_modules/{}", package_name));
            if local_package_path.exists() {
                return Ok::<(), Box<dyn Error + Send + Sync>>(());
            }

            if package_path.exists() {
                println!("Package {} already installed, using cache.", package_name_owned);
                folder_symlink(&current_dir_clone, &cache_dir_clone, package_name);
            } else {
                println!("Downloading package {}", package_name_owned);
                download_and_extract_with_reqwest(&tarball_url, &current_dir_clone, &cache_dir_clone).await?;
            }
            
            add_to_package_json(&package_name_owned, &current_dir_clone);

            let segments: Vec<&str> = file_name.split('-').collect();
            let version_with_extension = segments.last().ok_or_else(|| DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Failed to extract version and file extension from URL")))?;
            let package_version = version_with_extension.trim_end_matches(".tgz");
            add_to_package_lock_json(&package_name_owned, &current_dir_clone, &package_version, &tarball_url, serde_json::json!({})).await?;

            install_package_dependencies(&package_name_owned, &tarball_url, &current_dir_clone, &cache_dir_clone).await?;
            Ok(())
        });

        tasks.push(task);
    }

    for task in tasks {
        task.await??;
    }

    Ok(())
}


#[async_recursion]
pub async fn install_package_dependencies(
    package_name: &str,
    url: &str,
    current_dir: &Arc<PathBuf>,
    cache_dir: &Arc<PathBuf>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let file_name = url.rsplit('/').next()
        .ok_or_else(|| DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Failed to extract file name from URL")))?;
    let downloaded_package_name = file_name.splitn(2, ".tgz").next().ok_or_else(|| DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Invalid file name format")))?;

    let package_json_path = cache_dir
        .join(format!("node_modules/{}/package.json", downloaded_package_name))
        .into_os_string()
        .into_string()
        .unwrap()
        .replace("\\", "/");

    let package_json = match std::fs::read_to_string(Path::new(&package_json_path)) {
        Ok(content) => content,
        Err(_) => {
            println!("package.json not found for package {}", package_name);
            return Ok(())
        }
    };
    let package: Value = serde_json::from_str(&package_json)?;

    if let Some(dependencies) = package["dependencies"].as_object() {
        let dep_names: Vec<&str> = dependencies.keys().map(AsRef::as_ref).collect();
        add_packages_with_dependencies(&dep_names, Arc::clone(current_dir), Arc::clone(cache_dir)).await?;
    }

    Ok(())
}


pub async fn download_and_extract_with_reqwest(
    url: &str,
    current_dir: &Arc<PathBuf>,
    cache_dir: &Arc<PathBuf>,
) -> Result<(), DownloadError> {
    let response = reqwest::get(url).await?.bytes().await?;
    let cursor = Cursor::new(response);
    let tar_gz = GzDecoder::new(cursor);
    let mut archive = Archive::new(tar_gz);

    let url_split: Vec<&str> = url.split('/').collect();
    let file_name = url_split
        .last()
        .ok_or(DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Failed to extract file name from URL")))?;

    let package_name = file_name.split(".tgz").next().ok_or(DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Invalid file name format")))?;
    let package_path = cache_dir.join("node_modules/".to_owned()+package_name);
    fs::create_dir_all(&package_path)?;

    for file in archive.entries()? {
        let mut file = file?;
        if file.header().entry_type() != EntryType::Regular
            && file.header().entry_type() != EntryType::Directory
        {
            continue;
        }
        let path: std::borrow::Cow<'_, Path> = file.path()?;
        let mut components: std::path::Components<'_> = path.components();
        components.next();
        let new_path = package_path.join(components.as_path());
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if file.header().entry_type() == EntryType::Directory {
            fs::create_dir_all(&new_path)?;
        } else {
            file.unpack(new_path)?;
        }
    }

    folder_symlink(current_dir, cache_dir, package_name);
    
    Ok(())
}

use std::os::windows::fs::symlink_dir;
pub fn folder_symlink(current_dir:&PathBuf, cache_dir:&PathBuf, downloadedpackagename:&str) {
    symlink_dir(cache_dir.join("node_modules").join(downloadedpackagename), current_dir.join("node_modules").join(downloadedpackagename));
}