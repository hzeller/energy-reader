use image::{DynamicImage, GrayImage, Luma};
use rustfft::{FftPlanner, num_complex::Complex};
use std::cmp;
use std::env;

#[cfg(feature = "debug_img")]
mod debugdigit;

pub const THRESHOLD: f32 = 0.6;

pub struct DigitPos {
    digit: u32,
    score: f32,
    pos: u32,
}
pub type ColumnFeatureScore = Vec<f32>;

fn load_image_as_grayscale(path: &str) -> GrayImage {
    let rgba = image::open(path).unwrap().into_rgba8();
    DynamicImage::ImageRgba8(rgba).into_luma8()
}

struct IntegralImages {
    sum: Vec<u64>,
    sum_sq: Vec<u64>,
    width: usize,
}

fn create_integral_images(img: &GrayImage) -> IntegralImages {
    let (w, h) = (img.width() as usize, img.height() as usize);
    // We size it (w+1)x(h+1) to handle boundaries (zeros in the first row/col)
    let mut sum = vec![0u64; (w + 1) * (h + 1)];
    let mut sum_sq = vec![0u64; (w + 1) * (h + 1)];

    for y in 0..h {
        let mut row_sum = 0u64;
        let mut row_sum_sq = 0u64;
        for x in 0..w {
            let pixel = img.get_pixel(x as u32, y as u32)[0] as u64;

            row_sum += pixel;
            row_sum_sq += pixel * pixel;

            let idx = (y + 1) * (w + 1) + (x + 1);
            let above_idx = y * (w + 1) + (x + 1);

            sum[idx] = row_sum + sum[above_idx];
            sum_sq[idx] = row_sum_sq + sum_sq[above_idx];
        }
    }
    IntegralImages {
        sum,
        sum_sq,
        width: w + 1,
    }
}

impl IntegralImages {
    // Returns (sum, sum_of_squares) for a rectangle at (x, y) with size (nw, nh)
    fn get_window_stats(&self, x: usize, y: usize, nw: usize, nh: usize) -> (f32, f32) {
        let x2 = x + nw;
        let y2 = y + nh;

        let get_val = |data: &[u64], px: usize, py: usize| data[py * self.width + px] as f32;

        let s = get_val(&self.sum, x2, y2) - get_val(&self.sum, x, y2) - get_val(&self.sum, x2, y)
            + get_val(&self.sum, x, y);

        let s_sq = get_val(&self.sum_sq, x2, y2)
            - get_val(&self.sum_sq, x, y2)
            - get_val(&self.sum_sq, x2, y)
            + get_val(&self.sum_sq, x, y);

        (s, s_sq)
    }
}

// Determine score of given needle pattern existing at given haystack column.
// Use FFT and the integral image to produce a normalized cross correlation.
fn cross_correlate_ncc_fft(haystack: &GrayImage, needle: &GrayImage) -> ColumnFeatureScore {
    let (nw, nh) = (needle.width() as usize, needle.height() as usize);
    let n_pixels = (nw * nh) as f32;

    // 1. Prepare Zero-Mean Needle
    let n_sum: f32 = needle.iter().map(|&p| p as f32).sum();
    let n_avg = n_sum / n_pixels;

    // Calculate needle variance
    let n_sq_diff_sum: f32 = needle
        .iter()
        .map(|&p| {
            let diff = p as f32 - n_avg;
            diff * diff
        })
        .sum();
    let n_std_dev = n_sq_diff_sum.sqrt();

    // 2. Prepare Integral Images for Haystack
    let integral = create_integral_images(haystack);

    // 3. FFT Setup (Padding)
    let width = (haystack.width() + needle.width()).next_power_of_two() as usize;
    let height = (haystack.height() + needle.height()).next_power_of_two() as usize;

    let mut n_space = vec![Complex::new(0.0, 0.0); width * height];
    n_space
        .chunks_exact_mut(width)
        .zip(needle.rows())
        .for_each(|(padded_row, image_row)| {
            // For each pair...
            padded_row
                .iter_mut() // Iterate over the padded row
                .zip(image_row) // Zip with pixels in the image row
                .for_each(|(target, pixel)| {
                    *target = Complex::new(pixel[0] as f32 - n_avg, 0.0);
                });
        });

    let mut h_space = vec![Complex::new(0.0, 0.0); width * height];
    h_space
        .chunks_exact_mut(width)
        .zip(haystack.rows())
        .for_each(|(padded_row, image_row)| {
            padded_row
                .iter_mut()
                .zip(image_row)
                .for_each(|(target, pixel)| {
                    *target = Complex::new(pixel[0] as f32, 0.0);
                });
        });

    // 4. Perform FFTs & Conjugate Multiplication
    let mut planner = FftPlanner::new();
    fft_2d(&mut h_space, width, height, &mut planner, false);
    fft_2d(&mut n_space, width, height, &mut planner, false);

    h_space.iter_mut().zip(n_space.iter()).for_each(|(h, n)| {
        *h *= n.conj();
    });

    fft_2d(&mut h_space, width, height, &mut planner, true);

    // 5. Normalization and extracting max in column as feature score
    let fft_norm = (width * height) as f32;
    (0..(haystack.width() - needle.width()))
        .map(|x| {
            let x = x as usize;
            // Find max score in this column
            (0..(haystack.height() - needle.height()) as usize)
                .map(|y| {
                    let numerator = h_space[y * width + x].re / fft_norm;
                    let (sum, sum_sq) = integral.get_window_stats(x, y, nw, nh);

                    let h_var = (sum_sq - (sum * sum) / n_pixels).max(0.0);
                    let denom = n_std_dev * h_var.sqrt();

                    if denom > 1e-6 { numerator / denom } else { 0.0 }
                })
                .fold(0.0f32, |max, score| max.max(score)) // Efficiently find the max
        })
        .collect()
}

