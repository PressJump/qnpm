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
use thiserror::Error;
use reqwest::Error as ReqwestError;
use std::io::Cursor;
use tokio::task;

#[derive(Debug, Error)]
pub enum AddCommandError {
    #[error("Failed to parse JSON: {0}")]
    FailedToParsePackageMeta(reqwest::Error),
    #[error("Failed to retrieve package data: {0}")]
    FailedToRetrievePackageData(reqwest::Error),
    #[error("No valid tarball url for package '{0}'")]
    NoValidTarballUrl(String),
    #[error("Failed to extract file name from URL")]
    FailedToExtractFileName,
    #[error("Failed to spawn aria2c process: {0}")]
    FailedToSpawnAria2c(std::io::Error),
    #[error("Failed to wait for aria2c process: {0}")]
    FailedToWaitForAria2c(std::io::Error),
    #[error("Failed to open file: {0}")]
    FailedToOpenFile(std::io::Error),
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("Failed to download file: {0}")]
    DownloadFailed(ReqwestError),
    #[error("Failed to extract file: {0}")]
    ExtractionFailed(std::io::Error),
    #[error("Failed to create directories: {0}")]
    DirectoryCreationFailed(std::io::Error),
}

impl From<std::io::Error> for AddCommandError {
    fn from(err: std::io::Error) -> AddCommandError {
        AddCommandError::FailedToOpenFile(err)
    }
}

impl From<ReqwestError> for DownloadError {
    fn from(err: ReqwestError) -> Self {
        DownloadError::DownloadFailed(err)
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(err: std::io::Error) -> Self {
        DownloadError::ExtractionFailed(err)
    }
}

#[derive(Clone)]
pub struct Package {
    name: String,
    tarball_url: String,
    version: String,
}

async fn get_pkg_details(package_name: &str) -> Result<Package, AddCommandError> {
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
            return Ok(Package {
                name: package_name.to_string(),
                tarball_url: tarball_url.to_string(),
                version: latest_version.to_string(),
            });
        }
    }
    Err(AddCommandError::NoValidTarballUrl(package_name.to_string()))
}

async fn get_pkg_details_with_version(package_name: &str, version: &str) -> Result<Package, AddCommandError> {
    let url = format!("https://registry.npmjs.org/{}/{}", package_name, version);
    let package_metadata = reqwest::get(&url)
        .await
        .map_err(|e| AddCommandError::FailedToRetrievePackageData(e))?
        .json::<Value>()
        .await
        .map_err(|e| AddCommandError::FailedToParsePackageMeta(e))?;
    if let Some(tarball_url) = package_metadata["dist"]["tarball"].as_str() {
        return Ok(Package {
            name: package_name.to_string(),
            tarball_url: tarball_url.to_string(),
            version: version.to_string(),
        });
    }
    Err(AddCommandError::NoValidTarballUrl(package_name.to_string()))
}

fn add_to_package_json(package: Package, current_dir: &PathBuf) {
    let package_json_path = current_dir.join("package.json");
    let mut package_json = std::fs::read_to_string(&package_json_path).unwrap();
    let package_json_value: Value = serde_json::from_str(&package_json).unwrap();
    let mut package_json_object = package_json_value
        .as_object()
        .unwrap()
        .clone();
    let dependencies = package_json_object
        .entry("dependencies")
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if let Some(dep_object) = dependencies.as_object_mut() {
        dep_object.insert(
            package.name,
            serde_json::Value::String(package.version),
        );
    }
    let updated_json = serde_json::to_string_pretty(&package_json_object).unwrap();
    std::fs::write(package_json_path, updated_json).unwrap();
}

#[async_recursion]
pub async fn add_packages_with_dependencies_from_names(
    package_names: &[String],
    current_dir: Arc<PathBuf>,
    cache_dir: Arc<PathBuf>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut packages = Vec::new();
    for package_name in package_names {
        //Get version if specified
        let package = if package_name.contains("@") {
            let package_split: Vec<&str> = package_name.split('@').collect();
            //if @latest is specified, get latest version
            if package_split[1] == "latest" {
                get_pkg_details(package_split[0]).await?
            } else {
                get_pkg_details_with_version(package_split[0], package_split[1]).await?
            }
        } else {
            get_pkg_details(package_name).await?
        };
        packages.push(package);
    }
    add_packages_with_dependencies(&packages, current_dir, cache_dir).await
}

#[async_recursion]
pub async fn add_packages_with_dependencies(
    packages: &[Package],
    current_dir: Arc<PathBuf>,
    cache_dir: Arc<PathBuf>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut tasks = Vec::new();

    for package in packages {
        let current_dir_clone = Arc::clone(&current_dir);
        let cache_dir_clone = Arc::clone(&cache_dir);
        let package_clone = Package {
            name: package.name.clone(),
            tarball_url: package.tarball_url.clone(),
            version: package.version.clone(),
        };

        let task = tokio::spawn(async move {
            let tarball_url = package_clone.tarball_url.clone();
            let file_name = tarball_url.rsplit('/').next()
                .ok_or_else(|| DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Failed to extract file name from URL")))?;

            let package_name = file_name.splitn(2, ".tgz").next()
                .ok_or_else(|| DownloadError::ExtractionFailed(std::io::Error::new(std::io::ErrorKind::Other, "Invalid file name format")))?;
            let package_path = cache_dir_clone.join(format!("node_modules/{}", package_name));

            let local_package_path = current_dir_clone.join(format!("node_modules/{}", package_name));
            if local_package_path.exists() {
                return Ok::<(), Box<dyn Error + Send + Sync>>(());
            }

            add_to_package_json(package_clone.clone(), &current_dir_clone);
            if package_path.exists() {
                println!("Package {} already installed, using cache.", format!("{}@{}", package_clone.name, package_clone.version));
                folder_symlink(&current_dir_clone, &cache_dir_clone, package_name);
            } else {
                println!("Downloading package {}", format!("{}@{}", package_clone.name, package_clone.version));
                download_and_extract_with_reqwest(&tarball_url, &current_dir_clone, &cache_dir_clone).await?;
            }
            install_package_dependencies(&package_clone.name, &tarball_url, &current_dir_clone, &cache_dir_clone).await?;
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
        let mut dep_packages = Vec::new();
        for (name, version) in dependencies.iter() {
            let version_str = version.as_str().unwrap();
            let package_detail = if version_str.starts_with("^") {
                get_pkg_details(name).await?
            } else {
                get_pkg_details_with_version(name, version_str).await?
            };
            dep_packages.push(package_detail);
        }
        add_packages_with_dependencies(&dep_packages, Arc::clone(current_dir), Arc::clone(cache_dir)).await?;
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
pub fn folder_symlink(current_dir: &PathBuf, cache_dir: &PathBuf, downloadedpackagename: &str) {
    let local_package_name = if let Some(pos) = downloadedpackagename.rfind('-') {
        downloadedpackagename[..pos].to_string()
    } else {
        downloadedpackagename.to_string()
    };
    // if symlink already exists remove it / replace it 
    // could be a better way to do this in the future for optimization
    // but for now this way we can ensure that the symlink is always up to date
    // and we don't have to worry about checking if it exists or not
    let _ = std::fs::remove_dir(current_dir.join("node_modules").join(&local_package_name));
    symlink_dir(
        cache_dir.join("node_modules").join(downloadedpackagename),
        current_dir.join("node_modules").join(&local_package_name),
    ).unwrap_or_else(|e| eprintln!("Failed to create symlink for {}: {}", downloadedpackagename, e));
}
