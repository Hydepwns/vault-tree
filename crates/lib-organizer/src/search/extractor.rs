use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse PDF: {0}")]
    Pdf(String),
    #[error("failed to parse EPUB: {0}")]
    Epub(String),
}

#[derive(Debug, Clone)]
pub struct ExtractedText {
    pub content: String,
    pub page_count: Option<u32>,
}

impl ExtractedText {
    pub fn empty() -> Self {
        Self {
            content: String::new(),
            page_count: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

pub fn extract_pdf_text(path: &Path) -> Result<ExtractedText, ExtractError> {
    let bytes = std::fs::read(path)?;
    extract_pdf_text_from_bytes(&bytes)
}

pub fn extract_pdf_text_from_bytes(bytes: &[u8]) -> Result<ExtractedText, ExtractError> {
    match pdf_extract::extract_text_from_mem(bytes) {
        Ok(text) => {
            let page_count = estimate_page_count(&text);
            let content = normalize_text(&text);
            Ok(ExtractedText {
                content,
                page_count,
            })
        }
        Err(e) => {
            // Return empty for encrypted/corrupted PDFs instead of failing
            if is_recoverable_error(&e) {
                Ok(ExtractedText::empty())
            } else {
                Err(ExtractError::Pdf(e.to_string()))
            }
        }
    }
}

fn is_recoverable_error(err: &pdf_extract::OutputError) -> bool {
    let msg = err.to_string().to_lowercase();
    msg.contains("encrypted")
        || msg.contains("password")
        || msg.contains("corrupt")
        || msg.contains("invalid")
}

fn normalize_text(text: &str) -> String {
    text.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn estimate_page_count(text: &str) -> Option<u32> {
    // Form feed characters often separate pages in PDF text extraction
    let ff_count = text.matches('\x0C').count();
    if ff_count > 0 {
        Some(ff_count as u32 + 1)
    } else {
        None
    }
}

pub fn extract_epub_text(path: &Path) -> Result<ExtractedText, ExtractError> {
    let mut doc = epub::doc::EpubDoc::new(path).map_err(|e| ExtractError::Epub(e.to_string()))?;

    let mut content = String::new();
    let num_pages = doc.spine.len();

    loop {
        if let Some((chapter_content, _mime)) = doc.get_current_str() {
            let plain = strip_html(&chapter_content);
            if !plain.is_empty() {
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(&plain);
            }
        }

        if !doc.go_next() {
            break;
        }
    }

    let content = normalize_text(&content);
    Ok(ExtractedText {
        content,
        page_count: Some(num_pages as u32),
    })
}

fn strip_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    let mut last_was_space = true;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => {
                if c.is_whitespace() {
                    if !last_was_space {
                        result.push(' ');
                        last_was_space = true;
                    }
                } else {
                    result.push(c);
                    last_was_space = false;
                }
            }
            _ => {}
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_extraction() {
        let extracted = ExtractedText::empty();
        assert!(extracted.is_empty());
        assert_eq!(extracted.page_count, None);
    }

    #[test]
    fn normalize_text_removes_empty_lines() {
        let input = "hello\n\n  world  \n\n\ntest";
        let output = normalize_text(input);
        assert_eq!(output, "hello\nworld\ntest");
    }

    #[test]
    fn estimate_page_count_from_form_feeds() {
        let text = "page1\x0Cpage2\x0Cpage3";
        assert_eq!(estimate_page_count(text), Some(3));
    }

    #[test]
    fn estimate_page_count_no_form_feeds() {
        let text = "single page content";
        assert_eq!(estimate_page_count(text), None);
    }

    #[test]
    fn extract_nonexistent_file() {
        let result = extract_pdf_text(Path::new("/nonexistent/file.pdf"));
        assert!(result.is_err());
    }
}
