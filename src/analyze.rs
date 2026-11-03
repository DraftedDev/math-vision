use ollama_rs::{
    Ollama, OllamaBuilder, generation::completion::request::GenerationRequest, models::ModelOptions,
};
use serde::{Deserialize, Serialize};

use crate::extract::ExtractedItem;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DocumentItem {
    pub page: usize,
    pub bbox_2d: [u32; 4], // [xmin, ymin, xmax, ymax]
    pub language: String,
    pub task: String,
    pub error: Option<String>,
    pub correction_latex: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    pub url: String,
    pub model: String,
    pub prompt: String,
    pub temp: f32,
    pub ctx: u64,
    pub predict: i32,
    pub top_p: f32,
}

pub struct Analyzer {
    client: Ollama,
    config: AnalyzerConfig,
}

impl Analyzer {
    pub fn new(config: AnalyzerConfig) -> Self {
        Self {
            client: OllamaBuilder::new().url(&config.url).build(),
            config,
        }
    }

    pub async fn analyze(&self, extracted_items: &[ExtractedItem]) -> Vec<DocumentItem> {
        let prompt_template = std::fs::read_to_string(&self.config.prompt)
            .expect("Failed to read prompt file")
            .trim()
            .to_string();

        let items_json = serde_json::to_string_pretty(extracted_items)
            .expect("Failed to serialize extracted items");

        let full_prompt = format!(
            "{}\n\nHere are the pre-extracted layout items and text segments from the document:\n{}\n\nAnalyze each item, evaluate its mathematical correctness, and return the complete JSON array with 'error' and 'correction_latex' populated.",
            prompt_template, items_json
        );

        let req = GenerationRequest::new(self.config.model.clone(), full_prompt).options(
            ModelOptions::default()
                .temperature(self.config.temp)
                .num_ctx(self.config.ctx)
                .num_predict(self.config.predict)
                .top_p(self.config.top_p),
        );

        log::info!(
            "Sending analysis request to Ollama text model '{}'...",
            self.config.model
        );
        let res = self
            .client
            .generate(req)
            .await
            .expect("Failed to send generation request to Ollama");

        log::debug!("Generation Response: {res:#?}");

        let raw_json = res.response.trim();
        let clean_json = raw_json
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        serde_json::from_str(clean_json)
            .expect("Failed to parse LLM output into DocumentItem array")
    }
}
