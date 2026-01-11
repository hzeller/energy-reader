use crate::ScopedTimer;
use crate::image_util::load_image_as_grayscale;

use anyhow::{Context, Result};
use image::GrayImage;
use nokhwa::Camera;
use nokhwa::pixel_format::LumaFormat; // Use Luma for grayscale
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use std::path::PathBuf;
use std::time::SystemTime;

pub struct TimestampedImage {
    pub timestamp: SystemTime,
    pub image: GrayImage,
}

/// Acquisition of new images for the detection logic.
pub trait ImageSource {
    fn read_image(&self) -> Result<TimestampedImage>;
}

pub struct FilenameSource {
    filename: PathBuf,
}
impl FilenameSource {
    pub fn new(filename: PathBuf) -> FilenameSource {
        FilenameSource { filename }
    }
}

impl ImageSource for FilenameSource {
    fn read_image(&self) -> Result<TimestampedImage> {
        let timestamp = std::fs::metadata(&self.filename)?.created()?;
        let result = TimestampedImage {
            timestamp,
            image: load_image_as_grayscale(&self.filename),
        };
        Ok(result)
    }
}

pub struct WebCamSource;
impl ImageSource for WebCamSource {
    fn read_image(&self) -> Result<TimestampedImage> {
        let _timer = ScopedTimer::new("read_image() from webcam");
        let cam = CameraIndex::Index(0);
        let format =
            RequestedFormat::new::<LumaFormat>(RequestedFormatType::AbsoluteHighestResolution);
        let mut camera = Camera::new(cam, format).context("could not find/access webcam")?;
        camera.open_stream().context("failed to open stream")?;

        for _ in 0..5 {
            let _ = camera.frame(); // Let camera adjust brightness
        }
        let frame = camera.frame().context("Could not capture image")?;
        let timestamp = SystemTime::now();
        let image = frame.decode_image::<LumaFormat>()?;

        Ok(TimestampedImage { timestamp, image })
    }
}
