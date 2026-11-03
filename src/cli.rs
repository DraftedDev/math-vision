use std::{io::Cursor, path::Path};

use base64::{Engine, engine::general_purpose::STANDARD};
use clap::Parser;
use image::{DynamicImage, GenericImageView, ImageFormat, imageops::FilterType};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium};
use tokio::fs;

use crate::analyze::{Analyzer, AnalyzerConfig};

/// Interface to Math Vision Pipeline.
#[derive(Debug, Clone, Parser)]
pub struct Cli {
    /// The path to the pdfium library.
    #[arg(long = "pdfium", alias = "pdf", default_value = "./libs/libpdfium.so")]
    pub pdfium: String,
    /// The Ollama Service URL.
    #[arg(long = "url", short = 'u', default_value = "http://localhost:11434")]
    pub url: String,
    /// The Ollama model to use.
    #[arg(long = "model", short = 'm', default_value = "gemma4:e4b")]
    pub model: String,
    /// The file to read the model prompt from.
    #[arg(long = "prompt", short = 'p', default_value = "prompt.txt")]
    pub prompt: String,
    /// The model temperature.
    #[arg(long = "temp", short = 't', default_value = "0.1")]
    pub temp: f32,
    /// The maximum context tokens of the model.
    #[arg(long = "ctx", alias = "nc", default_value = "8192")]
    pub ctx: u64,
    /// The maximum prediction tokens of the model.
    #[arg(long = "predict", alias = "np", default_value = "8192")]
    pub predict: i32,
    /// The top-p model parameter.
    #[arg(long = "top_p", alias = "tp", default_value = "1.0")]
    pub top_p: f32,
    /// The thinking mode for the model. Can be: 'true', 'false', 'low', 'medium', 'high'.
    #[arg(long = "thinking", alias = "think", default_value = "false")]
    pub thinking: String,
    /// The maximum width of a rendered PDF page. Rendered pages larger than this, will be resized.
    #[arg(long = "max-width", alias = "mw", default_value = "4000")]
    pub max_width: u32,
    /// The maximum height of a rendered PDF page. Rendered pages larger than this, will be resized.
    #[arg(long = "max-height", alias = "mh", default_value = "4000")]
    pub max_height: u32,
    /// If specified, save computed document items. Good for debugging.
    #[arg(long = "save-items", alias = "si")]
    pub save_items: Option<String>,
    /// If specified, load the document items and skip the processing steps. Good for debugging.
    #[arg(long = "load-items", alias = "li")]
    pub load_items: Option<String>,
    /// Run the application in debug mode.
    #[arg(long = "debug", short = 'd', default_value = "false")]
    pub debug: bool,
    /// The target PDF file to analyze.
    pub file: String,
    /// The path where the annotated PDF should be saved to.
    pub output: String,
}

impl Cli {
    pub async fn run(self) {
        let path = Path::new(&self.file);
        let bytes = tokio::fs::read(path)
            .await
            .expect("Failed to read document file");

        log::info!("Loading pdfium library...");

        let pdfium_path = fs::canonicalize(self.pdfium)
            .await
            .expect("Failed to canonicalize pdfium library path");

        if !fs::try_exists(&pdfium_path)
            .await
            .expect("Failed to check for lib pdfium path existence")
        {
            panic!(
                "Pdfium library not found at '{}'! Please install a valid pdfium library.",
                pdfium_path.display()
            );
        }

        let bindings =
            Pdfium::bind_to_library(pdfium_path).expect("Failed to bind to pdfium library");
        let pdfium = Pdfium::new(bindings);

        log::info!("Reading PDF document...");
        let doc = pdfium
            .load_pdf_from_byte_slice(&bytes, None)
            .expect("Failed to read PDF document");

        let items = if let Some(items_path) = self.load_items {
            log::info!("Loading items from {items_path}...");
            let data = fs::read(&items_path)
                .await
                .expect("Failed to read document items");

            serde_json::from_slice(&data).expect("Failed to deserialize items")
        } else {
            let analyzer_config = AnalyzerConfig {
                url: self.url,
                model: self.model,
                prompt: self.prompt,
                temp: self.temp,
                ctx: self.ctx,
                predict: self.predict,
                top_p: self.top_p,
            };

            log::info!("Initializing analyzer with config: {analyzer_config:#?}");
            let analyzer = Analyzer::new(analyzer_config);

            log::info!("Running structural layout extraction...");
            let extracted_items = crate::extract::extract_document_items(&doc)
                .expect("Failed to extract document items");

            log::debug!("Extracted Items: {extracted_items:#?}");

            let render_cfg = PdfRenderConfig::default()
                .use_lcd_text_rendering(true)
                .disable_native_text_rendering(true);

            let mut images = Vec::with_capacity(doc.pages().len() as usize);

            log::info!("Processing document pages into images...");
            for (idx, page) in doc.pages().iter().enumerate() {
                let image = page
                    .render_with_config(&render_cfg)
                    .expect("Failed to render PDF")
                    .as_image()
                    .expect("Failed to convert bitmap to dynamic image");

                let processed = process_image(image, self.max_width, self.max_height)
                    .expect("Failed to process image");

                if self.debug {
                    if !fs::try_exists("debug")
                        .await
                        .expect("Failed to check for debug dir existence")
                    {
                        fs::create_dir("debug")
                            .await
                            .expect("Failed to create debug dir");
                    }

                    let img_path =
                        Path::new("debug").join(format!("{}_{}.png", path.display(), idx));
                    log::debug!("Storing processed image at: {}", img_path.display());

                    fs::write(img_path, &processed)
                        .await
                        .expect("Failed to write processed image");
                }

                let base64 = STANDARD.encode(processed);
                images.push(base64);
            }

            log::info!("Running semantic analysis on extracted items...");
            let items = analyzer.analyze(&extracted_items).await;

            if let Some(items_path) = self.load_items {
                let data = serde_json::to_vec(&items).expect("Failed to serialize items");
                fs::write(&items_path, data)
                    .await
                    .expect("Failed to write document items");
            }

            items
        };

        log::debug!("Evaluated Document Items: {items:#?}");

        log::info!("Annotating PDF document...");
        crate::annotate::annotate_pdf_document(doc, &self.output, &items)
            .expect("Failed to annotate PDF document");

        log::info!("Saved annotated PDF at '{}'.", self.output);
    }
}

fn process_image(
    img: DynamicImage,
    max_width: u32,
    max_height: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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

    let mut buffer = Cursor::new(Vec::new());
    processed_img.write_to(&mut buffer, ImageFormat::Png)?;

    Ok(buffer.into_inner())
}
