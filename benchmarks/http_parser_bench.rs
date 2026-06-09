// =============================================================================
// HTTP Parser Benchmark — Rust Baseline
// =============================================================================
// Hand-rolled HTTP/1.1 parser for fair byte-level comparison with Salt.
// Uses memchr crate pattern (via Iterator::position) for delimiter scanning.
//
// Build: rustc -C opt-level=3 http_parser_bench.rs -o bin/http_parser_bench_rs
// =============================================================================

use std::time::Instant;

const ITERATIONS: usize = 1_000_000;
const MAX_HEADERS: usize = 32;

#[derive(Clone, Copy)]
struct Slice<'a> {
    data: &'a [u8],
}

impl<'a> Slice<'a> {
    fn new(data: &'a [u8]) -> Self {
        Slice { data }
    }

    fn len(&self) -> usize {
        self.data.len()
    }
}

struct HttpRequest<'a> {
    method: Slice<'a>,
    uri: Slice<'a>,
    version: Slice<'a>,
    header_count: usize,
    last_header_name: Slice<'a>,
    last_header_value: Slice<'a>,
}

fn parse_request(buf: &[u8]) -> Option<HttpRequest> {
    // Find end of request line (\r)
    let line_end = buf.iter().position(|&b| b == b'\r')?;
    let request_line = &buf[..line_end];

    // Find first space (method boundary)
    let sp1 = request_line.iter().position(|&b| b == b' ')?;
    let method = Slice::new(&request_line[..sp1]);

    // Find second space (URI boundary)
    let rest = &request_line[sp1 + 1..];
    let sp2_rel = rest.iter().position(|&b| b == b' ')?;
    let uri = Slice::new(&rest[..sp2_rel]);
    let version = Slice::new(&rest[sp2_rel + 1..]);

    // Parse headers
    let empty = Slice::new(&[]);
    let mut last_header_name = empty;
    let mut last_header_value = empty;
    let mut header_count = 0;

    let mut cursor = line_end + 2; // skip \r\n

    while cursor < buf.len() && header_count < MAX_HEADERS {
        // Check for empty line (end of headers)
        if cursor + 1 < buf.len() && buf[cursor] == b'\r' && buf[cursor + 1] == b'\n' {
            break;
        }

        // Find end of header line
        let remaining = &buf[cursor..];
        let hdr_end_rel = remaining.iter().position(|&b| b == b'\r')?;

        let header_line = &remaining[..hdr_end_rel];

        // Split on ':'
        if let Some(colon_pos) = header_line.iter().position(|&b| b == b':') {
            last_header_name = Slice::new(&header_line[..colon_pos]);

            // Trim leading whitespace from value
            let mut val_start = colon_pos + 1;
            while val_start < header_line.len() && header_line[val_start] == b' ' {
                val_start += 1;
            }
            last_header_value = Slice::new(&header_line[val_start..]);
            header_count += 1;
        }

        cursor += hdr_end_rel + 2; // skip \r\n
    }

    Some(HttpRequest {
        method,
        uri,
        version,
        header_count,
        last_header_name,
        last_header_value,
    })
}

fn main() {
    let request = b"GET /index.html HTTP/1.1\r\nHost: www.example.com\r\nUser-Agent: Salt/1.0\r\nAccept: text/html\r\nConnection: keep-alive\r\n\r\n";

    // Warm-up
    let warmup = parse_request(request).expect("parse failed");
    println!("Method len: {}", warmup.method.len());
    println!("URI len: {}", warmup.uri.len());
    println!("Version len: {}", warmup.version.len());
    println!("Headers: {}", warmup.header_count);

    // Benchmark
    let start = Instant::now();

    for i in 0..ITERATIONS {
        let req = parse_request(request).expect("parse failed");
        // Prevent dead code elimination
        std::hint::black_box(&req);
    }

    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_millis();
    let elapsed_ns = elapsed.as_nanos() as u64;
    let req_per_sec = (ITERATIONS as u64 * 1_000_000_000) / elapsed_ns;

    println!("=== Rust HTTP Parser Benchmark ===");
    println!("Iterations: {}", ITERATIONS);
    println!("Time: {} ms", elapsed_ms);
    println!("Throughput: {} req/s", req_per_sec);
}