fn fft_2d(
    data: &mut [Complex<f32>],
    width: usize,
    height: usize,
    planner: &mut FftPlanner<f32>,
    inverse: bool,
) {
    let fft_row = if inverse {
        planner.plan_fft_inverse(width)
    } else {
        planner.plan_fft_forward(width)
    };

    // Rows: process each chunk directly
    data.chunks_exact_mut(width).for_each(|row| {
        fft_row.process(row);
    });

    // Columns: Use iterators to pull and push column data
    let fft_col = if inverse {
        planner.plan_fft_inverse(height)
    } else {
        planner.plan_fft_forward(height)
    };

    for x in 0..width {
        let mut column: Vec<_> = (0..height).map(|y| data[y * width + x]).collect();

        fft_col.process(&mut column);

        column.into_iter().enumerate().for_each(|(y, val)| {
            data[y * width + x] = val;
        });
    }
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
            let gx = -nw         + ne
		+    (-2 * west) + (2 * east)
		+    -sw         + se;

            #[rustfmt::skip]
            let gy = -nw + (-2 * north) + -ne
		+     sw + ( 2 * south) +  se;

            let mut mag = ((gx as f32).powi(2) + (gy as f32).powi(2)).sqrt();

            mag = mag.clamp(0.0, 255.0);

            result.put_pixel(x, y, Luma([mag as u8]));
        }
    }
    result
}

// Find the hightest score digits and emit their positions.
fn locate_digits(scores: &[ColumnFeatureScore], picture_width: u32,
		 digit_width: u32) -> Vec<DigitPos> {
    let mut result = Vec::new();
    let mut current = DigitPos {
        digit: u32::MAX,
        score: 0.0,
        pos: picture_width,
    };
    for x in 0..picture_width - digit_width {
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

        if x >= current.pos + digit_width {
            // best seen for digit-width
            result.push(current);
            current = DigitPos {
                digit: u32::MAX,
                score: 0.0,
                pos: picture_width,
            };
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


    let mut digit_scores: Vec<ColumnFeatureScore> = Vec::new();
    for digit in digits.iter() {
        let highest_column = cross_correlate_ncc_fft(&haystack, digit);
        digit_scores.push(highest_column);
    }

    // Output to stdout for further processing.
    let digit_locations = locate_digits(&digit_scores, haystack.width(), max_digit_width);
    for loc in &digit_locations {
        println!(
            "{} {} {:4} {:.3}",
            loc.digit,
            env::args()
                .nth((loc.digit + 2) as usize)
                .expect("should be valid arg"),
            loc.pos,
            loc.score
        );
    }

    #[cfg(feature = "debug_img")]
    debugdigit::debug_print_digits(
        &haystack,
        &digits,
        max_digit_width,
        &digit_scores,
        &digit_locations,
    )
    .save("output.png")
    .unwrap();
}
