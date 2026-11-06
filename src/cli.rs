use std::path::Path;

use clap::Parser;
use pdfium_render::prelude::Pdfium;

use crate::{
    analyzer::{Analyzer, AnalyzerConfig},
    extractor::{Extractor, ExtractorConfig},
    utils::Config,
};

/// Interface to Math Vision Pipeline.
#[derive(Debug, Clone, Parser)]
pub struct Cli {
    /// The analyzer configuration file.
    #[arg(long = "analyzer", short = 'a', default_value = "./analyzer.json")]
    pub analyzer: String,
    /// The extractor configuration file.
    #[arg(long = "extractor", short = 'e', default_value = "./extractor.json")]
    pub extractor: String,
    /// The path to the pdfium library.
    #[arg(long = "pdfium", alias = "p", default_value = "./libs/libpdfium.so")]
    pub pdfium: String,
    /// The maximum width of a rendered PDF page. Rendered pages larger than this, will be resized.
    #[arg(long = "max-width", alias = "w", default_value = "4000")]
    pub max_width: u32,
    /// The maximum height of a rendered PDF page. Rendered pages larger than this, will be resized.
    #[arg(long = "max-height", alias = "h", default_value = "4000")]
    pub max_height: u32,
    /// If specified, save computed document items. Good for debugging.
    #[arg(long = "save-items", short = 's')]
    pub save_items: Option<String>,
    /// If specified, load the document items and skip the processing steps. Good for debugging.
    #[arg(long = "load-items", short = 'l')]
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
        let bytes = std::fs::read(path).expect("Failed to read document file");

        log::info!("Loading pdfium library...");

        let pdfium_path =
            std::fs::canonicalize(self.pdfium).expect("Failed to canonicalize pdfium library path");

        if !std::fs::exists(&pdfium_path).expect("Failed to check for lib pdfium path existence") {
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
            let data = std::fs::read(&items_path).expect("Failed to read document items");

            serde_json::from_slice(&data).expect("Failed to deserialize items")
        } else {
            log::info!("Processing PDF pages into images...");
            let images = crate::pdf::render_pdf(&doc, self.max_width, self.max_height)
                .expect("Failed to render PDF documents into images");

            if self.debug {
                log::info!("Saving debug images...");
                for (idx, image) in images.iter().enumerate() {
                    image
                        .save(Path::new("debug").join(format!("page_{idx}.png")))
                        .expect("Failed to save debug image");
                }
            }

            let extractor_config =
                ExtractorConfig::load(self.extractor).expect("Failed to load extractor config");
            let extractor = Extractor::new(extractor_config);

            log::info!("Running layout extraction on document...");

            let mut extracted_items = Vec::with_capacity(images.len() * 4);

            for (idx, image) in images.iter().enumerate() {
                extracted_items.extend(
                    extractor
                        .extract(image, idx)
                        .await
                        .expect("Failed to extract PDF items"),
                );
            }

            log::debug!("Extracted Items: {extracted_items:#?}");

            let analyzer_config =
                AnalyzerConfig::load(self.analyzer).expect("Failed to load analyzer config");
            let analyzer = Analyzer::new(analyzer_config);

            log::info!("Running analysis on extracted items...");
            let items = analyzer.analyze(&extracted_items).await;

            if let Some(items_path) = self.load_items {
                let data = serde_json::to_vec(&items).expect("Failed to serialize items");
                std::fs::write(&items_path, data).expect("Failed to write document items");
            }

            items
        };

        log::debug!("Evaluated Document Items: {items:#?}");

        log::info!("Annotating PDF document...");
        crate::pdf::annotate_pdf(doc, &self.output, &items)
            .expect("Failed to annotate PDF document");

        log::info!("Saved annotated PDF at '{}'.", self.output);
    }
}
