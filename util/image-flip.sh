set -x

blog_path="docs/blog"
blog_post_media="${blog_path}/media/new-skylight-model"

image_annotate() {
    local font="${blog_path}/fonts/SourceSansPro-SemiBold.ttf"
    local pointsize=48
    local fill="white"
    local stroke="black"
    local strokewidth=2
    local gravity="south-west"
    local text_pos="20,10"
    local input_text=$1
    local input=$2
    local output=$3
    magick convert $input \
        -font $font \
        -pointsize $pointsize \
        -fill $fill \
        -stroke $stroke \
        -strokewidth $strokewidth \
        -gravity $gravity \
        -draw "text ${text_pos} '${input_text}'" \
        $output
}

image_flip() {
    local input_text_0=$1
    local input_text_1=$2
    local input_0=$3
    local input_1=$4
    local frame_0="frame-0.png"
    local frame_1="frame-1.png"
    local output="${blog_post_media}/$5"

    image_annotate $input_text_0 $input_0 $frame_0
    image_annotate $input_text_1 $input_1 $frame_1

    magick convert \
        -delay 100 \
        -loop 0 \
        $frame_0 $frame_1 \
        $output
    rm $frame_0
    rm $frame_1

    echo Wrote to $output
}

image_exposure() {

    image_annotate "Exposure: 1/1" "image-1.png" "frame-1-1.png"
    image_annotate "Exposure: 1/2" "image-2.png" "frame-1-2.png"
    image_annotate "Exposure: 1/4" "image-4.png" "frame-1-4.png"
    image_annotate "Exposure: 1/8" "image-8.png" "frame-1-8.png"
    image_annotate "Exposure: 1/16" "image-16.png" "frame-1-16.png"
    image_annotate "Exposure: 1/32" "image-32.png" "frame-1-32.png"
    image_annotate "Exposure: 1/64" "image-64.png" "frame-1-64.png"
    image_annotate "Exposure: 1/128" "image-128.png" "frame-1-128.png"
    image_annotate "Exposure: 1/256" "image-256.png" "frame-1-256.png"
    image_annotate "Exposure: 1/512" "image-512.png" "frame-1-512.png"
    image_annotate "Exposure: 1/1024" "image-1024.png" "frame-1-1024.png"
    local output="${blog_post_media}/$1"
    magick convert \
        -delay 75 \
        -loop 0 \
        "frame-1-1.png" \
        "frame-1-2.png" \
        "frame-1-4.png" \
        "frame-1-8.png" \
        "frame-1-16.png" \
        "frame-1-32.png" \
        "frame-1-64.png" \
        "frame-1-128.png" \
        "frame-1-256.png" \
        "frame-1-512.png" \
        "frame-1-1024.png" \
        $output
    rm "frame-1-1.png"
    rm "frame-1-2.png"
    rm "frame-1-4.png"
    rm "frame-1-8.png"
    rm "frame-1-16.png"
    rm "frame-1-32.png"
    rm "frame-1-64.png"
    rm "frame-1-128.png"
    rm "frame-1-256.png"
    rm "frame-1-512.png"
    rm "frame-1-1024.png"

    echo Wrote to $output
}

# image_flip "Linear" "Tonemapped" 20230207-214316.png 20230207-214324-tone.png tonemap-compare-dim.apng
# image_flip "Linear" "Tonemapped" 20230207-214412.png 20230207-214419-tone.png tonemap-compare-bright.apng
image_exposure "exposure.apng"