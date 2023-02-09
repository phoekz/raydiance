input_0="work/render-elevation.apng"
input_1="work/render-azimuth.apng"
input_2="work/render-turbidity.apng"
input_3="work/render-albedo.apng"
output="new-offline-render"

ffmpeg \
    -y \
    -stream_loop 2 -i $input_0 \
    -stream_loop 2 -i $input_1 \
    -stream_loop 2 -i $input_2 \
    -stream_loop 2 -i $input_3 \
    -filter_complex "[0:v][1:v]hstack[a],[2:v][3:v]hstack[b],[a][b]vstack" \
    -c:v libx265 \
    -crf 18 \
    -g 240 \
    -pix_fmt yuv420p \
    -preset slow \
    -tag:v hvc1 \
    -movflags faststart \
    -an \
    $output-h265.mp4

ffmpeg \
    -y \
    -stream_loop 2 -i $input_0 \
    -stream_loop 2 -i $input_1 \
    -stream_loop 2 -i $input_2 \
    -stream_loop 2 -i $input_3 \
    -filter_complex "[0:v][1:v]hstack[a],[2:v][3:v]hstack[b],[a][b]vstack" \
    -c:v libvpx-vp9 \
    -crf 20 \
    -g 240 \
    -pix_fmt yuv420p \
    -row-mt 1 \
    -quality good \
    -speed 0 \
    -cpu-used 0 \
    -an \
    $output-vp9.webm