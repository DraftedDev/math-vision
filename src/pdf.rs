use std::path::Path;

use image::{DynamicImage, GenericImageView, GrayImage, imageops::FilterType};
use imageproc::{contrast::stretch_contrast, filter::sharpen_gaussian};
use latex_rust::{MathFont, PngOptions, latex_to_png};
use pdfium_render::prelude::{
    PdfColor, PdfDocument, PdfPageObjectCommon, PdfPageObjectsCommon, PdfPagePathObject, PdfPoints,
    PdfRect, PdfRenderConfig,
};

use crate::{analyzer::DocumentItem, utils::wrap_text};

pub fn annotate_pdf(
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

                let mut current_y = ymax.min(page_height - 30.0);

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

pub fn render_pdf(
    document: &PdfDocument,
    max_width: u32,
    max_height: u32,
) -> anyhow::Result<Vec<DynamicImage>> {
    let render_cfg = PdfRenderConfig::default()
        .use_lcd_text_rendering(true)
        .disable_native_text_rendering(true)
        .scale_page_by_factor(2.0);

    document
        .pages()
        .iter()
        .map(|page| {
            let img = page.render_with_config(&render_cfg)?.as_image()?;
            let processed = process_img(img, max_width, max_height)?;
            let optimized = optimize_img(processed);

            Ok(optimize_img(optimized))
        })
        .collect::<anyhow::Result<Vec<_>>>()
}

fn optimize_img(img: DynamicImage) -> DynamicImage {
    let gray_img: GrayImage = img.to_luma8();
    let stretched = stretch_contrast(&gray_img, 30, 220, 0, 255);
    let sharpened: GrayImage = sharpen_gaussian(&stretched, 1.0, 1.5);

    DynamicImage::ImageLuma8(sharpened)
}

fn process_img(img: DynamicImage, max_width: u32, max_height: u32) -> anyhow::Result<DynamicImage> {
    let (width, height) = img.dimensions();
    let width_f = width as f32;
    let height_f = height as f32;
    let max_w_f = max_width as f32;
    let max_h_f = max_height as f32;

    let processed_img = if width_f > max_w_f || height_f > max_h_f {
        let scale_x = max_w_f / width_f;
        let scale_y = max_h_f / height_f;
        let scale = scale_x.min(scale_y);

        let new_width = (width_f * scale) as u32;
        let new_height = (height_f * scale) as u32;

        img.resize_exact(new_width, new_height, FilterType::Lanczos3)
    } else {
        img
    };

    Ok(processed_img)
}
