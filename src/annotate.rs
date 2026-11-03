use std::path::Path;

use crate::analyze::DocumentItem;
use latex_rust::{MathFont, PngOptions, latex_to_png};
use pdfium_render::prelude::*;

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for line in text.split('\n') {
        let mut current_line = String::new();
        for word in line.split_whitespace() {
            if current_line.len() + word.len() + 1 > max_chars {
                lines.push(current_line);
                current_line = word.to_string();
            } else {
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(word);
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}

pub fn annotate_pdf_document(
    mut document: PdfDocument<'_>,
    output: impl AsRef<Path>,
    items: &[DocumentItem],
) -> Result<(), Box<dyn std::error::Error>> {
    let font = document.fonts_mut().helvetica();

    let math_font =
        MathFont::stix_two_math().map_err(|e| format!("Failed to load math font: {:?}", e))?;

    let page_count = document.pages().len();

    for page_idx in 0..page_count {
        let mut page_items: Vec<&DocumentItem> = items
            .iter()
            .filter(|item| item.page as i32 == page_idx)
            .collect();

        if page_items.is_empty() {
            continue;
        }

        page_items.sort_by(|a, b| a.bbox_2d[1].cmp(&b.bbox_2d[1]));

        let mut page = document.pages_mut().get(page_idx)?;
        let page_width = page.width().value;
        let page_height = page.height().value;

        let margin_x = page_width - 170.0;
        let max_margin_width = 155.0;

        for item in page_items {
            let xmin = (item.bbox_2d[0] as f32 / 1000.0) * page_width;
            let ymin_top_left = (item.bbox_2d[1] as f32 / 1000.0) * page_height;
            let xmax = (item.bbox_2d[2] as f32 / 1000.0) * page_width;
            let ymax_top_left = (item.bbox_2d[3] as f32 / 1000.0) * page_height;

            let ymin = page_height - ymax_top_left;
            let ymax = page_height - ymin_top_left;

            if let Some(ref err_msg) = item.error {
                let rect_path = PdfPagePathObject::new_rect(
                    &document,
                    PdfRect::new(
                        PdfPoints::new(xmin - 3.0),
                        PdfPoints::new(ymin - 3.0),
                        PdfPoints::new(xmax + 3.0),
                        PdfPoints::new(ymax + 3.0),
                    ),
                    Some(PdfColor::RED),
                    Some(PdfPoints::new(1.2)),
                    None,
                )?;
                page.objects_mut().add_path_object(rect_path)?;

                // 2. Dynamically anchor margin callout vertically aligned with the top of the problem box
                let mut current_y = ymax.min(page_height - 30.0);

                // 3. Draw a clean leader line connecting the problem box to the margin note
                let box_mid_y = (ymin + ymax) / 2.0;
                let leader_path = PdfPagePathObject::new_line(
                    &document,
                    PdfPoints::new(xmax + 3.0),
                    PdfPoints::new(box_mid_y),
                    PdfPoints::new(margin_x - 5.0),
                    PdfPoints::new(current_y - 4.0),
                    PdfColor::RED,
                    PdfPoints::new(0.8),
                )?;
                page.objects_mut().add_path_object(leader_path)?;

                let wrapped_err = wrap_text(err_msg, 24);
                let font_size = 8.0;
                let line_height = 10.0;

                for err_line in &wrapped_err {
                    let mut err_text_obj = page.objects_mut().create_text_object(
                        PdfPoints::new(margin_x),
                        PdfPoints::new(current_y),
                        err_line,
                        font,
                        PdfPoints::new(font_size),
                    )?;
                    err_text_obj.set_fill_color(PdfColor::RED)?;
                    current_y -= line_height;
                }

                current_y -= 4.0;

                if let Some(ref corr) = item.correction_latex
                    && let Ok(png_bytes) = latex_to_png(corr, &math_font, &PngOptions::new())
                    && let Ok(img) = image::load_from_memory(&png_bytes)
                {
                    let orig_w = img.width() as f32;
                    let orig_h = img.height() as f32;

                    let scale_w = max_margin_width / orig_w;
                    let scale_h = 22.0 / orig_h;
                    let scale = scale_w.min(scale_h).min(1.0);

                    let img_w = orig_w * scale;
                    let img_h = orig_h * scale;

                    current_y -= img_h;

                    page.objects_mut().create_image_object(
                        PdfPoints::new(margin_x),
                        PdfPoints::new(current_y),
                        &img,
                        Some(PdfPoints::new(img_w)),
                        Some(PdfPoints::new(img_h)),
                    )?;
                }
            } else {
                // Draw a subtle, clean green bounding box for fully correct student tasks
                let green_rect = PdfPagePathObject::new_rect(
                    &document,
                    PdfRect::new(
                        PdfPoints::new(xmin - 2.0),
                        PdfPoints::new(ymin - 2.0),
                        PdfPoints::new(xmax + 2.0),
                        PdfPoints::new(ymax + 2.0),
                    ),
                    Some(PdfColor::GREEN),
                    Some(PdfPoints::new(0.75)),
                    None,
                )?;
                page.objects_mut().add_path_object(green_rect)?;
            }
        }
    }

    document.save_to_file(&output)?;
    Ok(())
}
