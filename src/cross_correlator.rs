use rustfft::{FftPlanner, num_complex::Complex, FftDirection};
use image::GrayImage;

pub type ColumnFeatureScore = Vec<f32>;

pub struct CrossCorrelator<'a> {
    haystack: &'a GrayImage,
    fft_width: usize,
    fft_height: usize,
    haystack_fft: Vec<Complex<f32>>,
    haystack_integral: IntegralImages,
}

// Use FFT and the integral image to produce a normalized cross correlation.
impl<'a> CrossCorrelator<'a> {
    // Create a new cross correlator for given search image (the 'haystack') that
    // then provides an efficient way to cross correlate with elements found inside.
    pub fn new(haystack: &'a GrayImage,
	       max_needle_width: u32, max_needle_height: u32)
	       -> CrossCorrelator<'a> {
	let haystack_integral = IntegralImages::new(haystack);
	let fft_width = (haystack.width() + max_needle_width) as usize;
	let fft_height = (haystack.height() + max_needle_height) as usize;
	let mut haystack_fft = vec![Complex::new(0.0, 0.0); fft_width * fft_height];
	haystack_fft
            .chunks_exact_mut(fft_width)
            .zip(haystack.rows())
            .for_each(|(padded_row, image_row)| {
		padded_row
                    .iter_mut()
                    .zip(image_row)
                    .for_each(|(target, pixel)| {
			*target = Complex::new(pixel[0] as f32, 0.0);
                    });
            });
	let mut planner = FftPlanner::new();
	fft_2d(&mut haystack_fft, fft_width, fft_height, &mut planner, FftDirection::Forward);

	Self {
	    haystack,
	    fft_width,
	    fft_height,
	    haystack_fft,
	    haystack_integral,
	}
    }

    // Cross correlate the haystack with "needle" and provide a needls score
    // over the length of the haystack.
    pub fn cross_correlate_with(&self, needle: &GrayImage)
				-> ColumnFeatureScore {
	let n_pixels = (needle.width() * needle.height()) as f32;
	let n_sum: f32 = needle.iter().map(|&p| p as f32).sum();
	let n_avg = n_sum / n_pixels;

	let (w, h) = (self.fft_width, self.fft_height);
	let mut n_space = vec![Complex::new(0.0, 0.0); w * h];
	n_space
            .chunks_exact_mut(self.fft_width)
            .zip(needle.rows())
            .for_each(|(padded_row, image_row)| {
		padded_row
                    .iter_mut()
                    .zip(image_row)
                    .for_each(|(target, pixel)| {
			*target = Complex::new(pixel[0] as f32 - n_avg, 0.0);
                    });
            });

	// Don't touch preprocessed haystack_fft, do all modifications in local n_space
	let mut planner = FftPlanner::new();
	fft_2d(&mut n_space, w, h, &mut planner, FftDirection::Forward);
	n_space.iter_mut().zip(self.haystack_fft.iter()).for_each(|(n, h)| {
            *n = h * n.conj();
	});
	fft_2d(&mut n_space, w, h, &mut planner, FftDirection::Inverse);

	let (nw, nh) = (needle.width() as usize, needle.height() as usize);

	// Prepare needle variance for normalized cross correlation
	let n_sq_diff_sum: f32 = needle
            .iter()
            .map(|&p| {
		let diff = p as f32 - n_avg;
		diff * diff
            })
            .sum();
	let n_std_dev = n_sq_diff_sum.sqrt();

	let fft_norm = (w * h) as f32;
	(0..(self.haystack.width() - needle.width()))
            .map(|x| {
		let x = x as usize;
		// Find max score in this column
		(0..(self.haystack.height() - needle.height()) as usize)
                    .map(|y| {
			let numerator = n_space[y * w + x].re / fft_norm;
			let (sum, sum_sq) = self.haystack_integral.get_window_stats(x, y, nw, nh);

			let h_var = (sum_sq - (sum * sum) / n_pixels).max(0.0);
			let denom = n_std_dev * h_var.sqrt();

			if denom > 1e-6 { numerator / denom } else { 0.0 }
                    })
                    .fold(0.0f32, |max, score| max.max(score)) // find the max in this column.
            })
            .collect()
    }
}

struct IntegralImages {
    sum: Vec<u64>,
    sum_sq: Vec<u64>,
    width: usize,
}

impl IntegralImages {
    fn new(img: &GrayImage) -> Self {
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

    // (sum, sum_of_squares) for rect at (x, y) with size (nw, nh)
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

fn fft_2d(data: &mut [Complex<f32>], width: usize, height: usize,
	  planner: &mut FftPlanner<f32>, direction: FftDirection) {
    // Rows: process each chunk directly
    let fft_row = planner.plan_fft(width, direction);
    data.chunks_exact_mut(width).for_each(|row| {
        fft_row.process(row);
    });

    // Extract columns, then operate on these
    let fft_col = planner.plan_fft(height, direction);
    for x in 0..width {
        let mut column: Vec<_> = (0..height).map(|y| data[y * width + x]).collect();

        fft_col.process(&mut column);

        column.into_iter().enumerate().for_each(|(y, val)| {
            data[y * width + x] = val;
        });
    }
}
