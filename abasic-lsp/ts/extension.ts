import * as path from "path";
import * as net from "net";
import { workspace, ExtensionContext } from "vscode";

import { Executable, LanguageClient, LanguageClientOptions, ServerOptions, StreamInfo, Trace, TransportKind } from "vscode-languageclient/node";

let client: LanguageClient;

function connectToLanguageServer(): Promise<StreamInfo> {
    return new Promise((resolve, reject) => {
        console.log("Attempting to connect to language server...");
        let socket = net.connect({
            port: 5007,
            host: "127.0.0.1",
        });
        socket.once('connect', () => {
            console.log("Connected to language server!");
            socket.removeAllListeners();
            resolve({
                writer: socket,
                reader: socket
            });
        });
        socket.once('error', (err) => {
            const code = (err as any).code;
            if (code === "ECONNREFUSED") {
                socket.removeAllListeners();
                console.log("Connection refused, retrying...");
                setTimeout(() => {
                    resolve(connectToLanguageServer());
                }, 1000);
            } else {
                reject(err);
            }
        });
    });
}

export function activate(context: ExtensionContext) {
    /*
    const server = context.asAbsolutePath("../target/debug/abasic-lsp");
    const run: Executable = {
        command: server, transport: TransportKind.stdio
    };
    const serverOptions: ServerOptions = run;*/
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: "file", language: "abasic" }],
    };
    client = new LanguageClient("abasicLanguageServer", "ABASIC Language Server", connectToLanguageServer, clientOptions);
    client.setTrace(Trace.Verbose);
    console.log("Initialized language server.", client.traceOutputChannel.name);
    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
