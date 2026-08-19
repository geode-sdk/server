use super::*;
use std::path::PathBuf;

pub struct LocalBackend {
    base_path: PathBuf,
}

impl LocalBackend {
    pub fn new(base_path: impl Into<PathBuf>) -> LocalBackend {
        LocalBackend {
            base_path: base_path.into(),
        }
    }

    fn safe_join(&self, suffix: &str) -> Result<PathBuf, StorageError> {
        let path = self.base_path.join(suffix);
        if path.starts_with(&self.base_path) {
            Ok(path)
        } else {
            Err(StorageError::Other(
                "Attempted accessing a path outside storage directory".to_owned(),
            ))
        }
    }
}

impl StorageBackend for LocalBackend {
    fn store<'a>(
        &'a self,
        relative_path: &'a str,
        data: &'a [u8],
        _mime_type: &'a str,
    ) -> BoxFuture<'a, StorageResult<()>> {
        Box::pin(async move {
            let path = self.safe_join(relative_path)?;
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            tokio::fs::write(path, data).await.map_err(|e| e.into())
        })
    }

    fn read<'a>(&'a self, path: &'a str) -> BoxFuture<'a, StorageResult<Vec<u8>>> {
        Box::pin(async move {
            let path = self.safe_join(path)?;
            match tokio::fs::read(path).await {
                Ok(data) => Ok(data),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
                Err(e) => Err(e),
            }
            .map_err(|e| e.into())
        })
    }

    fn exists<'a>(&'a self, path: &'a str) -> BoxFuture<'a, StorageResult<bool>> {
        Box::pin(async move {
            let path = self.safe_join(path)?;
            Ok(tokio::fs::metadata(path).await.is_ok())
        })
    }

    fn delete<'a>(&'a self, path: &'a str) -> BoxFuture<'a, StorageResult<()>> {
        Box::pin(async move {
            let path = self.safe_join(path)?;
            match tokio::fs::remove_file(path).await {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
            .map_err(|e| e.into())
        })
    }
}
