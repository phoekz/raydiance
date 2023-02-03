name=$1
post=$2
input="${name}.mp4"
output_h265="${name}-h265.mp4"
output_vp9="${name}-vp9.webm"
crf_h265=23
crf_vp9=$(($crf_h265 * 63 / 51))

ffmpeg \
    -i $input \
    -c:v libx265 \
    -crf $crf_h265 \
    -preset slow \
    -tag:v hvc1 \
    -movflags faststart \
    -an \
    $output_h265

ffmpeg \
    -i $input \
    -c:v libvpx-vp9 \
    -crf $crf_vp9 \
    -row-mt 1 \
    -deadline good \
    -cpu-used 0 \
    -an \
    $output_vp9

echo "
<video width=\"800\" height=\"450\" autoplay loop muted playsinline>
    <source src=\"media/${post}/${output_h265}\" type=\"video/mp4\" />
    <source src=\"media/${post}/${output_vp9}\" type=\"video/webm\" />
</video>
"
