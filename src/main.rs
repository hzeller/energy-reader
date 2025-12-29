#![allow(dead_code)]
#![allow(unreachable_code)]

use image::{DynamicImage,GrayImage,Luma};
use std::env;
use std::cmp;

const THRESHOLD:f32 = 0.6;
//const THRESHOLD:f32 = 0.4;  // w/o sobel

fn load_as_grayscale(path : &str) -> GrayImage {
    let rgba = image::open(path).unwrap().into_rgba8();
    DynamicImage::ImageRgba8(rgba).into_luma8()
}

type ColumnFeatureScore = Vec<f32>;

fn graph(values: &ColumnFeatureScore, height: u32) -> GrayImage {
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

fn sobel(input: &GrayImage) -> GrayImage {
    //return input.clone();

    let width: u32 = input.width() - 2;
    let height: u32 = input.height() - 2;
    let mut buff = GrayImage::new(width, height);

    for x in 0..width {
        for y in 0..height {
            let nw = input.get_pixel(x, y)[0] as i32;
            let ne = input.get_pixel(x + 2, y)[0] as i32;

            let north = input.get_pixel(x + 1 , y)[0] as i32;
            let west = input.get_pixel(x, y + 1)[0] as i32;
            let east = input.get_pixel(x + 2, y + 1)[0] as i32;
            let south = input.get_pixel(x + 1, y + 2)[0] as i32;

	    let sw = input.get_pixel(x, y + 2)[0] as i32;
            let se = input.get_pixel(x + 2, y + 2)[0] as i32;

	    let gx = (-1 *   nw) + (1 *   ne)
		+    (-2 * west) + (2 * east)
		+    (-1 *   sw) + (1 *   se);
	    let gy = (-1 * nw) + (-2 * north) + (-1 * ne)
                +    ( 1 * sw) + ( 2 * south) + ( 1 * se);

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
    // First image is the text containing image, followed by digits.
    let haystack_raw = load_as_grayscale(env::args().nth(1).unwrap().as_str());
    let haystack = sobel(&haystack_raw);

    let mut max_digit_width = 0;
    let mut digits = Vec::new();
    for picture in env::args().skip(2) {
	let digit = sobel(&load_as_grayscale(picture.as_str()));
	max_digit_width = cmp::max(max_digit_width, digit.width());
	digits.push(digit);
    }

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
	let highest_column = cross_correlate(&haystack, digit);
	digit_scores.push(highest_column);
	let visualize = graph(&digit_scores[digit_scores.len()-1],
			      haystack.height());

	image::imageops::overlay(&mut output, digit, 0,
				 vertical_pos as i64);
	image::imageops::overlay(&mut output, &visualize, max_digit_width as i64,
				 vertical_pos as i64);

	vertical_pos += haystack.height();
    }

    let mut last_print_pos = 0;
    let mut decision_cutoff = u32::MAX;
    let mut decision_score = 0.0;
    let mut decision_index = -1;

    for x in 0..haystack.width() {
	// What is the highest scoring digit
	let mut found_index = -1;
	for i in 0..digit_scores.len() {
	    let feature_score = &digit_scores[i];
	    let value = feature_score[x as usize];
	    if value < THRESHOLD {
		continue;
	    }
	    if value > decision_score {
		found_index = i as i32;
		decision_score = value;
	    }
	}

	if found_index >= 0 && found_index != decision_index {
	    decision_index = found_index;
	    decision_cutoff = x + digits[decision_index as usize].width() as u32;
	}

	if x >= decision_cutoff {
	    println!("Digit {} at {} (Δ={})", decision_index, x,
		     x - last_print_pos);
	    last_print_pos = x;
	    let digit = &digits[decision_index as usize];
	    image::imageops::overlay(&mut output, digit,
				     (max_digit_width + (x - digit.width()))
				     as i64,
				     vertical_pos as i64);
	    decision_cutoff = u32::MAX;
	    decision_index = -1;
	    decision_score = 0.0;
	}
    }
    // The final chosen match.
    output.save("output.png")?;

    Ok(())
}
