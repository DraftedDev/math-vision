use oar_ocr::prelude::*;
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use whatlang::{Lang, detect};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExtractedItem {
    pub page: usize,
    pub bbox_2d: [u32; 4],
    pub language: String,
    pub task: String,
}

pub fn extract_document_items(
    document: &PdfDocument<'_>,
) -> Result<Vec<ExtractedItem>, Box<dyn std::error::Error>> {
    let structure = OARStructureBuilder::pp_structurev3().build()?;

    let mut items = Vec::new();
    let page_count = document.pages().len();

    let render_cfg = PdfRenderConfig::default()
        .use_lcd_text_rendering(true)
        .disable_native_text_rendering(true);

    for page_idx in 0..page_count {
        let page = document.pages().get(page_idx)?;
        let page_width = page.width().value;
        let page_height = page.height().value;

        let img = page.render_with_config(&render_cfg)?.as_image()?;

        let temp_filename = format!("temp_page_{}.jpg", page_idx);
        img.save(&temp_filename)?;

        let structure_result = structure.predict(&temp_filename)?;
        let _ = std::fs::remove_file(&temp_filename);

        for element in &structure_result.layout_elements {
            let bbox = &element.bbox;

            let xmin = bbox.x_min();
            let ymin = bbox.y_min();
            let xmax = bbox.x_max();
            let ymax = bbox.y_max();

            let xmin_norm = ((xmin / page_width) * 1000.0).clamp(0.0, 1000.0) as u32;
            let ymin_norm = ((ymin / page_height) * 1000.0).clamp(0.0, 1000.0) as u32;
            let xmax_norm = ((xmax / page_width) * 1000.0).clamp(0.0, 1000.0) as u32;
            let ymax_norm = ((ymax / page_height) * 1000.0).clamp(0.0, 1000.0) as u32;

            let task_text = element.text.clone().unwrap_or_else(|| "".to_string());

            // TODO: Other languages.
            let language = if !task_text.is_empty() {
                match detect(&task_text) {
                    Some(info) if info.lang() == Lang::Deu => "de".to_string(),
                    _ => "en".to_string(),
                }
            } else {
                "en".to_string()
            };

            items.push(ExtractedItem {
                page: page_idx as usize,
                bbox_2d: [xmin_norm, ymin_norm, xmax_norm, ymax_norm],
                language,
                task: task_text,
            });
        }
    }

    Ok(items)
}
