use anyhow::Result;
use clap::Parser;
use std::cmp;
use std::path::Path;
use std::process::ExitCode;
use std::time::{UNIX_EPOCH, Duration};

mod cross_correlator;
use cross_correlator::{CrossCorrelator, ColumnFeatureScore};

mod image_util;
use image_util::{sobel, load_image_as_grayscale, apply_ops};

mod debugdigit;

// Where images are coming from ...
mod sources;
use sources::{ImageSource, FilenameSource, WebCamSource};

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
    #[arg(long, value_name="png-file")]
    filename: Option<String>,

    /// Image operations to apply (in that sequence) after image is acquired.
    /// One of ["rotate90", "rotate180", "crop:<x>:<y>:<w>:<h>"]
    #[arg(long="op", value_name="op")]
    process_ops: Vec<String>,

    /// Process input images through sobel edge-detect. Can improve accuracy
    /// with very clean and non-distorted images.
    #[arg(long = "sobel", default_value="false")]
    edge_process: bool,

    /// Number of digits to OCR verify and emit. Good to limit if the last
    /// digit is finicky due to roll-over.
    #[arg(long, value_name="#", default_value="7")]
    emit_count: usize,

    /// Repeat every these number of seconds (useful with --webcam)
    #[arg(long, value_name="seconds")]
    repeat_sec: Option<u64>,

    /// Output the image captured.
    #[arg(long, value_name="img-file")]
    debug_capture: Option<String>,

    /// Output the image after the process ops have been applied.
    #[arg(long, value_name="img-file")]
    debug_post_ops: Option<String>,

    /// Generate a debug image that illustrates the detection details.
    #[arg(long, value_name="img-file")]
    debug_scoring: Option<String>,

    /// Directory to store images that could not detect all digits.
    #[arg(long, value_name="dir")]
    failed_capture_dir: Option<String>,

    /// Digit template images to match; the first digit found in the filename
    /// is the matched digit. Allows to have multiple templates for the same
    /// digit if needed (e.g. d1-0.png, d1-1.png).
    digit_images: Vec<String>,
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
        let best_at_x = scores.iter().enumerate()
            .map(|(i, score_vec)| (i, score_vec[x as usize]))
            .filter(|&(_, score)| score >= THRESHOLD)
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        if let Some((template_idx, score)) = best_at_x {
            if current_best.as_ref().map_or(true, |c| score > c.score) {
                current_best = Some(DigitPos {
                    digit_template: template_idx as u32,
                    score,
                    pos: x,
                });
            }
        }

        // Check if we've passed the width of the current best digit
        if let Some(best) = &current_best {
            if x >= best.pos + digit_width {
                result.push(best.clone());
                current_best = None;
            }
        }
    }

    if let Some(best) = current_best { result.push(best); }
    result
}

fn verify_looks_plausible(locations: &[DigitPos],
                          expect_count: usize) -> Result<(), String> {
    if locations.len() < 2 {
        return Err("Not even two digits".to_string());
    }
    const LO_ALLOW: f32 = 1.0 - ALLOWED_DIGIT_DISTANCE_JITTER_PERCENT / 100.0;
    const HI_ALLOW: f32 = 1.0 + ALLOWED_DIGIT_DISTANCE_JITTER_PERCENT / 100.0;
    let mut last_delta = (locations[1].pos - locations[0].pos) as f32;
    for i in 2..locations.len() {
        let now_delta = (locations[i].pos - locations[i-1].pos) as f32;
        let fraction = now_delta / last_delta;
        if !(LO_ALLOW ..= HI_ALLOW).contains(&fraction) {
            return Err(format!(
                "Digit distance before {:.0}, now {:.0} ({:.1}%) is more than expected ±{}% off.",
                last_delta, now_delta, 100.0 * fraction, ALLOWED_DIGIT_DISTANCE_JITTER_PERCENT));
        }
        last_delta = now_delta;
    }
    // We do this last, as the above loop might more specifically point out
    // 'holes'
    if locations.len() < expect_count {
        return Err(format!("Got {} digits, but expected {}",
                           locations.len(), expect_count));
    }
    Ok(())
}

