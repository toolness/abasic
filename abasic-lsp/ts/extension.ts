import * as path from "path";
import * as net from "net";
import { ExtensionContext } from "vscode";

import {
  LanguageClient,
  LanguageClientOptions,
  StreamInfo,
  Trace,
} from "vscode-languageclient/node";

let client: LanguageClient;

function connectToLanguageServer(): Promise<StreamInfo> {
  return new Promise((resolve, reject) => {
    console.log("Attempting to connect to language server...");
    let socket = net.connect({
      port: 5007,
      host: "127.0.0.1",
    });
    socket.once("connect", () => {
      console.log("Connected to language server!");
      socket.removeAllListeners();
      resolve({
        writer: socket,
        reader: socket,
      });
    });
    socket.once("error", (err) => {
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

// A lot of this is based off: https://code.visualstudio.com/api/language-extensions/language-server-extension-guide
export function activate(context: ExtensionContext) {
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "abasic" }],
  };
  client = new LanguageClient(
    "abasicLanguageServer",
    "ABASIC Language Server",
    connectToLanguageServer,
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
