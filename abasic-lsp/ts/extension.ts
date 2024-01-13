import * as path from "path";
import { workspace, ExtensionContext } from "vscode";

import { Executable, LanguageClient, LanguageClientOptions, ServerOptions, Trace, TransportKind } from "vscode-languageclient/node";

let client: LanguageClient;

export function activate(context: ExtensionContext) {
    console.log("WOOO ACTIVATE", context);
    const server = context.asAbsolutePath("../target/debug/abasic-lsp");
    const run: Executable = {
        command: server, transport: TransportKind.stdio
    };
    const serverOptions: ServerOptions = run;
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "abasic" }],
    };
    client = new LanguageClient("abasicLanguageServer", "ABASIC Language Server", serverOptions, clientOptions);
    client.setTrace(Trace.Verbose);
    console.log("Initialized language server maybe.", client.traceOutputChannel.name, server);
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
