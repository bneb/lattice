use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <raw_http_response_file>", args[0]);
        std::process::exit(1);
    }

    let data = fs::read(&args[1]).expect("Failed to read file");

    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut response = httparse::Response::new(&mut headers);

    match response.parse(&data) {
        Ok(httparse::Status::Complete(body_offset)) => {
            let status_code = response.code.unwrap_or(0);
            let header_count = response.headers.len();

            // Detect chunked transfer encoding
            let mut is_chunked = false;
            let mut content_length: i64 = -1;
            for header in response.headers.iter() {
                let name = header.name.to_lowercase();
                if name == "transfer-encoding" {
                    let val = String::from_utf8_lossy(header.value).to_lowercase();
                    if val.contains("chunked") {
                        is_chunked = true;
                    }
                }
                if name == "content-length" {
                    let val = String::from_utf8_lossy(header.value);
                    content_length = val.trim().parse().unwrap_or(-1);
                }
            }

            // Extract body
            let body = &data[body_offset..];
            let body_content = if is_chunked {
                decode_chunked(body)
            } else {
                body.to_vec()
            };

            println!("== HTTP LEXER IR ==");
            println!("STATUS={}", status_code);
            println!("HEADERS={}", header_count);
            println!("BODY_OFFSET={}", body_offset);
            println!("IS_CHUNKED={}", if is_chunked { 1 } else { 0 });
            if content_length >= 0 {
                println!("CONTENT_LENGTH={}", content_length);
            }
            println!("BODY_LEN={}", body_content.len());
            // Print first 256 bytes of body as hex for comparison
            let show = std::cmp::min(body_content.len(), 256);
            print!("BODY_HEAD=");
            for b in &body_content[..show] {
                print!("{:02x}", b);
            }
            println!();
        }
        Ok(httparse::Status::Partial) => {
            println!("== HTTP LEXER IR ==");
            println!("STATUS=PARTIAL");
        }
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Decode chunked transfer encoding
fn decode_chunked(data: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < data.len() {
        // Parse hex chunk size
        let mut size: usize = 0;
        while i < data.len() {
            let b = data[i];
            if b == b'\r' {
                i += 1; // skip \r
                continue;
            }
            if b == b'\n' {
                i += 1; // skip \n
                break;
            }
            size = size * 16 + hex_val(b) as usize;
            i += 1;
        }

        if size == 0 {
            break; // Terminal chunk
        }

        // Read exactly `size` bytes
        let end = std::cmp::min(i + size, data.len());
        result.extend_from_slice(&data[i..end]);
        i = end;

        // Skip trailing \r\n
        if i < data.len() && data[i] == b'\r' { i += 1; }
        if i < data.len() && data[i] == b'\n' { i += 1; }
    }

    result
}

fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}
