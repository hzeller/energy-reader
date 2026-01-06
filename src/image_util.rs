use image::{GrayImage, Luma};
use anyhow::{Result, Context, anyhow};
use image::imageops::{rotate90, rotate180, crop};

pub fn load_image_as_grayscale(path: &str) -> GrayImage {
    image::open(path)
        .expect("Failed to open image")
        .into_luma8()
}

// Classic edge detection.
pub fn sobel(input: &GrayImage) -> GrayImage {
    let (width, height) = (input.width(), input.height());
    if width < 3 || height < 3 { return input.clone(); }

    let (out_w, out_h) = (width - 2, height - 2);
    let mut result = GrayImage::new(out_w, out_h);

    for y in 0..out_h {
        for x in 0..out_w {
            // direct relative indexing.
            let p = |dx: u32, dy: u32| input[(x + dx, y + dy)][0] as i32;

            let (nw,   north,   ne) = (p(0, 0), p(1, 0), p(2, 0));
            let (west,        east) = (p(0, 1), p(2, 1));
            let (sw,   south,   se) = (p(0, 2), p(1, 2), p(2, 2));

            // Sobel kernel in x and y direction
            let gx = (ne - nw) + 2 * (east - west)   + (se - sw);
            let gy = (sw - nw) + 2 * (south - north) + (se - ne);

            let mag = ((gx * gx + gy * gy) as f32).sqrt().min(255.0);
            result.put_pixel(x, y, Luma([mag as u8]));
        }
    }
    result
}

pub fn apply_ops(image: &mut GrayImage, ops: &[String]) -> Result<()> {
    for op in ops {
        let parts: Vec<&str> = op.split(':').collect();

        match parts.as_slice() {
            ["rotate180"] => *image = rotate180(image),
            ["rotate90"] => *image = rotate90(image),
            ["crop", x, y, w, h] => {
                let x : u32 = x.parse().context("Invalid x")?;
                let y : u32 = y.parse().context("Invalid y")?;
                let w : u32 = w.parse().context("Invalid width")?;
                let h : u32 = h.parse().context("Invalid height")?;
                if x + w > image.width() || y + h > image.height() {
                    return Err(anyhow!("Crop dimensions out of image bounds"));
                }
                *image = crop(image, x, y, w, h).to_image();
            }
            _ => anyhow::bail!("unknown or malformed op: {}", op),
        }
    }
    Ok(())
}
