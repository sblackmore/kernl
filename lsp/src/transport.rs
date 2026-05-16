use std::io::{BufRead, Write};

use crate::error::LspError;

pub fn read_message(reader: &mut impl BufRead) -> Result<serde_json::Value, LspError> {
    let mut header_line = String::new();
    reader.read_line(&mut header_line)?;

    if header_line.is_empty() {
        return Err(LspError::Protocol("unexpected EOF reading header".into()));
    }

    let content_length = header_line
        .strip_prefix("Content-Length: ")
        .and_then(|s| s.trim().parse::<usize>().ok())
        .ok_or_else(|| LspError::Protocol(format!("bad Content-Length header: {header_line:?}")))?;

    // Read remaining headers until blank line (\r\n)
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line == "\r\n" || line == "\n" || line.is_empty() {
            break;
        }
    }

    let mut body = vec![0u8; content_length];
    reader.read_exact(&mut body)?;

    let value: serde_json::Value = serde_json::from_slice(&body)?;
    Ok(value)
}

pub fn write_message(writer: &mut impl Write, msg: &serde_json::Value) -> Result<(), LspError> {
    let body = serde_json::to_string(msg)?;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer.write_all(header.as_bytes())?;
    writer.write_all(body.as_bytes())?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn round_trip_message() {
        let msg = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "test"});

        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let mut reader = Cursor::new(buf);
        let parsed = read_message(&mut reader).unwrap();

        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "test");
    }

    #[test]
    fn read_message_with_crlf_header() {
        let body = r#"{"jsonrpc":"2.0","id":42,"method":"initialize"}"#;
        let raw = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        let mut reader = Cursor::new(raw.into_bytes());
        let parsed = read_message(&mut reader).unwrap();

        assert_eq!(parsed["id"], 42);
        assert_eq!(parsed["method"], "initialize");
    }

    #[test]
    fn write_produces_content_length_header() {
        let msg = serde_json::json!({"ok": true});
        let mut buf = Vec::new();
        write_message(&mut buf, &msg).unwrap();

        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with("Content-Length: "));
        assert!(output.contains("\r\n\r\n"));
    }

    #[test]
    fn read_empty_input_returns_error() {
        let mut reader = Cursor::new(Vec::<u8>::new());
        assert!(read_message(&mut reader).is_err());
    }
}
