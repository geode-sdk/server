use std::io::Seek;
use std::io::{BufReader, Cursor, Read};

use actix_web::web::Bytes;
use image::codecs::png::PngDecoder;
use image::codecs::png::PngEncoder;
use image::ImageEncoder;
use image::{DynamicImage, GenericImageView};
use zip::read::ZipFile;
use zip::ZipArchive;

use crate::types::api::ApiError;

pub fn extract_mod_logo(file: &mut ZipFile<Cursor<Bytes>>) -> Result<Vec<u8>, ApiError> {
    const FIVE_MEGABYTES: u64 = 5 * 1000 * 1000;
    if file.size() > FIVE_MEGABYTES {
        return Err(ApiError::BadRequest(
            "Logo size excedes max allowed size (5 MB)".into(),
        ));
    }

    let mut logo: Vec<u8> = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut logo)
        .inspect_err(|e| log::error!("logo.png read fail: {}", e))
        .or(Err(ApiError::BadRequest("Couldn't read logo.png".into())))?;

    let mut reader = BufReader::new(Cursor::new(logo));

    let mut img = PngDecoder::new(&mut reader)
        .and_then(DynamicImage::from_decoder)
        .inspect_err(|e| log::error!("Failed to create PngDecoder: {}", e))
        .or(Err(ApiError::BadRequest("Invalid logo.png".into())))?;

    let dimensions = img.dimensions();

    if dimensions.0 != dimensions.1 {
        return Err(ApiError::BadRequest(format!(
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
        .inspect_err(|e| log::error!("Failed to downscale image to 336x336: {}", e))
        .or(Err(ApiError::BadRequest("Invalid mod.json".into())))?;

    cursor.seek(std::io::SeekFrom::Start(0)).unwrap();

    let mut bytes: Vec<u8> = vec![];
    cursor.read_to_end(&mut bytes).unwrap();

    Ok(bytes)
}

pub fn validate_mod_logo(file: &mut ZipFile<Cursor<Bytes>>) -> Result<(), ApiError> {
    const FIVE_MEGABYTES: u64 = 5 * 1000 * 1000;
    if file.size() > FIVE_MEGABYTES {
        return Err(ApiError::BadRequest(
            "Logo size excedes max allowed size (5 MB)".into(),
        ));
    }

    let mut logo: Vec<u8> = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut logo)
        .inspect_err(|e| log::error!("logo.png read fail: {}", e))
        .or(Err(ApiError::BadRequest("Couldn't read logo.png".into())))?;

    let mut reader = BufReader::new(Cursor::new(logo));

    let img = PngDecoder::new(&mut reader)
        .and_then(DynamicImage::from_decoder)
        .inspect_err(|e| log::error!("Failed to create PngDecoder: {}", e))
        .or(Err(ApiError::BadRequest("Invalid logo.png".into())))?;

    let dimensions = img.dimensions();

    if dimensions.0 != dimensions.1 {
        Err(ApiError::BadRequest(format!(
            "Mod logo must have 1:1 aspect ratio. Current size is {}x{}",
            dimensions.0, dimensions.1
        )))
    } else {
        Ok(())
    }
}

pub async fn download_mod(url: &str, limit_mb: u32) -> Result<Bytes, ApiError> {
    download(url, limit_mb).await
}

pub async fn download_mod_hash_comp(
    url: &str,
    hash: &str,
    limit_mb: u32,
) -> Result<Bytes, ApiError> {
    let bytes = download(url, limit_mb).await?;

    let slice: &[u8] = &bytes;

    let new_hash = sha256::digest(slice);
    if new_hash != hash {
        return Err(ApiError::BadRequest(format!(
            ".geode hash mismatch: old {hash}, new {new_hash}",
        )));
    }

    Ok(bytes)
}

pub fn bytes_to_ziparchive(bytes: Bytes) -> Result<ZipArchive<Cursor<Bytes>>, ApiError> {
    ZipArchive::new(Cursor::new(bytes))
        .inspect_err(|e| log::error!("Failed to create ZipArchive: {}", e))
        .or(Err(ApiError::BadRequest(
            "Invalid .geode file, couldn't read archive".into(),
        )))
}

async fn download(url: &str, limit_mb: u32) -> Result<Bytes, ApiError> {
    let limit_bytes: u64 = limit_mb as u64 * 1_000_000;
    let mut response = reqwest::get(url)
        .await
        .inspect_err(|e| log::error!("Failed to fetch .geode file: {e}"))
        .or(Err(ApiError::BadRequest(
            "Failed to fetch .geode file".into(),
        )))?;

    // Check Content-Length, but the server can lie about this, so we'll also stream the file
    // If the header is somehow unavailable, we'll just check the size when streaming
    let content_length = response.content_length().unwrap_or(0);

    if content_length > limit_bytes {
        let len_mb = content_length / 1_000_000;
        return Err(ApiError::BadRequest(format!(
            "Mod file is too large ({} mb), max size is {} mb",
            len_mb, limit_mb
        )));
    }

    let mut data: Vec<u8> = Vec::with_capacity(content_length as usize);

    let mut streamed: u64 = 0;
    while let Some(chunk) = response.chunk().await.or(Err(ApiError::BadRequest(
        "Failed to read .geode chunk".into(),
    )))? {
        streamed += chunk.len() as u64;

        if streamed > limit_bytes {
            let len_mb = streamed / 1_000_000;
            return Err(ApiError::BadRequest(format!(
                "Mod file is too large ({} mb), max size is {} mb",
                len_mb, limit_mb
            )));
        }

        data.extend_from_slice(&chunk);
    }

    Ok(Bytes::from(data))
}
