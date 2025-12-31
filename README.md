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

will output a list with one line per matching digit. The columns contain the digit,
their positions on the x-axis and a score, which will allow post-processing to determine
if the data looks plausible:

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

If compiled with `--features debug_img`, a `debug-output.png` image is created
to illustrate how well each digit scores on each column of the meter image.
It shows the edge-preprocessed original image, a sparkline of 'matching score'
for each digit and as final row with the assembled images of the match digits.


![](img/example-output.png)
