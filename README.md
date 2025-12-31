# Read energy counter such as gas or electricity.

Mostly I need that at home to integrate the analog reader output into my
home automation and monitoring.

Rather than doing a generic OCR, this is just matching digits using normalized
cross correlation.

## Build

```
cargo build
```

## Run

```
energy-reader image-of-counter.png digit-0.png digit-1.png ...
```

The digits need to be pre-processed images of digits extracted from images of
counters before (i.e. same size as they appear in the counter).

Then running the program on

![](img/example-counter.png)

will output a list of the matching digits. First column is the digit,
followed by the corresponding file-name, the x-position in the image and
score; easy to process with awk etc.

```
1 img-foo/digit-1.png   37 0.839
7 img-foo/digit-7.png  132 0.791
3 img-foo/digit-3.png  221 0.802
0 img-foo/digit-0.png  309 0.797
0 img-foo/digit-0.png  399 0.927
7 img-foo/digit-7.png  492 0.909
3 img-foo/digit-3.png  580 0.957
4 img-foo/digit-4.png  676 0.921
```

If compiled with `--features debug_img`, a debug image is created to figure
out how well things score; it shows the edge-preprocessed original image,
a sparkkline of 'matching score' for each digit and as final row thw chosen
digits.

![](img/example-output.png)
