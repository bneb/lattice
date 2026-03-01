// Salt Language — VS Code Extension (v0.2.0)
//
// Features:
//   - Syntax highlighting via TextMate grammar (always active)
//   - In-memory diagnostics via salt-front library (salt-lsp v0.2.0)
//   - Semantic hover: function signatures + Z3 contracts
//   - SIR-powered completions: functions + structs from compiled modules

import * as path from 'path';
import * as fs from 'fs';
import { workspace, ExtensionContext, window } from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

/** Find the salt-lsp binary in standard locations. */
function findLspBinary(extensionPath: string): string | undefined {
    // 1. User-configured path
    const configPath = workspace.getConfiguration('salt').get<string>('lspPath');
    if (configPath && fs.existsSync(configPath)) {
        return configPath;
    }

    // 2. Environment variable
    const envPath = process.env.SALT_LSP_PATH;
    if (envPath && fs.existsSync(envPath)) {
        return envPath;
    }

    // 3. Monorepo relative paths (release first for performance)
    const candidates = [
        path.join(extensionPath, '..', '..', 'target', 'release', 'salt-lsp'),
        path.join(extensionPath, '..', '..', 'target', 'debug', 'salt-lsp'),
        // From vscode extension dir → tools/salt-lsp/target
        path.join(extensionPath, '..', '..', '..', '..', 'target', 'debug', 'salt-lsp'),
    ];

    for (const candidate of candidates) {
        if (fs.existsSync(candidate)) {
            return candidate;
        }
    }

    return undefined;
}

export function activate(context: ExtensionContext) {
    // Syntax highlighting is provided by the TextMate grammar in package.json
    // — it works automatically without any code here.

    const serverPath = findLspBinary(context.extensionPath);

    if (!serverPath) {
        console.log('[Salt] LSP binary not found — syntax highlighting still active.');
        console.log('[Salt] Build with: cd tools/salt-lsp && cargo build');
        return;
    }

    try {
        const serverOptions: ServerOptions = {
            run: { command: serverPath },
            debug: { command: serverPath },
        };

        const clientOptions: LanguageClientOptions = {
            documentSelector: [{ scheme: 'file', language: 'salt' }],
            synchronize: {
                fileEvents: workspace.createFileSystemWatcher('**/*.salt'),
            },
        };

        client = new LanguageClient(
            'salt-lsp',
            'Salt Language Server',
            serverOptions,
            clientOptions
        );

        client.start().catch((err: Error) => {
            console.log(`[Salt] LSP failed to start: ${err.message} — syntax highlighting still active.`);
            client = undefined;
        });
    } catch (err) {
        console.log('[Salt] LSP initialization error — syntax highlighting still active.');
        client = undefined;
    }
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
