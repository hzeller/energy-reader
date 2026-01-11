use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, UNIX_EPOCH};

mod cross_correlator;
use cross_correlator::{ColumnFeatureScore, CrossCorrelator};

mod image_util;
use image_util::{ImageOp, apply_ops, load_image_as_grayscale, sobel};

mod debugdigit;

#[cfg(feature = "debug_timing")]
mod scoped_timer;

#[cfg(feature = "debug_timing")]
pub use scoped_timer::ScopedTimer;

#[cfg(not(feature = "debug_timing"))]
mod empty_scoped_timer;
#[cfg(not(feature = "debug_timing"))]
pub use empty_scoped_timer::ScopedTimer;

// Where images are coming from ...
mod sources;
use sources::{FilenameSource, ImageSource, TimestampedImage, WebCamSource};

// ... and the acquired values are sent to.
mod sinks;
use sinks::{ResultSink, StdOutSink};

// Minimum feature threshold to consider robust digit detection.
const THRESHOLD: f32 = 0.6;

// Plausibility checks. If a digit is missing, that would be aoubt 100% off, so
// 40% makes sure digits (even with a bit of jitter) are contiguous.
const ALLOWED_DIGIT_DISTANCE_JITTER_PERCENT: f32 = 40.0;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// Capture counter image from webcam.
    #[arg(long)]
    webcam: bool,

    /// Read counter image from file.
    #[arg(long, value_name = "png-file")]
    filename: Option<PathBuf>,

    /// Image operation to apply after image is acquired.
    /// One of ["rotate90", "rotate180", "flip-x", "flip-y", "crop:<x>:<y>:<w>:<h>"].
    /// Multiple --op are applied in sequence provided on command line.
    #[arg(long = "op", value_name = "op")]
    process_ops: Vec<ImageOp>,

    /// Process input images through sobel edge-detect. Can improve accuracy
    /// with very clean and non-distorted images.
    #[arg(long = "sobel", default_value = "false")]
    edge_process: bool,

    /// Number of digits to OCR verify and emit. Good to limit if the last
    /// digit is finicky due to roll-over.
    #[arg(long, value_name = "#", default_value = "7")]
    emit_count: usize,

    /// Maximum plausible value change per second to avoid logging bogus
    /// values.
    #[arg(long, value_name = "count/sec", default_value = "0.1")]
    max_plausible_rate: f32,

    /// Repeat every these number of seconds (useful with --webcam)
    #[arg(long, value_name = "seconds")]
    repeat_sec: Option<u64>,

    /// Output the image captured.
    /// If existing directory, writes snap-<timestemp>.png images, otherwise
    /// intepreted as filename.
    #[arg(long, value_name = "file-or-dir")]
    debug_capture: Option<PathBuf>,

    /// Output the image after the process ops have been applied.
    /// If existing directory, writes processed-<timestemp>.png images,
    /// otherwise intepreted as filename.
    #[arg(long, value_name = "file-or-dir")]
    debug_post_ops: Option<PathBuf>,

    /// Output image that could not detect all digits.
    /// If existing directory, writes fail-<timestemp>.png images, otherwise
    /// intepreted as filename.
    #[arg(long, value_name = "file-or-dir")]
    failed_capture: Option<PathBuf>,

    /// Generate a debug image that illustrates the detection details.
    #[arg(long, value_name = "img-file")]
    debug_scoring: Option<PathBuf>,

    /// Digit template images to match; the first digit found in the filename
    /// is the matched digit. Allows to have multiple templates for the same
    /// digit if needed (e.g. d1-0.png, d1-1.png).
    digit_images: Vec<PathBuf>,
}

/// Detection output: the digit template detected with associated infor.
#[derive(Clone)]
pub struct DigitPos {
    digit_template: u32,
    score: f32,
    pos: u32,
}

// Find the hightest score digits and emit their positions.
fn locate_digits(scores: &[ColumnFeatureScore], digit_width: u32) -> Vec<DigitPos> {
    let x_range = scores.iter().map(|v| v.len()).min().unwrap_or(0) as u32;
    let mut result = Vec::new();

    let mut current_best: Option<DigitPos> = None;

    for x in 0..x_range {
        // Find the best template for this specific X position
        let best_at_x = scores
            .iter()
            .enumerate()
            .map(|(i, score_vec)| (i, score_vec[x as usize]))
            .filter(|&(_, score)| score >= THRESHOLD)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        if let Some((template_idx, score)) = best_at_x
            && current_best.as_ref().is_none_or(|c| score > c.score)
        {
            current_best = Some(DigitPos {
                digit_template: template_idx as u32,
                score,
                pos: x,
            });
        }

        // Check if we've passed the width of the current best digit
        if let Some(best) = &current_best
            && x >= best.pos + digit_width
        {
            result.push(best.clone());
            current_best = None;
        }
    }

    if let Some(best) = current_best {
        result.push(best);
    }
    result
}

