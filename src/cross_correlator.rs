use rustfft::{FftPlanner, num_complex::Complex, FftDirection};
use image::GrayImage;

use crate::ScopedTimer;

pub type ColumnFeatureScore = Vec<f32>;

struct ImageFFT {
    freq_domain: Vec<Complex<f32>>,
    width: u32,
    height: u32,
}

impl ImageFFT {
    fn new(image: &GrayImage, average: f32, padded_w: usize,
           padded_h: usize, planner: &mut FftPlanner<f32>) -> Self {
        let mut fft = vec![Complex::default(); padded_w * padded_h];
        for (y, row) in image.rows().enumerate() {
            let offset = y * padded_w;
            for (x, pixel) in row.enumerate() {
                fft[offset + x] = Complex::new(pixel[0] as f32 - average, 0.0);
            }
        }
        fft_2d(&mut fft, padded_w, padded_h, planner, FftDirection::Forward);
        Self {
            freq_domain: fft,
            width: image.width(),
            height: image.height(),
        }
    }
}

struct PreparedNeedle {
    fft: ImageFFT,
    pixel_count: f32,
    std_dev: f32,
}

pub struct CrossCorrelator {
    padded_width: usize,
    padded_height: usize,

    needles: Vec<PreparedNeedle>,

    planner: FftPlanner<f32>,  // Caches decisions
}

// Use FFT and the integral image to produce a normalized cross correlation.
impl CrossCorrelator {
    /// Create a new cross correlator for given size (haystack+needle dimensions)
    pub fn new(fft_width: u32, fft_height: u32)-> CrossCorrelator {
        let planner = FftPlanner::new();
         Self {
            padded_width: fft_width as usize,
            padded_height: fft_height as usize,
            needles: Vec::new(),
            planner,
        }
    }

    /// Add needle the haystack is checked against. The cross-correlate
    /// function considers all these needles.
    pub fn add_needle(&mut self, needle: &GrayImage) {
        let pixel_count = (needle.width() * needle.height()) as f32;
        let n_sum: f32 = needle.iter().map(|&p| p as f32).sum();
        let n_avg = n_sum / pixel_count;

        let n_sq_diff_sum: f32 = needle
            .iter()
            .map(|&p| {
                let diff = p as f32 - n_avg;
                diff * diff
            })
            .sum();
        let std_dev = n_sq_diff_sum.sqrt();

        self.needles.push(PreparedNeedle {
            fft: ImageFFT::new(needle, n_avg, self.padded_width,
                               self.padded_height, &mut self.planner),
            pixel_count,
            std_dev,
        })
    }

    /// Given a haystack, run cross correlation with all added needles,
    /// and emit a feature score for each.
    pub fn calculate_needle_scores_for(&mut self, haystack: &GrayImage)
                                       -> Vec<ColumnFeatureScore> {
        let haystack_fft = ImageFFT::new(haystack, 0.0,
                                         self.padded_width, self.padded_height,
                                         &mut self.planner);
        let haystack_integral = IntegralImages::new(haystack);
        let (w, h) = (self.padded_width, self.padded_height);

        let mut results = Vec::with_capacity(self.needles.len());
        let mut workspace = vec![Complex::default(); w * h];

        for needle in &self.needles {
            workspace.iter_mut()
                .zip(&haystack_fft.freq_domain)
                .zip(&needle.fft.freq_domain)
                .for_each(|((out, h_val), n_val)| {
                    *out = h_val * n_val.conj();
                });
            fft_2d(&mut workspace, w, h, &mut self.planner, FftDirection::Inverse);

            let (nw, nh) = (needle.fft.width as usize, needle.fft.height as usize);
            let fft_norm = (w * h) as f32;

            let _ = ScopedTimer::new("collect score");
            let score = (0..(haystack_fft.width - needle.fft.width))
                .map(|x| {
                    let x = x as usize;
                    // Find max score in this column
                    (0..(haystack_fft.height - needle.fft.height) as usize)
                        .map(|y| {
                            let numerator = workspace[y * w + x].re / fft_norm;
                            let (sum, sum_sq) = haystack_integral.get_window_stats(x, y, nw, nh);

                            let h_var = (sum_sq - (sum * sum) / needle.pixel_count).max(0.0);
                            let denom = needle.std_dev * h_var.sqrt();

                            if denom > 1e-6 { (numerator / denom).clamp(-1.0, 1.0) } else { 0.0 }
                        })
                        .fold(0.0f32, |max, score| max.max(score)) // find the max in this column.
                })
                .collect();
            results.push(score);
        }
        results
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

        for (y, row) in img.rows().enumerate() {
            let mut row_sum = 0u64;
            let mut row_sum_sq = 0u64;
            for (x, pixel) in row.enumerate() {
                let pixel = pixel[0] as u64;

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
    let _ = ScopedTimer::new("fft_2d()");
    let fft_row = planner.plan_fft(width, direction);
    data.chunks_exact_mut(width).for_each(|row| fft_row.process(row));

    let fft_col = planner.plan_fft(height, direction);
    let mut column = vec![Complex::default(); height];
    for x in 0..width {
        // slice through data and extract the column.
        for y in 0..height {
            column[y] = data[y * width + x];
        }

        fft_col.process(&mut column);

        for y in 0..height {
            data[y * width + x] = column[y];
        }
    }
}
