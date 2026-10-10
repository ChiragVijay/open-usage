use anyhow::{Result, anyhow, ensure};
use std::io::Read;

pub(crate) const AUTH_BYTES: usize = 1024 * 1024;
pub(crate) const RESPONSE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const PRICE_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn read(reader: impl Read, limit: usize) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|_| anyhow!("Input could not be read"))?;
    ensure!(data.len() <= limit, "Input exceeds the allowed size");
    Ok(data)
}

pub(crate) fn json(reader: impl Read, limit: usize) -> Result<serde_json::Value> {
    serde_json::from_slice(&read(reader, limit)?).map_err(|_| anyhow!("Input is not valid JSON"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stops_oversized_streams_even_without_a_content_length() {
        assert!(read(std::io::repeat(b'x'), 1024).is_err());
        assert_eq!(read(&b"1234"[..], 4).unwrap(), b"1234");
        assert!(read(&b"12345"[..], 4).is_err());
    }
    #[test]
    fn invalid_json_errors_do_not_include_the_input() {
        let error = json(&b"private-token-is-not-json"[..], 100).unwrap_err();
        assert_eq!(error.to_string(), "Input is not valid JSON");
    }
}
