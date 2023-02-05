# Todo: rewrite in Rust.

name=$1
post=$2
input="${name}.mp4"
output_h265="docs/blog/media/${post}/${name}-h265.mp4"
output_vp9="docs/blog/media/${post}/${name}-vp9.webm"
loops=2
fps=30

time ffmpeg \
    -y -hide_banner \
    -i $input \
    -c:v libx265 \
    -crf 21 \
    -g 240 \
    -pix_fmt yuv420p \
    -filter:v fps=$fps \
    -preset slow \
    -tag:v hvc1 \
    -movflags faststart \
    -an \
    $output_h265
du -h $output_h265

# # https://developers.google.com/media/vp9/the-basics
time ffmpeg \
    -y -hide_banner \
    -i $input \
    -c:v libvpx-vp9 \
    -crf 33 \
    -g 240 \
    -pix_fmt yuv420p \
    -filter:v fps=$fps \
    -row-mt 1 \
    -quality good \
    -speed 0 \
    -cpu-used 0 \
    -an \
    $output_vp9
du -h $output_vp9

echo "
<video width=\"800\" height=\"450\" autoplay loop muted playsinline>
    <source src=\"media/${post}/${output_h265}\" type=\"video/mp4\" />
    <source src=\"media/${post}/${output_vp9}\" type=\"video/webm\" />
</video>
"
