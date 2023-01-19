font="./docs/blog/fonts/SourceCodePro-Regular.ttf"
pointsize=16

magick montage \
    -font $font -pointsize $pointsize -label '%t' \
    -border 1 -bordercolor "#000000" \
    -geometry 380x+4+4 -tile 2x3 \
    work/side-cosine-sobol-256.png work/side-uniform-sobol-256.png \
    work/side-cosine-random-256.png work/side-uniform-random-256.png \
    work/side-cosine-grid-256.png work/side-uniform-grid-256.png \
    docs/blog/images/20230120-011300-side.png

magick montage \
    -font $font -pointsize $pointsize -label '%t' \
    -border 1 -bordercolor "#000000" \
    -geometry 256x+4+4 -tile 3x2 \
    work/top-cosine-sobol-256.png work/top-cosine-random-256.png work/top-cosine-grid-256.png \
    work/top-uniform-sobol-256.png work/top-uniform-random-256.png work/top-uniform-grid-256.png \
    docs/blog/images/20230120-011300-top.png