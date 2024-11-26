use crate::prelude::*;
use aws_config::{meta::region::RegionProviderChain, BehaviorVersion};
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::{
    config::{Credentials, Region},
    error::SdkError,
    primitives::ByteStream,
    Client, Error,
};
use serde::Deserialize;
use std::path::Path;
use tokio;

#[derive(Debug, Deserialize)]
pub struct AwsConfig {
    pub bucket: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

pub struct AwsS3Service {
    client: Client,
    bucket: String,
}

impl AwsS3Service {
    pub async fn new(config: AwsConfig) -> Result<Self, Error> {
        debug!("Config: {:?}", config);
        let region = Region::new(config.region.clone());

        debug!("Region: {:?}", region.clone());

        let shared_config = aws_config::defaults(BehaviorVersion::v2024_03_28())
            .region(region)
            .credentials_provider(Credentials::new(
                config.access_key_id.clone(),
                config.secret_access_key.clone(),
                None,
                None,
                "custom_provider",
            ))
            .load()
            .await;

        let client = Client::new(&shared_config);
        Ok(Self {
            client,
            bucket: config.bucket,
        })
    }

    pub async fn upload_file(&self, local_path: &str, remote_path: &str) -> bool {
        let file_path = Path::new(local_path);
        if !file_path.exists() {
            info!("The file '{}' was not found.", local_path);
            return false;
        }

        let byte_stream = match ByteStream::from_path(file_path).await {
            Ok(stream) => stream,
            Err(e) => {
                error!(
                    "Failed to create ByteStream for file '{}': {:?}",
                    local_path, e
                );
                return false;
            }
        };

        match self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(remote_path)
            .body(byte_stream)
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    "File '{}' has been uploaded to bucket '{}' as '{}'.",
                    local_path, self.bucket, remote_path
                );
                true
            }
            Err(e) => {
                error!("Failed to upload file: {:?}", e);
                false
            }
        }
    }

    pub async fn download_file(&self, local_path: &str, remote_path: &str) -> bool {
        match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(remote_path)
            .send()
            .await
        {
            Ok(output) => {
                let body = output.body.collect().await.unwrap().into_bytes();
                if let Err(e) = tokio::fs::write(local_path, &body).await {
                    error!("Failed to write file '{}': {}", local_path, e);
                    return false;
                }
                info!(
                    "File '{}' downloaded from S3 bucket '{}' under '{}'.",
                    local_path, self.bucket, remote_path
                );
                true
            }
            Err(e) => {
                error!("Error downloading file '{}': {:?}", remote_path, e);
                false
            }
        }
    }

    pub async fn file_exists(&self, remote_path: &str) -> bool {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(remote_path)
            .send()
            .await
        {
            Ok(_) => {
                info!("File '{}' exists in bucket '{}'.", remote_path, self.bucket);
                true
            }
            Err(SdkError::ServiceError(err)) => {
                if err.err().code() == Some("NotFound") {
                    info!(
                        "File '{}' does not exist in bucket '{}'.",
                        remote_path, self.bucket
                    );
                    false
                } else {
                    error!(
                        "Service error while checking file existence '{}': {:?}",
                        remote_path, err
                    );
                    false
                }
            }
            Err(e) => {
                error!("Error checking file existence '{}': {:?}", remote_path, e);
                false
            }
        }
    }

    pub async fn delete_file(&self, remote_path: &str) -> bool {
        match self
            .client
            .delete_object()
            .bucket(&self.bucket)
            .key(remote_path)
            .send()
            .await
        {
            Ok(_) => {
                info!(
                    "File '{}' has been deleted from bucket '{}'.",
                    remote_path, self.bucket
                );
                true
            }
            Err(e) => {
                error!("Failed to delete file '{}': {:?}", remote_path, e);
                false
            }
        }
    }
}