fn verify_looks_plausible(locations: &[DigitPos], expect_count: usize) -> Result<()> {
    if locations.len() < 2 {
        return Err(anyhow!("Not even two digits"));
    }
    const LO_ALLOW: f32 = 1.0 - ALLOWED_DIGIT_DISTANCE_JITTER_PERCENT / 100.0;
    const HI_ALLOW: f32 = 1.0 + ALLOWED_DIGIT_DISTANCE_JITTER_PERCENT / 100.0;
    let mut last_delta = (locations[1].pos - locations[0].pos) as f32;
    for i in 2..locations.len() {
        let now_delta = (locations[i].pos - locations[i - 1].pos) as f32;
        let fraction = now_delta / last_delta;
        if !(LO_ALLOW..=HI_ALLOW).contains(&fraction) {
            return Err(anyhow!(
                "Digit distance before {:.0}, now {:.0} ({:.1}%) is more than expected ±{}% off.",
                last_delta,
                now_delta,
                100.0 * fraction,
                ALLOWED_DIGIT_DISTANCE_JITTER_PERCENT
            ));
        }
        last_delta = now_delta;
    }
    // We do this last, as the above loop might more specifically point out
    // 'holes'
    if locations.len() < expect_count {
        return Err(anyhow!(
            "Got {} digits, but expected {}",
            locations.len(),
            expect_count
        ));
    }
    Ok(())
}

fn extract_number(
    locations: &[DigitPos],
    digit_filenames: &[PathBuf],
    expect_count: usize,
) -> Result<u64> {
    verify_looks_plausible(locations, expect_count)?;
    let get_first_digit_from = |f: &PathBuf| -> Result<u64> {
        Ok(f.file_name()
            .ok_or(anyhow!("invalid filename"))?
            .to_string_lossy()
            .chars()
            .find(|c| c.is_ascii_digit())
            .and_then(|c| c.to_digit(10))
            .ok_or_else(|| anyhow!("Filename {:?} must contain a digit", f))? as u64)
    };
    // Go from left to right, assembling the decimal number
    locations.iter().take(expect_count).try_fold(0, |acc, loc| {
        let filename = &digit_filenames[loc.digit_template as usize];
        Ok(acc * 10 + get_first_digit_from(filename)?)
    })
}

fn maybe_debug_image(file_or_dir: &Option<PathBuf>, prefix: &str, ts_img: &TimestampedImage) {
    let ts = ts_img
        .timestamp
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Some(path) = file_or_dir {
        let img_file = if path.is_dir() {
            &path.join(format!("{}-{}.png", prefix, ts))
        } else {
            path
        };
        let _ = ts_img
            .image
            .save(img_file)
            .context("failed to save debug capture");
    }
}

// Params: utility-reader <counter-image> <digit0> <digit1>...
fn main() -> ExitCode {
    let args = CliArgs::parse();

    let source: Box<dyn ImageSource> = if let Some(file) = args.filename {
        Box::new(FilenameSource::new(file))
    } else if args.webcam {
        Box::new(WebCamSource {})
    } else {
        eprintln!("Need one of --filename or --webcam");
        return ExitCode::FAILURE;
    };

    let mut logger = StdOutSink::new(args.max_plausible_rate);

    let mut digits = Vec::new();
    for digit_picture in &args.digit_images {
        let digit = load_image_as_grayscale(digit_picture);
        let digit = if args.edge_process {
            sobel(&digit)
        } else {
            digit
        };
        digits.push(digit);
    }
    let max_digit_w = digits.iter().map(|d| d.width()).max().unwrap_or(0);
    let max_digit_h = digits.iter().map(|d| d.height()).max().unwrap_or(0);

    let mut correlator: Option<CrossCorrelator> = None;
    loop {
        let mut captured = match source.read_image() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Trouble capturing: {}", e);
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        maybe_debug_image(&args.debug_capture, "snap", &captured);

        if let Err(e) = apply_ops(&mut captured.image, &args.process_ops) {
            eprintln!("Check your image ops: {e:#}");
            return ExitCode::FAILURE;
        }
        maybe_debug_image(&args.debug_post_ops, "processed", &captured);

        let haystack = if args.edge_process {
            &sobel(&captured.image)
        } else {
            &captured.image
        };

        let corr = correlator.get_or_insert_with(|| {
            let mut c = CrossCorrelator::new(
                haystack.width() + max_digit_w,
                haystack.height() + max_digit_h,
            );
            for digit_needle in &digits {
                // First time: add all needles.
                c.add_needle(digit_needle);
            }
            c
        });

        let digit_scores = corr.calculate_needle_scores_for(haystack);
        let digit_locations = locate_digits(&digit_scores, max_digit_w);

        if let Some(ref debug_scoring) = args.debug_scoring {
            debugdigit::debug_print_digits(
                haystack,
                &digits,
                max_digit_w,
                max_digit_h,
                &digit_scores,
                &digit_locations,
                &args.digit_images,
            )
            .save(debug_scoring)
            .context("While saving --debug-scoring image")
            .unwrap();
        }

        let result = extract_number(&digit_locations, &args.digit_images, args.emit_count);
        let current_exit_code = match result {
            Ok(meter_value) => {
                logger.log_value(captured.timestamp, meter_value);
                ExitCode::SUCCESS
            }

            Err(e) => {
                logger.log_error(captured.timestamp, &e.to_string());
                maybe_debug_image(&args.failed_capture, "fail", &captured);
                ExitCode::FAILURE
            }
        };

        match args.repeat_sec {
            Some(sec) => {
                std::thread::sleep(Duration::from_secs(sec));
            }
            None => break current_exit_code,
        };
    }
}
