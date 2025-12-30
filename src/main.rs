use image::{DynamicImage,GrayImage,Luma};
use std::env;
use std::cmp;

#[cfg(feature = "debug_img")]
mod debugdigit;

pub const THRESHOLD:f32 = 0.6;

pub struct DigitPos {
    digit: u32,
    score: f32,
    pos: u32,
}
pub type ColumnFeatureScore = Vec<f32>;

fn load_image_as_grayscale(path : &str) -> GrayImage {
    let rgba = image::open(path).unwrap().into_rgba8();
    DynamicImage::ImageRgba8(rgba).into_luma8()
}

// Score of the particular digit image at a particular position.


// Determine score of given needle pattern existing at given haystack column.
fn cross_correlate(haystack: &GrayImage, needle: &GrayImage)
		   -> ColumnFeatureScore {
    // Prepare to normalize the output.
    let needle_sum = needle.iter().fold(0, |sum:u32, &p| { sum + p as u32 });
    let mut output = vec![0.0; haystack.width() as usize];

    // Expensive, but ok for now as we only look at a small image.
    for hx in 0..haystack.width() - needle.width() {
	let mut column_high_score : f32 = 0.0;
	for hy in 0..haystack.height() - needle.height() {
	    let mut value = 0.0;
	    for ny in 0..needle.height() {
		for nx in 0..needle.width() {
		    value += needle.get_pixel(nx, ny)[0] as f32 *
			haystack.get_pixel(hx+nx, hy+ny)[0] as f32;
		}
	    }
	    value /= needle_sum as f32;
	    column_high_score = column_high_score.max(value / 255.0);
	}
	output[hx as usize] = column_high_score;
    }
    output
}

// Classic edge detection.
fn sobel(input: &GrayImage) -> GrayImage {
    let width: u32 = input.width() - 2;
    let height: u32 = input.height() - 2;
    let mut result = GrayImage::new(width, height);

    for x in 0..width {
        for y in 0..height {
            let nw = input.get_pixel(x, y)[0] as i32;
	    let north = input.get_pixel(x + 1 , y)[0] as i32;
            let ne = input.get_pixel(x + 2, y)[0] as i32;

            let west = input.get_pixel(x, y + 1)[0] as i32;
            let east = input.get_pixel(x + 2, y + 1)[0] as i32;

	    let sw = input.get_pixel(x, y + 2)[0] as i32;
	    let south = input.get_pixel(x + 1, y + 2)[0] as i32;
            let se = input.get_pixel(x + 2, y + 2)[0] as i32;

	    // Sobel kernel in x and y direction
	    let gx = -nw + ne
		+    (-2 * west) + (2 * east)
		+    -sw + se;
	    let gy = -nw + (-2 * north) + -ne
                +     sw + ( 2 * south) + se;

            let mut mag = ((gx as f64).powi(2) + (gy as f64).powi(2)).sqrt();

	    mag = mag.clamp(0.0, 255.0);

            result.put_pixel(x, y, Luma([mag as u8]));
        }
    }
    result
}

fn locate_digits(scores: &[ColumnFeatureScore], picture_width: u32,
		 digit_width: u32)
		 -> Vec<DigitPos> {
    let mut result = Vec::new();
    let mut current = DigitPos{digit: u32::MAX, score: 0.0, pos: picture_width};
    for x in 0..picture_width {
	// What is the highest scoring digit in each of the picture columns
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

	if x >= current.pos + digit_width {  // best seen for digit-width
	    result.push(current);
	    current = DigitPos{digit: u32::MAX, score: 0.0, pos: picture_width};
	}
    }
    result
}

// Params: energy-reader image <digit0> <digit1>...
fn main() {
    // First image is the text containing image, followed by digits.
    let haystack_file = env::args().nth(1).expect("want metering image.");
    let haystack = sobel(&load_image_as_grayscale(haystack_file.as_str()));

    let mut max_digit_width = 0;
    let mut digits = Vec::new();
    for digit_picture in env::args().skip(2) {
	let digit = sobel(&load_image_as_grayscale(digit_picture.as_str()));
	max_digit_width = cmp::max(max_digit_width, digit.width());
	digits.push(digit);
    }

    // Create similarity score per haystack pixel column.
    let mut digit_scores : Vec<ColumnFeatureScore> = Vec::new();
    for digit in digits.iter() {
	let highest_column = cross_correlate(&haystack, digit);
	digit_scores.push(highest_column);
    }

    let digit_locations = locate_digits(&digit_scores, haystack.width(),
					max_digit_width);
    for loc in &digit_locations {
	println!("{} {} {:4} {:.3}", loc.digit,
		 env::args().nth((loc.digit + 2) as usize)
		 .expect("should be valid arg"), loc.pos, loc.score);
    }

    #[cfg(feature = "debug_img")]
    debugdigit::debug_print_digits(&haystack, &digits, max_digit_width,
				   &digit_scores, &digit_locations)
	.save("output.png").unwrap();
}
