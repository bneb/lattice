use hpack::Decoder;
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <hpack_encoded_file>", args[0]);
        std::process::exit(1);
    }

    let data = fs::read(&args[1]).expect("Failed to read file");

    let mut decoder = Decoder::new();
    match decoder.decode(&data) {
        Ok(headers) => {
            println!("== HPACK IR ==");
            println!("HEADER_COUNT={}", headers.len());
            for (name, value) in &headers {
                let name_str = String::from_utf8_lossy(name);
                let val_str = String::from_utf8_lossy(value);
                println!("HDR {}={}", name_str, val_str);
            }
        }
        Err(e) => {
            eprintln!("HPACK decode error: {:?}", e);
            std::process::exit(1);
        }
    }
}
