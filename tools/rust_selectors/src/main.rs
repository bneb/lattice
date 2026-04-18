use scraper::{Html, Selector};
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <html_file> <selector>", args[0]);
        std::process::exit(1);
    }

    let html_content = fs::read_to_string(&args[1]).expect("Failed to read HTML");
    let selector_str = &args[2];

    let document = Html::parse_fragment(&html_content);
    let selector = Selector::parse(selector_str).expect("Failed to parse selector");

    println!("== SELECTOR IR ==");
    
    // Scraper provides a collection of elements. We just need to check if ANY match
    // for our current Salt differential test (which returns the first match)
    if let Some(element) = document.select(&selector).next() {
        // We need a way to identify the element index to compare with Salt
        // In Salt, indices are based on DOM insertion order.
        // We'll use the 'data-index' attribute if present to match the IR.
        let idx = element.value().attr("data-index").unwrap_or("0");
        println!("MATCH {}", idx);
    } else {
        println!("MATCH NONE");
    }
}
