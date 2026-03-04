use anyhow::{Result, anyhow};
use std::path::Path;
use std::fs;
use hard_xml::XmlWrite;

use std::io::Read;

pub fn extract_text(path: &Path) -> Result<String> {
    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    
    // Skip files larger than 1MB (configurable) for semantic search
    if size > 1_048_576 {
        return Err(anyhow!("File too large for indexing: {} bytes", size));
    }

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
        "txt" | "md" | "rs" | "py" | "js" | "ts" | "json" | "toml" | "yaml" | "yml" | "css" | "scss" | "sql" | "c" | "cpp" | "h" | "hpp" | "go" | "sh" | "bash" | "zsh" | "rb" | "php" | "java" | "kt" | "swift" | "m" | "mm" | "pl" | "pm" | "t" | "xml" | "csv" | "log" => {
            fs::read_to_string(path).map_err(Into::into)
        },
        _ => {
            // Check for binary content before reading
            let mut file = fs::File::open(path)?;
            let mut buffer = [0; 1024];
            let n = file.read(&mut buffer)?;
            if buffer[..n].iter().any(|&b| b == 0) {
                return Err(anyhow!("Binary file detected: {}", extension));
            }
            
            // Try as text if it seems like text
            fs::read_to_string(path).map_err(|_| anyhow!("Unsupported or binary file type: {}", extension))
        }
    }
}

pub fn get_metadata(path: &Path) -> Result<u64> {
    let metadata = fs::metadata(path)?;
    Ok(metadata.modified()?.duration_since(std::time::UNIX_EPOCH)?.as_secs())
}
