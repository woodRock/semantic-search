use anyhow::{Result, anyhow};
use std::path::Path;
use std::fs;
use hard_xml::XmlWrite;

pub fn extract_text(path: &Path) -> Result<String> {
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    
    match extension.as_str() {
        "pdf" => {
            let bytes = fs::read(path)?;
            pdf_extract::extract_text_from_mem(&bytes).map_err(|e| anyhow!("PDF error: {}", e))
        },
        "docx" => {
            let docx = docx_rust::DocxFile::from_file(path)?;
            let docx = docx.parse()?;
            Ok(docx.document.body.to_string()?)
        },
        "html" | "htm" => {
            let html = fs::read_to_string(path)?;
            Ok(html2text::from_read(html.as_bytes(), 80)?)
        },
        "txt" | "md" | "rs" | "py" | "js" | "ts" | "json" | "toml" | "yaml" | "yml" => {
            fs::read_to_string(path).map_err(Into::into)
        },
        _ => {
            // Try as text if it seems like text
            fs::read_to_string(path).map_err(|_| anyhow!("Unsupported file type: {}", extension))
        }
    }
}

pub fn get_metadata(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path)?;
    Ok(metadata.modified()?.duration_since(std::time::UNIX_EPOCH)?.as_secs())
}
