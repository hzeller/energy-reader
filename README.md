# Read utility meters such as gas, water, or electricity.

Mostly I need that at home to integrate the analog reader output into my
home automation and monitoring.

Rather than doing a generic OCR, this is matching digits using normalized
cross correlation (this requires to extract sample digit images first).

This is meant to read the typical mechanical counters found in utility meters.

## Build

```
cargo build --release
```

## Run

```
Usage: utility-reader [OPTIONS] [DIGIT_IMAGES]...

Arguments:
  [DIGIT_IMAGES]...  Digit template images to match; the first digit found in the filename is the matched digit. Allows to have multiple templates for the same digit if needed (e.g. d1-0.png, d1-1.png)

Options:
      --webcam                          Capture counter image from webcam
      --filename <png-file>             Read counter image from file
      --op <op>                         Image operation to apply after image is acquired. One of ["rotate90", "rotate180", "flip-x", "flip-y", "crop:<x>:<y>:<w>:<h>"]. Multiple --op are applied in sequence provided on command line
      --sobel                           Process input images through sobel edge-detect. Can improve accuracy with very clean and non-distorted images
      --emit-count <#>                  Number of digits to OCR verify and emit. Good to limit if the last digit is finicky due to roll-over [default: 7]
      --max-plausible-rate <count/sec>  Maximum plausible value change per second to avoid logging bogus values [default: 0.1]
      --repeat-sec <seconds>            Repeat every these number of seconds (useful with --webcam)
      --debug-capture <file-or-dir>     Output the image captured. If existing directory, writes snap-<timestemp>.png images, otherwise intepreted as filename
      --debug-post-ops <file-or-dir>    Output the image after the process ops have been applied. If existing directory, writes processed-<timestemp>.png images, otherwise intepreted as filename
      --failed-capture <file-or-dir>    Output image that could not detect all digits. If existing directory, writes fail-<timestemp>.png images, otherwise intepreted as filename
      --debug-scoring <img-file>        Generate a debug image that illustrates the detection details
  -h, --help                            Print help
  -V, --version                         Print version
```

The digit-images need to be extracted from images of counters before, i.e. single digits the same size as they appear in the counter, e.g. looking like:

![](img/digit-3.png)

The first digit that is found in the filename is considered the digit it
represents. You can have multiple templates for the same digit in case a
single template is not enough; below in the debugging section you see examples
with multiple templates (in particular the `1` matched two different shapes,
but is interpreted as the same digit).

Then running the program with `--filename` on the image:

![](img/example-counter.png)

will output the sequence of digits observed including timestamp here `17300734`, so it can be used directly in scripts for further processing.

If there is a plausibility check failing (uneven physical distance of digits
or not exepected number of digits), then there is an error message on stderr and
exit code is non-zero (while stdout still outputs whatever digits it could
read). Number of digits that is to be checked and emitted can be controlled with
the `--emit-count`.

If instead of `--filename`, the `--webcam` option is used, the image is fetched
from the webcam.

Since the image from the webcam probably needs some massaging to just extract
the area with the counter, there are image operations that can be applied
before sent to the digit detection.
For instance the following flags `--op rotate180 --op crop:10:30:1270:200`
will first rotate the image from the webcam by 180 degrees, then crop from
(x,y) = (10, 30) and the given width and height of 1280, 200.

Use `--debug-post-ops` to determine if the resulting image is as expected
(Since you're on a shell, you probaly want to use [timg](https://timg.sh) as
image viewer).

The `--repeat-sec` option will keep the program running and re-capturing new
images, the typical application when monitoring with a webcam.

## Debugging

There are a few debugging options which help while setting up the reader the
first time

   * `--debug-capture` option allows to output the captured image to a file,
   * `--debug-post-ops` emits the image _after_ the image process operations.
   * `--debug-scoring` emits an image with detailed detection information (see below)
   * `--failed-capture-dir` collects all images that could not be properly OCR'ed.

### Debug Scoring

With the `--debug-scoring` option, an image file is generated to illustrate how
well each digit scores on each column of the meter image.
It shows the edge-preprocessed original image, a spark-line of 'matching score'
for each digit and as final row with the assembled images of the match digits.

![](img/example-output.png)

It also outputs a list (to stderr) with one line per matching digit.
The columns contain the digit, their positions on the x-axis and a score as
well as the digit filename that matched.

```
digits/d1-1.png    69 0.960
digits/d7-2.png   230 0.975
digits/d5-1.png   385 0.941
digits/d7-2.png   536 0.944
digits/d3-0.png   688 0.975
digits/d2-0.png   838 0.970
digits/d1-0.png  1003 0.978
digits/d3-0.png  1164 0.877
```

## Postprocessing

When running with `--repeat-sec`, the energy reader will regularly read the
values from the counter and write to stdout; timestamp and value.

You can use the awk-script [`plot.awk`](./plot.awk) to postprocess that data
to adapt the decimal point and calculate some derivation to calculate the
currently used Kilowatt, and then use the gnuplot
script [`plot.gp`](./plot.gp) to generate a graph.

```
./plot.awk < reader.log > /tmp/data.log
./plot.gp
```

The gnuplot script will directly draw the graph on the terminal (should be
sufficiently modern terminal, such as `konsole`, but most can do graphics these
days); alternatively you can modify the script to output to a PNG.

The following example also shows that it is good to have some light-source for
the camera to see at night :)

![](img/sample-graph.png)
