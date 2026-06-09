use std::fs::File;
use std::io::Read;
use syn::Expr;
use syn::File as SynFile;

fn main() {
    let mut file = File::open("lettuce/aof.salt").unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();

    let ast: SynFile = syn::parse_file(&content).unwrap();
    for item in ast.items {
        if let syn::Item::Fn(f) = item {
            if f.sig.ident == "aof_init" {
                println!("{:#?}", f.block);
            }
        }
    }
}
