mod extractor;
mod index;
mod query;

pub use extractor::{
    extract_epub_text, extract_pdf_text, extract_pdf_text_from_bytes, ExtractError, ExtractedText,
};
pub use index::{
    extract_parallel, ExtractionJob, ExtractionResult, IndexError, IndexStats, SearchIndex,
    SearchSchema,
};
pub use query::{format_search_results, QueryError, SearchOptions, SearchResult};
