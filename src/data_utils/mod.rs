pub mod data_utils_s3;

use aws_sdk_s3::config::Region;
use data_utils_s3::AwsConfig;
use data_utils_s3::AwsS3Service;

pub enum StorageService {
    AwsS3(AwsS3Service),
}

impl StorageService {
    pub async fn new_aws_s3(conf: AwsConfig) -> Result<Self, aws_sdk_s3::Error> {
        let aws_s3_service = AwsS3Service::new(conf).await?;

        Ok(StorageService::AwsS3(aws_s3_service))
    }

    pub async fn upload_file(&self, local_path: &str, remote_path: &str) -> bool {
        match self {
            StorageService::AwsS3(service) => service.upload_file(local_path, remote_path).await,
        }
    }

    pub async fn download_file(&self, local_path: &str, remote_path: &str) -> bool {
        match self {
            StorageService::AwsS3(service) => service.download_file(local_path, remote_path).await,
        }
    }

    pub async fn file_exists(&self, remote_path: &str) -> bool {
        match self {
            StorageService::AwsS3(service) => service.file_exists(remote_path).await,
        }
    }

    pub async fn delete_file(&self, remote_path: &str) -> bool {
        match self {
            StorageService::AwsS3(service) => service.delete_file(remote_path).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use figment::{
        providers::{Format, Toml},
        Figment,
    };
    use rstest::{fixture, rstest};
    use serde::Deserialize;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[derive(Debug, Deserialize)]
    struct Config {
        aws_s3: AwsConfig,
    }

    #[fixture]
    async fn s3_store() -> StorageService {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .init();

        let conf = Figment::new()
            .merge(Toml::file("config.toml"))
            .extract::<Config>()
            .expect("Failed to load config")
            .aws_s3;

        StorageService::new_aws_s3(conf)
            .await
            .expect("Failed to initialize AWS S3 storage service")
    }

    #[rstest]
    #[tokio::test]
    async fn test_s3(#[future] s3_store: StorageService) {
        let storage_service = s3_store.await;

        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(temp_file, "This is a test file").expect("Failed to write to temp file");

        let temp_file_path = temp_file.path().to_str().unwrap();
        let remote_file_path = "test_file_combined.txt";

        let upload_result = storage_service
            .upload_file(temp_file_path, remote_file_path)
            .await;
        assert!(upload_result, "File upload should succeed");

        let exists = storage_service.file_exists(remote_file_path).await;
        assert!(exists, "File should exist in the bucket after upload");

        let temp_download_path = NamedTempFile::new().expect("Failed to create temp download file");
        let download_result = storage_service
            .download_file(
                temp_download_path.path().to_str().unwrap(),
                remote_file_path,
            )
            .await;

        assert!(download_result, "File download should succeed");

        let delete_result = storage_service.delete_file(remote_file_path).await;
        assert!(delete_result, "File deletion should succeed");

        let exists_after_delete = storage_service.file_exists(remote_file_path).await;
        assert!(
            !exists_after_delete,
            "File should no longer exist in the bucket after deletion"
        );
    }
}
