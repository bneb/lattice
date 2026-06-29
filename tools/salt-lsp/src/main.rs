//! Salt Language Server — Entry Point
//!
//! Launches the salt-lsp server over stdio using tower-lsp.
//! v0.3.0: semantic tokens, references, document symbols, code actions.

mod backend;
mod completion;
mod diagnostics;
pub mod sir_index;
mod sir_display;
mod semantic_tokens;
mod source_check;
#[cfg(test)]
mod tests_lsp;
#[cfg(test)]
mod source_check_test;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(backend::SaltBackend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
