import * as path from "path";
import { ExtensionContext } from "vscode";

import {
  LanguageClient,
  LanguageClientOptions,
  Trace,
} from "vscode-languageclient/node";

let client: LanguageClient;

// A lot of this is based off: https://code.visualstudio.com/api/language-extensions/language-server-extension-guide
export function activate(context: ExtensionContext) {
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "abasic" }],
  };
  client = new LanguageClient(
    "abasicLanguageServer",
    "ABASIC Language Server",
    {
      run: {
        // Note that the server needs to be installed somewhere on the PATH.
        command: "abasic-lsp",
        // Note that we're not specifying any kind of 'transport'. This is
        // an extremely confusing property and seems to only be relevant
        // for node-based LSP servers--it just ends up confusing VSCode if we
        // use it for our LSP. If we leave it out entirely, VSCode just uses
        // stdio to communicate with the LSP server and everything works fine.
      },
      debug: {
        command: context.asAbsolutePath(
          path.join("..", "target", "debug", "abasic-lsp")
        ),
      },
    },
    clientOptions
  );
  client.setTrace(Trace.Verbose);
  console.log("Initialized language server.", client.traceOutputChannel.name);
  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  console.log("Shutting down client.");
  return client.stop();
}
