use image::{DynamicImage,GrayImage,ImageBuffer,Luma};
use std::env;
use std::cmp;

fn load_as_grayscale(path : &str) -> GrayImage {
    let rgba = image::open(path).unwrap().into_rgba8();
    DynamicImage::ImageRgba8(rgba).into_luma8()
}

type ProcessingImage = ImageBuffer::<Luma<f32>, Vec<f32>>;
fn normalize(img : &ProcessingImage) -> GrayImage {
    let mut result = GrayImage::new(img.width(), img.height());
    let min = img.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let range = img.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)) - min;
    println!("min:{}, range:{}", min, range);
    // TODO: Do some sort of map()
    for ix in 0..img.width() {
	for iy in 0..img.height() {
	    let p = img.get_pixel(ix, iy)[0];
	    let zero_to_one = (p - min) / range;
	    let normalized = [(255.0 * zero_to_one) as u8; 1];
	    result.put_pixel(ix, iy, Luma::<u8>::from(normalized));
	}
    }
    result
}

fn graph(img : &ProcessingImage) -> GrayImage {
    let mut result = GrayImage::new(img.width(), img.height());
    let min = img.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max = img.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b)) - min;
    let range = max - min;
    // TODO: Do some sort of map()
    let white = Luma::<u8>::from([255;1]);
    for ix in 0..img.width() {
	let mut highest_column_value : f32 = 0.0;
	let mut has_high_sum = false;
	for iy in 0..img.height() {
	    let p = img.get_pixel(ix, iy)[0];
	    has_high_sum |= p == max;
	    let zero_to_one = (p - min) / range;
	    highest_column_value = highest_column_value.max(zero_to_one);
	}

	result.put_pixel(ix,
			 (img.height() - 1) - (highest_column_value * (img.height() - 1) as f32) as u32,
			 white);
	if has_high_sum && false {
	    for y in 0..img.height() {
		result.put_pixel(ix, y, white);
	    }
	}
    }
    result
}

fn cross_correlate(haystack: &GrayImage, needle: &GrayImage) -> GrayImage {
    let mut output = ProcessingImage::new(haystack.width(),
					  haystack.height());
    for hx in 0..haystack.width() - needle.width() {
	for hy in 0..haystack.height() - needle.height() {
	    let mut value : [f32; 1] = [0.0; 1];
	    for x in 0..needle.width() {
		for y in 0..needle.height() {
		    value[0] += needle.get_pixel(x, y)[0] as f32 *
			haystack.get_pixel(hx+x, hy+y)[0] as f32;
		}
	    }
	    output.put_pixel(hx, hy, Luma::<f32>::from(value));
	}
    }
    //normalize(&output)
    graph(&output)
}

// Params: energy-reader image <digit0> <digit1>...
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let haystack_raw = load_as_grayscale(env::args().nth(1).unwrap().as_str());
    let haystack = edge_detection::canny(
	    haystack_raw,
	    2.0,  // sigma
	    0.2,  // strong threshold
	    0.01, // weak threshold
	    ).as_image().into_luma8();

    let mut max_digit_width = 0;
    let mut digits = Vec::new();
    for picture in env::args().skip(2) {
	let digit_raw = load_as_grayscale(picture.as_str());
	let digit = edge_detection::canny(
	    digit_raw,
	    2.0,  // sigma
	    0.2,  // strong threshold
	    0.01, // weak threshold
	    ).as_image().into_luma8();
	max_digit_width = cmp::max(max_digit_width, digit.width());
	digits.push(digit);
    }

    // Overlay the found digits.
    //image::imageops::overlay(&mut haystack, &digits[0], 50, 16);


    let mut output = GrayImage::new(max_digit_width + haystack.width(),
				    (digits.len() as u32+2) * haystack.height());
    // Original on top
    image::imageops::overlay(&mut output, &haystack, max_digit_width as i64, 0);

    // the heatmap of digit matches
    let mut vertical_pos = haystack.height();
    for digit in digits.iter() {
	let xcorr = cross_correlate(&haystack, digit);
	image::imageops::overlay(&mut output, digit, 0,
				 vertical_pos as i64);
	image::imageops::overlay(&mut output, &xcorr, max_digit_width as i64,
				 vertical_pos as i64);

	vertical_pos +=  haystack.height();
    }

    // The final chosen match.
    output.save("output.png")?;

    Ok(())
}
