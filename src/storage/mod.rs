use std::{pin::Pin, sync::Arc};

pub use local::LocalBackend;
pub use s3::*;

mod local;
mod s3;

#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("S3 error: {0}")]
    S3(#[from] ::s3::error::S3Error),
    #[error("{0}")]
    Other(String),
}

pub type StorageResult<T> = Result<T, StorageError>;
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait StorageBackend: Send + Sync {
    fn init(&self) -> BoxFuture<'_, StorageResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn store<'a>(&'a self, path: &'a str, data: &'a [u8]) -> BoxFuture<'a, StorageResult<()>>;
    fn read<'a>(&'a self, path: &'a str) -> BoxFuture<'a, StorageResult<Vec<u8>>>;
    #[allow(dead_code)]
    fn exists<'a>(&'a self, path: &'a str) -> BoxFuture<'a, StorageResult<bool>>;
    fn delete<'a>(&'a self, path: &'a str) -> BoxFuture<'a, StorageResult<()>>;
}

#[derive(Clone)]
struct DiskCore {
    backend: Arc<dyn StorageBackend>,
}

impl DiskCore {
    pub async fn init(&self) -> StorageResult<()> {
        self.backend.init().await
    }
    pub async fn store_hashed(
        &self,
        relative_path: &str,
        data: &[u8],
        extension: Option<&str>,
    ) -> StorageResult<String> {
        let hash = sha256::digest(data);

        let hashed_path = format!(
            "{}/{}/{}{}",
            relative_path,
            &hash[0..2],
            hash,
            extension.map_or("".to_string(), |ext| format!(
                ".{}",
                ext.trim_start_matches('.')
            ))
        );
        self.store(&hashed_path, data).await?;
        Ok(hashed_path)
    }
    pub async fn store(&self, path: &str, data: &[u8]) -> StorageResult<()> {
        self.backend.store(path, data).await
    }
    pub async fn read(&self, path: &str) -> StorageResult<Vec<u8>> {
        self.backend.read(path).await
    }
    pub async fn exists(&self, path: &str) -> StorageResult<bool> {
        self.backend.exists(path).await
    }
    pub async fn delete(&self, path: &str) -> StorageResult<()> {
        self.backend.delete(path).await
    }
}

#[derive(Clone)]
pub struct PublicDisk {
    core: DiskCore,
    public_url: String,
}

impl PublicDisk {
    pub fn new(backend: Arc<dyn StorageBackend>, public_url: String) -> PublicDisk {
        PublicDisk {
            core: DiskCore { backend },
            public_url,
        }
    }
    pub fn asset_url(&self, path: &str) -> String {
        format!("{}/{}", self.public_url, path)
    }
    pub async fn init(&self) -> StorageResult<()> {
        self.core.init().await
    }
    pub async fn store_hashed(
        &self,
        relative_path: &str,
        data: &[u8],
        extension: Option<&str>,
    ) -> StorageResult<String> {
        self.core.store_hashed(relative_path, data, extension).await
    }
    pub async fn store(&self, path: &str, data: &[u8]) -> StorageResult<()> {
        self.core.store(path, data).await
    }
    pub async fn read(&self, path: &str) -> StorageResult<Vec<u8>> {
        self.core.read(path).await
    }
    #[allow(dead_code)]
    pub async fn exists(&self, path: &str) -> StorageResult<bool> {
        self.core.exists(path).await
    }
    pub async fn delete(&self, path: &str) -> StorageResult<()> {
        self.core.delete(path).await
    }
}

#[derive(Clone)]
pub struct PrivateDisk {
    core: DiskCore,
}

impl PrivateDisk {
    pub fn new(backend: Arc<dyn StorageBackend>) -> PrivateDisk {
        PrivateDisk {
            core: DiskCore { backend },
        }
    }
    pub async fn init(&self) -> StorageResult<()> {
        self.core.init().await
    }
    #[allow(dead_code)]
    pub async fn store_hashed(
        &self,
        relative_path: &str,
        data: &[u8],
        extension: Option<&str>,
    ) -> StorageResult<String> {
        self.core.store_hashed(relative_path, data, extension).await
    }
    #[allow(dead_code)]
    pub async fn store(&self, path: &str, data: &[u8]) -> StorageResult<()> {
        self.core.store(path, data).await
    }
    #[allow(dead_code)]
    pub async fn read(&self, path: &str) -> StorageResult<Vec<u8>> {
        self.core.read(path).await
    }
    #[allow(dead_code)]
    pub async fn exists(&self, path: &str) -> StorageResult<bool> {
        self.core.exists(path).await
    }
    #[allow(dead_code)]
    pub async fn delete(&self, path: &str) -> StorageResult<()> {
        self.core.delete(path).await
    }
}
