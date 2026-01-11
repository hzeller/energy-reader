use crate::ScopedTimer;

use anyhow::{Context, Result};
use image::imageops::{crop, flip_horizontal, flip_vertical, rotate90, rotate180};
use image::{GrayImage, Luma};
use std::path::PathBuf;
use std::str::FromStr;

pub fn load_image_as_grayscale(path: &PathBuf) -> GrayImage {
    image::open(path)
        .expect("Failed to open image")
        .into_luma8()
}

// Classic edge detection.
pub fn sobel(input: &GrayImage) -> GrayImage {
    let (width, height) = (input.width(), input.height());
    if width < 3 || height < 3 {
        return input.clone();
    }

    let (out_w, out_h) = (width - 2, height - 2);
    let mut result = GrayImage::new(out_w, out_h);

    for y in 0..out_h {
        for x in 0..out_w {
            // direct relative indexing.
            let p = |dx: u32, dy: u32| input[(x + dx, y + dy)][0] as i32;

            let (nw, north, ne) = (p(0, 0), p(1, 0), p(2, 0));
            let (west, east) = (p(0, 1), p(2, 1));
            let (sw, south, se) = (p(0, 2), p(1, 2), p(2, 2));

            // Sobel kernel in x and y direction
            let gx = (ne - nw) + 2 * (east - west) + (se - sw);
            let gy = (sw - nw) + 2 * (south - north) + (se - ne);

            let mag = ((gx * gx + gy * gy) as f32).sqrt().min(255.0);
            result.put_pixel(x, y, Luma([mag as u8]));
        }
    }
    result
}

#[derive(Clone, Debug)]
pub enum ImageOp {
    Rotate90,
    Rotate180,
    FlipHorizontal,
    FlipVertical,
    Crop { x: u32, y: u32, w: u32, h: u32 },
}

impl FromStr for ImageOp {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.as_slice() {
            ["rotate90"] => Ok(ImageOp::Rotate90),
            ["rotate180"] => Ok(ImageOp::Rotate180),
            ["flip-x"] => Ok(ImageOp::FlipHorizontal),
            ["flip-y"] => Ok(ImageOp::FlipVertical),
            ["crop", x, y, w, h] => Ok(ImageOp::Crop {
                x: x.parse().context("Can't parse 1st ('x') as integer")?,
                y: y.parse().context("Can't parse 2nd ('y') as integer")?,
                w: w.parse().context("Can't parse 3rd ('width') as integer")?,
                h: h.parse().context("Can't parse 4th ('height') as integer")?,
            }),
            _ => anyhow::bail!(
                "Unknown operation format: {}; one of 'rotate90', 'rotate180', 'flip-x', flip-y', 'crop:<x>:<y>:<width>:<height>'",
                s
            ),
        }
    }
}

pub fn apply_ops(image: &mut GrayImage, ops: &[ImageOp]) -> Result<()> {
    let _timer = ScopedTimer::new("image_utils::apply_ops()");
    for op in ops {
        match op {
            ImageOp::Rotate90 => *image = rotate90(image),
            ImageOp::Rotate180 => *image = rotate180(image),
            ImageOp::FlipHorizontal => *image = flip_horizontal(image),
            ImageOp::FlipVertical => *image = flip_vertical(image),
            ImageOp::Crop { x, y, w, h } => {
                if x + w > image.width() || y + h > image.height() {
                    anyhow::bail!(
                        "Crop dimensions out of bounds; image size is {}x{}",
                        image.width(),
                        image.height()
                    );
                }
                *image = crop(image, *x, *y, *w, *h).to_image();
            }
        }
    }
    Ok(())
}