fn extract_number(locations: &[DigitPos], digit_filenames: &[String],
                  expect_count: usize)
                  -> anyhow::Result<u64, String> {
    verify_looks_plausible(locations, expect_count)?;
    let mut result: u64 = 0;
    for loc in &locations[0..expect_count] {
        let filename = &digit_filenames[loc.digit_template as usize];
        let first_digit = filename.chars().find(|c| c.is_ascii_digit())
            .ok_or("Digit filename needs to contain the digit it represents")?;
        let c = (first_digit as u8 - b'0') as u64;
        result = 10 * result + c;
    }
    Ok(result)
}

// Params: energy-reader <counter-image> <digit0> <digit1>...
fn main() -> ExitCode {
    let args = CliArgs::parse();

    if let Some(ref failed_capture_dir) = args.failed_capture_dir {
        if !Path::new(failed_capture_dir).is_dir() {
            eprintln!("'{}' needs to be an existing dir for --failed_capture_dir",
                      args.failed_capture_dir.as_ref().unwrap());
            return ExitCode::FAILURE;
        }
    }

    let source: Box<dyn ImageSource> = if let Some(file) = args.filename {
        Box::new(FilenameSource::new(file))
    } else if args.webcam {
        Box::new(WebCamSource{})
    } else {
        eprintln!("Need one of --filename or --webcam");
        return ExitCode::FAILURE;
    };

    let logger = StdOutSink{};

    let mut max_digit_w = 0;
    let mut max_digit_h = 0;
    let mut digits = Vec::new();
    for digit_picture in &args.digit_images {
        let digit = load_image_as_grayscale(digit_picture.as_str());
        let digit = if args.edge_process {
            sobel(&digit)
        } else {
            digit
        };
        max_digit_w = cmp::max(max_digit_w, digit.width());
        max_digit_h = cmp::max(max_digit_h, digit.height());
        digits.push(digit);
    }

    let mut last_success;

    loop {
        let mut captured = match source.read_image() {
            Ok(captured) => captured,
            Err(e) => {
                eprintln!("Trouble capturing: {}", e);
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        if let Some(ref debug_capture) = args.debug_capture {
            captured.image.save(debug_capture).unwrap();
        }
        if let Err(e) = apply_ops(&mut captured.image, &args.process_ops) {
            eprintln!("Check your image ops: {e:#}");
            return ExitCode::FAILURE;
        }
        if let Some(ref post_op_file) = args.debug_post_ops {
            captured.image.save(post_op_file).unwrap();
        }

        let haystack = &captured.image;
        let haystack = if args.edge_process {
            &sobel(haystack)
        } else {
            haystack
        };

        let correlator = CrossCorrelator::new(haystack, max_digit_w, max_digit_h);
        let mut digit_scores: Vec<ColumnFeatureScore> = Vec::new();
        for digit in digits.iter() {
            digit_scores.push(correlator.cross_correlate_with(digit));
        }

        let digit_locations = locate_digits(&digit_scores, max_digit_w);

        if let Some(ref debug_scoring) = args.debug_scoring {
            debugdigit::debug_print_digits(
                haystack,
                &digits,
                max_digit_w,
                max_digit_h,
                &digit_scores,
                &digit_locations,
                &args.digit_images
            )
                .save(debug_scoring)
                .unwrap();
        }

        match extract_number(&digit_locations,
                             &args.digit_images,
                             args.emit_count) {
            Err(e) => {
                logger.log_error(captured.timestamp, e);
                if let Some(ref capture_dir) = args.failed_capture_dir {
                    let ts = captured.timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();
                    let filename = format!("{}/fail-{}.png",
                                           capture_dir,
                                           ts,
                    );
                    captured.image.save(filename).unwrap();
                }
                last_success=ExitCode::FAILURE;
            }
            Ok(meter_value) => {
                logger.log_value(captured.timestamp, meter_value);
                last_success = ExitCode::SUCCESS;
            }
        };

        match args.repeat_sec {
            Some(sec) => { std::thread::sleep(Duration::from_secs(sec)); }
            None => { break; }
        };
    }

    last_success
}
