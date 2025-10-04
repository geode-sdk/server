use std::io::Seek;
use std::io::{BufReader, Cursor, Read};

use actix_web::web::Bytes;
use image::codecs::png::PngDecoder;
use image::codecs::png::PngEncoder;
use image::{DynamicImage, GenericImageView};
use image::{ImageEncoder, ImageError};
use zip::read::ZipFile;
use zip::result::ZipError;
use zip::ZipArchive;

#[derive(thiserror::Error, Debug)]
pub enum ModZipError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Image operation error: {0}")]
    ImageError(#[from] ImageError),
    #[error("Failed to unzip .geode file: {0}")]
    ZipError(#[from] ZipError),
    #[error("Failed to parse JSON: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Invalid mod logo: {0}")]
    InvalidLogo(String),
    #[error(".geode file hash mismatch: {0} doesn't match {1}")]
    ModFileHashMismatch(String, String),
    #[error("Failed to fetch .geode file: {0}")]
    ModFileFetchError(#[from] reqwest::Error),
    #[error(".geode file is too large ({0} MB), maximum is {1} MB")]
    ModFileTooLarge(u64, u64),
    #[error("Invalid mod.json: {0}")]
    InvalidModJson(String),
    #[error("Invalid binaries: {0}")]
    InvalidBinaries(String),
    #[error("{0}")]
    GenericError(String),
}

pub fn extract_mod_logo(file: &mut ZipFile<Cursor<Bytes>>) -> Result<Vec<u8>, ModZipError> {
    let mut logo: Vec<u8> = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut logo)
        .inspect_err(|e| log::error!("logo.png read fail: {}", e))?;

    let mut reader = BufReader::new(Cursor::new(logo));

    let mut img = PngDecoder::new(&mut reader)
        .and_then(DynamicImage::from_decoder)
        .inspect_err(|e| log::error!("Failed to create PngDecoder: {}", e))?;

    let dimensions = img.dimensions();

    if dimensions.0 != dimensions.1 {
        return Err(ModZipError::InvalidLogo(format!(
            "Mod logo must have 1:1 aspect ratio. Current size is {}x{}",
            dimensions.0, dimensions.1
        )));
    }

    if (dimensions.0 > 336) || (dimensions.1 > 336) {
        img = img.resize(336, 336, image::imageops::FilterType::Lanczos3);
    }

    let mut cursor: Cursor<Vec<u8>> = Cursor::new(vec![]);

    let encoder = PngEncoder::new_with_quality(
        &mut cursor,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::NoFilter,
    );

    let (width, height) = img.dimensions();

    encoder
        .write_image(img.as_bytes(), width, height, img.color().into())
        .inspect_err(|e| log::error!("Failed to downscale image to 336x336: {}", e))?;

    cursor.seek(std::io::SeekFrom::Start(0)).unwrap();

    let mut bytes: Vec<u8> = vec![];
    cursor.read_to_end(&mut bytes).unwrap();

    Ok(bytes)
}

pub fn validate_mod_logo(file: &mut ZipFile<Cursor<Bytes>>) -> Result<(), ModZipError> {
    let mut logo: Vec<u8> = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut logo)
        .inspect_err(|e| log::error!("logo.png read fail: {}", e))?;

    let mut reader = BufReader::new(Cursor::new(logo));

    let img = PngDecoder::new(&mut reader)
        .and_then(DynamicImage::from_decoder)
        .inspect_err(|e| log::error!("Failed to create PngDecoder: {}", e))?;

    let dimensions = img.dimensions();

    if dimensions.0 != dimensions.1 {
        Err(ModZipError::InvalidLogo(format!(
            "Mod logo must have 1:1 aspect ratio. Current size is {}x{}",
            dimensions.0, dimensions.1
        )))
    } else {
        Ok(())
    }
}

pub async fn download_mod(url: &str, limit_mb: u32) -> Result<Bytes, ModZipError> {
    download(url, limit_mb).await
}

pub async fn download_mod_hash_comp(
    url: &str,
    hash: &str,
    limit_mb: u32,
) -> Result<Bytes, ModZipError> {
    let bytes = download(url, limit_mb).await?;

    let slice: &[u8] = &bytes;

    let new_hash = sha256::digest(slice);
    if new_hash != hash {
        return Err(ModZipError::ModFileHashMismatch(hash.into(), new_hash));
    }

    Ok(bytes)
}

pub fn bytes_to_ziparchive(bytes: Bytes) -> Result<ZipArchive<Cursor<Bytes>>, ModZipError> {
    ZipArchive::new(Cursor::new(bytes))
        .inspect_err(|e| log::error!("Failed to create ZipArchive: {}", e))
        .map_err(|e| e.into())
}

async fn download(url: &str, limit_mb: u32) -> Result<Bytes, ModZipError> {
    let limit_bytes = limit_mb * 1_000_000;
    let response = reqwest::get(url)
        .await
        .inspect_err(|e| log::error!("Failed to fetch .geode file: {e}"))?;

    let len = response.content_length().ok_or(ModZipError::GenericError(
        "Couldn't determine .geode file size".into(),
    ))?;

    if len > limit_bytes as u64 {
        let len_mb = len / 1_000_000;
        return Err(ModZipError::ModFileTooLarge(len_mb, limit_mb.into()));
    }

    response
        .bytes()
        .await
        .inspect_err(|e| log::error!("Failed to get bytes from .geode: {}", e))
        .map_err(|e| e.into())
}
