use image::GrayImage;
use crate::image_util::load_image_as_grayscale;
use std::path::PathBuf;
use std::time::SystemTime;
use anyhow::Result;
use nokhwa::Camera;
use nokhwa::pixel_format::LumaFormat; // Use Luma for grayscale
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};

pub struct TimestampedImage {
    pub timestamp: SystemTime,
    pub image: GrayImage,
}

/// Acquisition of new images for the detection logic.
pub trait ImageSource {
    fn read_image(&self) -> Result<TimestampedImage>;
}

pub struct FilenameSource {
    filename: PathBuf
}
impl FilenameSource {
    pub fn new(filename: PathBuf) -> FilenameSource {
        FilenameSource{filename}
    }
}

impl ImageSource for FilenameSource {
    fn read_image(&self) -> Result<TimestampedImage> {
        let time = std::fs::metadata(&self.filename)?.created()?;
        let result = TimestampedImage {
            timestamp: time,
            image: load_image_as_grayscale(&self.filename.to_string_lossy()),
        };
        Ok(result)
    }
}

pub struct WebCamSource;
impl ImageSource for WebCamSource {
    fn read_image(&self) -> Result<TimestampedImage> {
        let cam = CameraIndex::Index(0);
        let format = RequestedFormat::new::<LumaFormat>(
            RequestedFormatType::AbsoluteHighestResolution,
        );
        let mut camera = Camera::new(cam, format)?;
        camera.open_stream()?;
        let frame = camera.frame()?;
        let now = SystemTime::now();
        let image = frame.decode_image::<LumaFormat>()?;
        Ok(TimestampedImage{timestamp: now, image})
    }
}
