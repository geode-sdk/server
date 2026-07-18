use super::*;
use ::s3::{Bucket, Region, creds::Credentials};

pub enum S3Provider {
    Aws,
    Cloudflare,
}

impl S3Provider {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "aws" => Some(S3Provider::Aws),
            "cloudflare" => Some(S3Provider::Cloudflare),
            _ => None,
        }
    }
}

pub struct S3Configuration {
    pub provider: S3Provider,
    pub bucket: String,
    pub region: Option<String>,
    pub access_key: String,
    pub secret_key: String,
    pub account_id: Option<String>,
    pub public_url: String,
}

impl S3Configuration {
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let provider = dotenvy::var("STORAGE_PROVIDER").unwrap_or_default();
        if provider.is_empty() {
            return Ok(None);
        }

        let provider = S3Provider::from_str(&provider).ok_or_else(|| {
            anyhow::anyhow!("Invalid STORAGE_PROVIDER value, must be 'aws' or 'cloudflare'")
        })?;

        let config = S3Configuration {
            provider,
            bucket: dotenvy::var("STORAGE_BUCKET")?,
            region: dotenvy::var("STORAGE_REGION").ok(),
            access_key: dotenvy::var("STORAGE_ACCESS_KEY")?,
            secret_key: dotenvy::var("STORAGE_SECRET_KEY")?,
            account_id: dotenvy::var("STORAGE_ACCOUNT_ID").ok(),
            public_url: dotenvy::var("STORAGE_PUBLIC_URL")?,
        };

        Ok(Some(config))
    }
}

pub struct S3Backend {
    bucket: Box<Bucket>,
}

impl S3Backend {
    pub fn new(config: &S3Configuration) -> anyhow::Result<S3Backend> {
        match config.provider {
            S3Provider::Aws => S3Backend::new_aws(config),
            S3Provider::Cloudflare => S3Backend::new_cloudflare(config),
        }
    }

    fn new_aws(config: &S3Configuration) -> anyhow::Result<S3Backend> {
        let Some(region) = &config.region else {
            return Err(anyhow::anyhow!("STORAGE_REGION is required for AWS S3"));
        };

        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )?;

        let bucket = Bucket::new(&config.bucket, region.parse()?, credentials)?;

        Ok(S3Backend { bucket })
    }

    fn new_cloudflare(config: &S3Configuration) -> anyhow::Result<S3Backend> {
        let Some(account_id) = config.account_id.clone() else {
            return Err(anyhow::anyhow!(
                "STORAGE_ACCOUNT_ID is required for Cloudflare R2"
            ));
        };

        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )?;

        let bucket = Bucket::new(&config.bucket, Region::R2 { account_id }, credentials)?;

        Ok(S3Backend { bucket })
    }
}

impl StorageBackend for S3Backend {
    fn store<'a>(
        &'a self,
        relative_path: &'a str,
        data: &'a [u8],
    ) -> BoxFuture<'a, StorageResult<()>> {
        Box::pin(async move {
            let _ = self.bucket.put_object(relative_path, data).await?;
            Ok(())
        })
    }

    fn read<'a>(&'a self, path: &'a str) -> BoxFuture<'a, StorageResult<Vec<u8>>> {
        Box::pin(async move {
            let data = self.bucket.get_object(path).await?;

            if data.status_code() != 200 {
                return Err(StorageError::Other(format!(
                    "S3 API returned HTTP error {}",
                    data.status_code()
                )));
            }

            Ok(data.to_vec())
        })
    }

    fn exists<'a>(&'a self, path: &'a str) -> BoxFuture<'a, StorageResult<bool>> {
        Box::pin(async move { Ok(self.bucket.object_exists(path).await?) })
    }

    fn delete<'a>(&'a self, path: &'a str) -> BoxFuture<'a, StorageResult<()>> {
        Box::pin(async move {
            let _ = self.bucket.delete_object(path).await?;
            Ok(())
        })
    }
}
