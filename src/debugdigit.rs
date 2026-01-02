use crate::{DigitPos, THRESHOLD};
use image::{GrayImage, Luma};

// Create a sparkline image of given height.
fn graph(values: &[f32], height: u32) -> GrayImage {
    let mut result = GrayImage::new(values.len() as u32, height);
    let white = Luma::<u8>::from([255; 1]);

    for (ix, value) in values.iter().enumerate() {
        let value = value.clamp(0.0, 1.0);
        let img_range = (height - 1) as f32;
        let iy = ((1.0 - value) * img_range) as u32;
        result.put_pixel(ix as u32, iy, white);
        if value > THRESHOLD {
            for y in iy..height {
                result.put_pixel(ix as u32, y, white);
            }
        }
    }
    result
}

pub fn debug_print_digits(
    haystack: &GrayImage,
    digits: &[GrayImage],
    max_digit_width: u32,
    max_digit_height: u32,
    digit_scores: &[Vec<f32>],
    digit_positions: &[DigitPos],
) -> GrayImage {
    let sparkline_height = (1.5 * max_digit_height as f32) as u32;

    let mut vertical_pos = 0;
    let width = max_digit_width + haystack.width();
    let height = haystack.height() + (1 + digits.len() as u32) * sparkline_height;
    let mut output = GrayImage::new(width, height);

    // Original as first
    image::imageops::overlay(
        &mut output,
        haystack,
        max_digit_width as i64,
        vertical_pos as i64,
    );
    vertical_pos += haystack.height();

    // For each digit its sparkline
    for (i, digit) in digits.iter().enumerate() {
        image::imageops::overlay(&mut output, digit, 0, vertical_pos as i64);
        let visualize = graph(&digit_scores[i], sparkline_height);
        image::imageops::overlay(
            &mut output,
            &visualize,
            max_digit_width as i64,
            vertical_pos as i64,
        );
        vertical_pos += sparkline_height;
    }

    // The final recognized digits.
    for loc in digit_positions {
        let digit_pic = &digits[loc.digit as usize];
        image::imageops::overlay(
            &mut output,
            digit_pic,
            (max_digit_width + loc.pos) as i64,
            vertical_pos as i64,
        );
        eprintln!("{} {:4} {:.3}", loc.digit, loc.pos, loc.score);
    }

    output
}
