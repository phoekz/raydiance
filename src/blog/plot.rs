use super::*;

pub fn gen() -> Result<()> {
    use plotters::prelude::*;
    use std::io::Write;

    // Create plots
    let samples = 5000;
    let aces_plot = {
        let mut points = vec![];
        for x in 0..=samples {
            let x = 10.0 * (x as f32) / (samples as f32);
            let y = ColorRgb::new(x, x, x).tonemap().red();
            points.push((x, y));
        }
        points
    };

    let linear_plot = {
        let mut points = vec![];
        for x in 0..=samples {
            let x = 10.0 * (x as f32) / (samples as f32);
            let y = x.min(1.0);
            points.push((x, y));
        }
        points
    };

    // Create svg file.
    let mut output_svg = String::new();
    {
        let font = "Source Sans Pro - Regular";
        let root = SVGBackend::with_string(&mut output_svg, (600, 300)).into_drawing_area();
        let root = root.margin(10, 10, 10, 10);
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(50)
            .y_label_area_size(60)
            .build_cartesian_2d((0.0_f32..10.0_f32).log_scale(), 0.0_f32..1.0_f32)?;

        chart
            .configure_mesh()
            .x_desc("Input color")
            .y_desc("Output color")
            .x_labels(5)
            .y_labels(5)
            .y_label_formatter(&|x| format!("{x:.2}"))
            .label_style((font, 18))
            .draw()?;

        let linear_color = RGBColor(0, 0, 0);
        let aces_color = RGBColor(255, 140, 0);

        chart
            .draw_series(LineSeries::new(linear_plot, linear_color))?
            .label("Linear")
            .legend(move |(x, y)| PathElement::new([(x, y), (x + 20, y)], linear_color));

        chart
            .draw_series(LineSeries::new(aces_plot, aces_color.stroke_width(2)))?
            .label("ACES")
            .legend(move |(x, y)| {
                PathElement::new([(x, y), (x + 20, y)], aces_color.stroke_width(2))
            });

        chart
            .configure_series_labels()
            .border_style(BLACK)
            .background_style(WHITE.mix(0.8))
            .label_font((font, 18))
            .draw()?;
    }

    // Hack: we have to override the svg size in order to center it in our blog.
    let output_svg = output_svg.replace(
        "width=\"600\" height=\"300\"",
        "width=\"800\" height=\"300\"",
    );

    let path = posts_dir().join("20230209-121100-new-skylight-model/plot.svg");
    File::create(path)?.write_all(output_svg.as_bytes())?;

    Ok(())
}
