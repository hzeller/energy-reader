use image::{DynamicImage, GrayImage, Luma};
use anyhow::Result;
use image::imageops::{rotate90, rotate180, crop};

pub fn load_image_as_grayscale(path: &str) -> GrayImage {
    let rgba = image::open(path).unwrap().into_rgba8();
    DynamicImage::ImageRgba8(rgba).into_luma8()
}

// Classic edge detection.
pub fn sobel(input: &GrayImage) -> GrayImage {
    let width: u32 = input.width() - 2;
    let height: u32 = input.height() - 2;
    let mut result = GrayImage::new(width, height);

    for x in 0..width {
        for y in 0..height {
            let nw = input.get_pixel(x, y)[0] as i32;
            let north = input.get_pixel(x + 1, y)[0] as i32;
            let ne = input.get_pixel(x + 2, y)[0] as i32;

            let west = input.get_pixel(x, y + 1)[0] as i32;
            let east = input.get_pixel(x + 2, y + 1)[0] as i32;

            let sw = input.get_pixel(x, y + 2)[0] as i32;
            let south = input.get_pixel(x + 1, y + 2)[0] as i32;
            let se = input.get_pixel(x + 2, y + 2)[0] as i32;

            // Sobel kernel in x and y direction
            #[rustfmt::skip]
            let gx = (-nw        + ne
                +    (-2 * west) + (2 * east)
                +    -sw         + se) as f32;

            #[rustfmt::skip]
            let gy = (-nw  + (-2 * north) + -ne
                +     sw   + ( 2 * south) +  se) as f32;

            let mag = gx.hypot(gy).clamp(0.0, 255.0);
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
                let x : u32 = x.parse()?;
                let y : u32 = y.parse()?;
                let w : u32 = w.parse()?;
                let h : u32 = h.parse()?;
                *image = crop(image, x, y, w, h).to_image();
            }
            _ => anyhow::bail!("unknown op: {}", op),
        }
    }
    Ok(())
}
