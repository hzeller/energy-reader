#![allow(dead_code)]
#![allow(unreachable_code)]

use image::{DynamicImage,GrayImage,ImageBuffer,Luma};
use std::env;
use std::cmp;

//const THRESHOLD:f32 = 0.75;
const THRESHOLD:f32 = 0.8;   // ok with sobel

fn load_as_grayscale(path : &str) -> GrayImage {
    let rgba = image::open(path).unwrap().into_rgba8();
    DynamicImage::ImageRgba8(rgba).into_luma8()
}

// Intemediate image has a high-resolution
type ProcessedImage = ImageBuffer::<Luma<f32>, Vec<f32>>;

fn highest_column_value(img : &ProcessedImage) -> Vec<f32> {
    let mut result = Vec::new();
    let min = img.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max = img.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)) - min;
    let range = max - min;
    println!("{}..{} (range: {})", min, max, range);
    // TODO: Do some sort of map()
    for ix in 0..img.width() {
	let mut highest_column_value : f32 = 0.0;
	for iy in 0..img.height() {
	    let p = img.get_pixel(ix, iy)[0];
	    let zero_to_one = (p - min) / range;
	    highest_column_value = highest_column_value.max(zero_to_one);
	}
	result.push(highest_column_value);
    }
    result
}

fn graph(values: &Vec<f32>, height: u32) -> GrayImage {
    let mut result = GrayImage::new(values.len() as u32, height);
    let white = Luma::<u8>::from([255;1]);

    for ix in 0..values.len() {
	let value = values[ix];
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

fn cross_correlate(haystack: &GrayImage, needle: &GrayImage) -> ProcessedImage {
    let pixel_sum = needle.iter().fold(0, |sum:u32, &p| { sum + p as u32 });
    //println!("Pixel sum: {}", pixel_sum);
    let mut output = ProcessedImage::new(haystack.width(),
					 haystack.height());
    for hx in 0..haystack.width() - needle.width() {
	for hy in 0..haystack.height() - needle.height() {
	    let mut value : [f32; 1] = [0.0; 1];
	    for nx in 0..needle.width() {
		for ny in 0..needle.height() {
		    value[0] += needle.get_pixel(nx, ny)[0] as f32 *
			haystack.get_pixel(hx+nx, hy+ny)[0] as f32;
		}
	    }
	    value[0] /= pixel_sum as f32;
	    output.put_pixel(hx, hy, Luma::<f32>::from(value));
	}
    }
    output
}

fn sobel(input: &GrayImage) -> GrayImage {
    //return input.clone();

    let width: u32 = input.width() - 2;
    let height: u32 = input.height() - 2;
    let mut buff = GrayImage::new(width, height);

    for x in 0..width {
        for y in 0..height {
            /* Unwrap those loops! */
            let val0 = input.get_pixel(x, y)[0] as i32;
            let val1 = input.get_pixel(x + 1 , y)[0] as i32;
            let val2 = input.get_pixel(x + 2, y)[0] as i32;
            let val3 = input.get_pixel(x, y + 1)[0] as i32;
            let val5 = input.get_pixel(x + 2, y + 1)[0] as i32;
            let val6 = input.get_pixel(x, y + 2)[0] as i32;
            let val7 = input.get_pixel(x + 1, y + 2)[0] as i32;
            let val8 = input.get_pixel(x + 2, y + 2)[0] as i32;
            /* Apply Sobel kernels */
            let gx = (-1 * val0) + (-2 * val3) + (-1 * val6) + val2 + (2 * val5) + val8;
            let gy = (-1 * val0) + (-2 * val1) + (-1 * val2) + val6 + (2 * val7) + val8;
            let mut mag = ((gx as f64).powi(2) + (gy as f64).powi(2)).sqrt();

            if mag > 255.0 {
                mag = 255.0;
            }

            buff.put_pixel(x, y, Luma([mag as u8]));
        }
    }
    return buff;
}

// Params: energy-reader image <digit0> <digit1>...
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let haystack_raw = load_as_grayscale(env::args().nth(1).unwrap().as_str());
    let haystack = sobel(&haystack_raw);

    let mut max_digit_width = 0;
    let mut digits = Vec::new();
    for picture in env::args().skip(2) {
	let digit = sobel(&load_as_grayscale(picture.as_str()));
	max_digit_width = cmp::max(max_digit_width, digit.width());
	digits.push(digit);
    }

    // Overlay the found digits.
    //image::imageops::overlay(&mut haystack, &digits[0], 50, 16);

    let mut vertical_pos = 0;
    let mut output = GrayImage::new(max_digit_width + haystack.width(),
				    (digits.len() as u32+2) * haystack.height());
    // Original as first
    image::imageops::overlay(&mut output, &haystack, max_digit_width as i64,
			     vertical_pos as i64);
    vertical_pos += haystack.height();

    // the heatmap/graphs of digit matches
    let mut digit_scores : Vec<Vec<f32>> = Vec::new();
    for digit in digits.iter() {
	let xcorr = cross_correlate(&haystack, digit);
	let highest_column = highest_column_value(&xcorr);
	digit_scores.push(highest_column);
	let visualize = graph(&digit_scores[digit_scores.len()-1],
			      haystack.height());

	image::imageops::overlay(&mut output, digit, 0,
				 vertical_pos as i64);
	image::imageops::overlay(&mut output, &visualize, max_digit_width as i64,
				 vertical_pos as i64);

	vertical_pos += haystack.height();
    }

    let mut descision_cutoff = u32::MAX;
    let mut descision_score = 0.0;
    let mut descision_index = -1;

    for x in 0..haystack.width() {
	// What is the highest scoring digit
	let mut found_index = -1;
	for i in 0..digit_scores.len() {
	    let feature_score = &digit_scores[i];
	    let value = feature_score[x as usize];
	    if value < THRESHOLD {
		continue;
	    }
	    if value > descision_score {
		found_index = i as i32;
		descision_score = value;
	    }
	}

	if found_index >= 0 && found_index != descision_index {
	    descision_index = found_index;
	    descision_cutoff = x + digits[descision_index as usize].width() as u32;
	}

	if x >= descision_cutoff {
	    let digit = &digits[descision_index as usize];
	    image::imageops::overlay(&mut output, digit,
				     x as i64,
				     vertical_pos as i64);
	    descision_cutoff = u32::MAX;
	    descision_index = -1;
	    descision_score = 0.0;
	}
    }
    // The final chosen match.
    output.save("output.png")?;

    Ok(())
}
