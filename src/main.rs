use anyhow::Result;
use clap::Parser;
use image::GrayImage;
use std::path::Path;
use std::cmp;
use std::process::ExitCode;
use std::time::{SystemTime,UNIX_EPOCH,Duration};

mod cross_correlator;
use cross_correlator::{CrossCorrelator,ColumnFeatureScore};

mod image_util;
use image_util::{sobel,load_image_as_grayscale, apply_ops};

mod debugdigit;

mod sources;
use sources::{FilenameSource,WebCamSource};

pub const THRESHOLD: f32 = 0.6;

// Plausibility checks
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

    /// toggle if the input images should go through edge detection (sobel
    /// filter).
    #[arg(long = "sobel", default_value="false")]
    edge_process: bool,

    /// Minimum number of expected digits in OCR (note that sometimes the last
    /// digit can not be detected as it scrolls through)
    #[arg(long, value_name="#", default_value="8")]
    expect_count: u32,

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

pub struct TimestampedImage {
    timestamp: SystemTime,
    image: GrayImage,
}
pub trait ImageSource {
    fn read_image(&self) -> Result<TimestampedImage>;
}

#[derive(Clone)]
pub struct DigitPos {
    digit_pattern: u32,
    score: f32,
    pos: u32,
}

// Find the hightest score digits and emit their positions.
fn locate_digits(scores: &[ColumnFeatureScore], digit_width: u32)
                 -> Vec<DigitPos> {
    // The shortest score vector is the max x-position we check out
    let x_range = scores.iter().map(|v| v.len()).min().unwrap_or(0) as u32;
    let mut result = Vec::new();
    let fresh_digit = DigitPos { digit_pattern: u32::MAX, score: 0.0, pos: x_range };
    let mut current = fresh_digit.clone();
    // Find highest score that does not change for the width of a digit.
    for x in 0..x_range {
        for (i, feature_score) in scores.iter().enumerate() {
            let digit_score = feature_score[x as usize];
            if digit_score < THRESHOLD {
                continue;
            }
            if digit_score > current.score {
                current.digit_pattern = i as u32;
                current.score = digit_score;
                current.pos = x;
            }
        }

        if x >= current.pos + digit_width { // best seen for digit-width
            result.push(current);
            current = fresh_digit.clone();
        }
    }
    if current.digit_pattern != u32::MAX {
        result.push(current);
    }
    result
}

fn looks_plausible(locations: &[DigitPos],
                   expect_count: u32) -> Result<(), String> {
    if locations.len() < expect_count as usize {
        return Err(format!("Got {} digits, but expected {}",
                           locations.len(), expect_count));
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
    Ok(())
}

fn log_result(out: &mut dyn std::io::Write, ts: &SystemTime,
              locations: &[DigitPos], digit_filenames: &[String]) {
    write!(out, "{}\t",
           ts.duration_since(UNIX_EPOCH).unwrap().as_secs()).unwrap();
    for loc in locations {
        let filename = &digit_filenames[loc.digit_pattern as usize];
        let first_digit = filename.chars().find(|c| c.is_numeric());
        match first_digit {
            Some(digit) => write!(out, "{}", digit).unwrap(),
            None => {},
        }
    }
    writeln!(out).unwrap();
}

// Params: energy-reader <counter-image> <digit0> <digit1>...
fn main() -> ExitCode {
    let args = CliArgs::parse();

    if args.failed_capture_dir.is_some()
        && !Path::new(args.failed_capture_dir.as_ref().unwrap()).is_dir() {
        eprintln!("'{}' needs to be an existing dir for --failed_capture_dir",
                  args.failed_capture_dir.as_ref().unwrap());
        return ExitCode::FAILURE;
    }

    let source: Box<dyn ImageSource >= if args.filename.is_some() {
        Box::new(FilenameSource::new(args.filename.unwrap()))
    } else if args.webcam {
        Box::new(WebCamSource{})
    } else {
        eprintln!("Need one of --filename or --webcam");
        return ExitCode::FAILURE;
    };

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
        if args.debug_capture.is_some() {
            captured.image.save(args.debug_capture.as_ref().unwrap()).unwrap();
        }
        if let Err(e) = apply_ops(&mut captured.image, &args.process_ops) {
            eprintln!("Check your image ops: {e:#}");
            return ExitCode::FAILURE;
        }
        if args.debug_post_ops.is_some() {
            captured.image.save(args.debug_post_ops.as_ref().unwrap()).unwrap();
        }

        let haystack = &captured.image;
        let haystack = if args.edge_process {
            &sobel(haystack)
        } else {
            haystack
        };

        let correlator = CrossCorrelator::new(&haystack, max_digit_w, max_digit_h);
        let mut digit_scores: Vec<ColumnFeatureScore> = Vec::new();
        for digit in digits.iter() {
            digit_scores.push(correlator.cross_correlate_with(digit));
        }

        let digit_locations = locate_digits(&digit_scores, max_digit_w);

        if args.debug_scoring.is_some() {
            let debug_filename = args.debug_scoring.as_ref().unwrap();
            debugdigit::debug_print_digits(
                &haystack,
                &digits,
                max_digit_w,
                max_digit_h,
                &digit_scores,
                &digit_locations,
                &args.digit_images
            )
                .save(debug_filename)
                .unwrap();
        }

        match looks_plausible(&digit_locations, args.expect_count) {
            Err(e) => {
                let ts = captured.timestamp.duration_since(UNIX_EPOCH).unwrap().as_secs();
                eprintln!("{} (at {})", e, ts);
                if args.failed_capture_dir.is_some() {
                    let filename = format!("{}/fail-{}.png",
                                           args.failed_capture_dir.as_ref().unwrap(),
                                           ts
                                           );
                    captured.image.save(filename).unwrap();
                }
                last_success=ExitCode::FAILURE;
            }
            Ok(_) => {
                log_result(&mut std::io::stdout(), &captured.timestamp, &digit_locations, &args.digit_images);
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
