use thiserror::Error;
use reqwest::Error as ReqwestError;

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