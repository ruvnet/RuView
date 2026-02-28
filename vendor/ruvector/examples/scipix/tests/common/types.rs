// Common types shared across tests
//
// Defines output formats, processing results, and configuration types

/// Output format for OCR processing
#[derive(Debug, Clone)]
pub enum OutputFormat {
    LaTeX,
    MathML,
    HTML,
    ASCII,
    All,
}

/// Processing options configuration
#[derive(Debug, Clone, Default)]
pub struct ProcessingOptions {
    pub enable_preprocessing: bool,
    pub enable_denoising: bool,
    pub enable_deskew: bool,
    pub include_latex: bool,
    pub include_mathml: bool,
    pub include_ascii: bool,
    pub include_text: bool,
}

/// Processing result from OCR
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    pub latex: String,
    pub mathml: Option<String>,
    pub html: Option<String>,
    pub ascii: Option<String>,
    pub text: Option<String>,
    pub confidence: f32,
    pub processing_time_ms: u64,
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub current_size: usize,
    pub max_size: usize,
}
