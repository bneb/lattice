//! Salt Language Server — Entry Point
//!
//! Launches the salt-lsp server over stdio using tower-lsp.

mod backend;
mod completion;
mod diagnostics;
pub mod sir_index;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| backend::SaltBackend::new(client));

    Server::new(stdin, stdout, socket).serve(service).await;
}
