use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct StaticStorage {
    base_path: PathBuf,
}

impl StaticStorage {
    pub fn new() -> Self {
        Self {
            base_path: PathBuf::from("storage/static"),
        }
    }
}

impl StorageDisk for StaticStorage {
    fn base_path(&self) -> &Path {
        &self.base_path
    }
}

#[derive(Clone, Debug)]
pub struct PrivateStorage {
    base_path: PathBuf,
}

impl PrivateStorage {
    pub fn new() -> Self {
        Self {
            base_path: PathBuf::from("storage/private"),
        }
    }
}

impl StorageDisk for PrivateStorage {
    fn base_path(&self) -> &Path {
        &self.base_path
    }
}

pub trait StorageDisk {
    async fn init(&self) -> std::io::Result<()> {
        tokio::fs::create_dir_all(self.base_path()).await?;
        Ok(())
    }
    fn base_path(&self) -> &Path;
    fn path(&self, relative_path: &str) -> PathBuf {
        self.base_path().join(relative_path)
    }
    async fn store(&self, relative_path: &str, data: &[u8]) -> std::io::Result<()> {
        let path = self.path(relative_path);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(path, data).await
    }
    /// Store data at a path calculated from the hash of the data. Uses content-addressable storage with 2 levels
    async fn store_hashed(&self, relative_path: &str, data: &[u8]) -> std::io::Result<String> {
        self.store_hashed_with_extension(relative_path, data, None)
            .await
    }
    /// Store data at a path calculated from the hash of the data. Uses content-addressable storage with 2 levels.
    /// Extension should not include the dot, and will be added to the end of the filename if provided.
    async fn store_hashed_with_extension(
        &self,
        relative_path: &str,
        data: &[u8],
        extension: Option<&str>,
    ) -> std::io::Result<String> {
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
    async fn read(&self, relative_path: &str) -> std::io::Result<Vec<u8>> {
        match tokio::fs::read(self.path(relative_path)).await {
            Ok(data) => Ok(data),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(e) => Err(e),
        }
    }
    async fn read_stream(&self, relative_path: &str) -> std::io::Result<tokio::fs::File> {
        tokio::fs::File::open(self.path(relative_path)).await
    }
    async fn exists(&self, relative_path: &str) -> std::io::Result<bool> {
        let path = self.path(relative_path);
        Ok(tokio::fs::metadata(path).await.is_ok())
    }
    async fn delete(&self, relative_path: &str) -> std::io::Result<()> {
        match tokio::fs::remove_file(self.path(relative_path)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}
