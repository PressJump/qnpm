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
pub async fn add_and_install_packages(
    package_names: &[&str],
    current_dir: Arc<PathBuf>,
    cache_dir: Arc<PathBuf>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    for &package_name in package_names {
        let current_dir_clone = Arc::clone(&current_dir);
        let cache_dir_clone = Arc::clone(&cache_dir);
        let package_name_owned = package_name.to_owned();
        let tarball_url = get_pkg_tarball_url(&package_name_owned).await?;
        let file_name = tarball_url.rsplit('/').next().ok_or_else(|| DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Failed to extract file name")))?;
        let (package, version) = file_name.splitn(2, ".tgz").next().zip(file_name.split('-').last().map(|v| v.trim_end_matches(".tgz"))).ok_or_else(|| DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Invalid file format")))?;
        let local_package_path = current_dir_clone.join(format!("node_modules/{}", package));
        if !local_package_path.exists() {
            if cache_dir_clone.join(format!("node_modules/{}", package)).exists() {
                println!("Using cached {}", package_name_owned);
                folder_symlink(&current_dir_clone, &cache_dir_clone, package);
            } else {
                println!("Downloading {}", package_name_owned);
                download_and_extract_with_reqwest(&tarball_url, &current_dir_clone, &cache_dir_clone).await?;
            }
            add_to_package_json(&package_name_owned, &current_dir_clone);
            add_to_package_lock_json(&package_name_owned, &current_dir_clone, version, &tarball_url, serde_json::json!({})).await?;
            let pkg_json_path = cache_dir_clone.join(format!("node_modules/{}/package.json", package)).into_os_string().into_string().unwrap().replace("\\", "/");
            if let Ok(content) = std::fs::read_to_string(Path::new(&pkg_json_path)) {
                if let Ok(package) = serde_json::from_str::<Value>(&content) {
                    if let Some(deps) = package["dependencies"].as_object() {
                        add_and_install_packages(&deps.keys().map(AsRef::as_ref).collect::<Vec<&str>>(), Arc::clone(&current_dir_clone), Arc::clone(&cache_dir_clone)).await?;
                    }
                }
            }
        }
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