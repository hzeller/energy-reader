# Read energy counter such as gas or electricity.

Mostly I need that at home to integrate the analog reader output into my
home automation and monitoring.

Rather than doing a generic OCR, this is just matching digits using normalized
cross correlation.

## Build

```
cargo build --release
```

## Run

```
energy-reader image-of-counter.png digit-0.png digit-1.png ...
```

The digit-images need to be extracted from images of counters before, i.e. single digits the same
size as they appear in the counter, such as:

![](img/digit-3.png)

Then running the program on

![](img/example-counter.png)

will output the sequence of digits observed, here `17300734`, so it can be used directly
in scripts for further processing.

If there is a plausibility check failing (uneven physical distance of digits
or not exepected number of digits), then there is an error message on stderr and
exit code is non-zero (while stdout still outputs whatever digits it could
read).

## Debugging
If compiled with `--features debug_img`, a `./debug-output.png` image is created
to illustrate how well each digit scores on each column of the meter image.
It shows the edge-preprocessed original image, a spark-line of 'matching score'
for each digit and as final row with the assembled images of the match digits.

![](img/example-output.png)

It also outputs a list (to stderr) with one line per matching digit.
The columns contain the digit, their positions on the x-axis and a score.

```
1   37 0.839
7  132 0.791
3  221 0.802
0  309 0.797
0  399 0.927
7  492 0.909
3  580 0.957
4  676 0.921
```
