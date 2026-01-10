use crate::DigitPos;
use image::{GrayImage, Luma};

// Create a sparkline image of given height.
fn graph(values: &[f32], highlight: f32, height: u32) -> GrayImage {
    let mut result = GrayImage::new(values.len() as u32, height);
    let white = Luma::<u8>::from([255; 1]);

    for (ix, value) in values.iter().enumerate() {
        let value = value.clamp(0.0, 1.0);
        let img_range = (height - 1) as f32;
        let iy = ((1.0 - value) * img_range) as u32;
        result.put_pixel(ix as u32, iy, white);
        if value >= highlight {
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
    digit_filename: &[String],
) -> GrayImage {
    let sparkline_height = (1.5 * max_digit_height as f32) as u32;

    let mut vertical_pos = 0;
    let width = max_digit_width + haystack.width();
    let height = haystack.height() + (1 + digits.len() as u32) * sparkline_height;
    let mut output = GrayImage::new(width, height);

    // Original image as first
    image::imageops::overlay(
        &mut output,
        haystack,
        max_digit_width as i64,
        vertical_pos as i64,
    );
    vertical_pos += haystack.height();

    // Show each digit followed by its sparkline
    for (i, digit) in digits.iter().enumerate() {
        image::imageops::overlay(&mut output, digit, 0, vertical_pos as i64);

        // Highlight starting from the minimum score this particular digit
        // was ever selected at.
        let highlight_score = digit_positions
            .iter()
            .filter_map(|p| {
                if p.digit_template as usize == i {
                    Some(p.score)
                } else {
                    None
                }
            })
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(1.0);

        let visualize = graph(&digit_scores[i], highlight_score, sparkline_height);
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
        let digit_pic = &digits[loc.digit_template as usize];
        image::imageops::overlay(
            &mut output,
            digit_pic,
            (max_digit_width + loc.pos) as i64,
            vertical_pos as i64,
        );
        eprintln!(
            "{} {:5} {:.3}",
            digit_filename[loc.digit_template as usize], loc.pos, loc.score
        );
    }

    output
}
