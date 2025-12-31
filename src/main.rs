#![allow(dead_code)]

use image::{DynamicImage, GrayImage, Luma};
use std::cmp;
use std::env;

mod cross_correlator;
use cross_correlator::{CrossCorrelator,ColumnFeatureScore};

#[cfg(feature = "debug_img")]
mod debugdigit;

pub const THRESHOLD: f32 = 0.6;

#[derive(Clone)]
pub struct DigitPos {
    digit: u32,
    score: f32,
    pos: u32,
}

fn load_image_as_grayscale(path: &str) -> GrayImage {
    let rgba = image::open(path).unwrap().into_rgba8();
    DynamicImage::ImageRgba8(rgba).into_luma8()
}

// Classic edge detection.
fn sobel(input: &GrayImage) -> GrayImage {
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

// Find the hightest score digits and emit their positions.
fn locate_digits(scores: &[ColumnFeatureScore], digit_width: u32)
                 -> Vec<DigitPos> {
    // The shortest score vector is the max x-position we check out
    let x_range = scores.iter().map(|v| v.len()).min().unwrap_or(0) as u32;
    let mut result = Vec::new();
    let fresh_digit = DigitPos { digit: u32::MAX, score: 0.0, pos: x_range };
    let mut current = fresh_digit.clone();
    // Find highest score that does not change for the width of a digit.
    for x in 0..x_range {
        for (i, feature_score) in scores.iter().enumerate() {
            let digit_score = feature_score[x as usize];
            if digit_score < THRESHOLD {
                continue;
            }
            if digit_score > current.score {
                current.digit = i as u32;
                current.score = digit_score;
                current.pos = x;
            }
        }

        if x >= current.pos + digit_width { // best seen for digit-width
            result.push(current);
            current = fresh_digit.clone();
        }
    }
    result
}

// Params: energy-reader <counter-image> <digit0> <digit1>...
fn main() {
    // First image is the text containing image, followed by digits.
    let haystack_file = env::args().nth(1).expect("want metering image.");
    let haystack = sobel(&load_image_as_grayscale(haystack_file.as_str()));

    let mut max_digit_w = 0u32;
    let mut max_digit_h = 0u32;
    let mut digits = Vec::new();
    for digit_picture in env::args().skip(2) {
        let digit = sobel(&load_image_as_grayscale(digit_picture.as_str()));
        max_digit_w = cmp::max(max_digit_w, digit.width());
        max_digit_h = cmp::max(max_digit_h, digit.height());
        digits.push(digit);
    }

    let correlator = CrossCorrelator::new(&haystack, max_digit_w, max_digit_h);
    let mut digit_scores: Vec<ColumnFeatureScore> = Vec::new();
    for digit in digits.iter() {
        digit_scores.push(correlator.cross_correlate_with(digit));
    }

    // Output to stdout for further processing.
    let digit_locations = locate_digits(&digit_scores, max_digit_w);
    for loc in &digit_locations {
        println!("{} {:4} {:.3}", loc.digit, loc.pos, loc.score);
    }

    #[cfg(feature = "debug_img")]
    debugdigit::debug_print_digits(
        &haystack,
        &digits,
        max_digit_w,
        max_digit_h,
        &digit_scores,
        &digit_locations,
    )
    .save("output.png")
    .unwrap();
}
