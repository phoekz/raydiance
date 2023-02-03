name="20230202-235500"
input="${name}.mp4"
output_h265="${name}-h265.mp4"
output_vp9="${name}-vp9.webm"
crf_h265=20
crf_vp9=$(($crf_h265 * 63 / 51))

ffmpeg \
    -i $input \
    -c:v libx265 \
    -crf $crf_h265 \
    -preset veryslow \
    -tag:v hvc1 \
    -movflags faststart \
    -an \
    $output_h265

ffmpeg \
    -i $input \
    -c:v libvpx-vp9 \
    -crf $crf_vp9 \
    -deadline best \
    -an \
    $output_vp9

echo "
<video width=\"800\" height=\"450\" autoplay loop muted playsinline>
    <source src=\"images/${output_h265}\" type=\"video/mp4\" />
    <source src=\"images/${output_vp9}\" type=\"video/webm\" />
</video>
"
