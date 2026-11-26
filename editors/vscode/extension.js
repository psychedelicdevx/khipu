const vscode = require("vscode");
const { LanguageClient } = require("vscode-languageclient/node");

let client;

function activate(context) {
  const command = vscode.workspace.getConfiguration("khipu").inspect("path")?.globalValue || "khipu";
  client = new LanguageClient(
    "khipu",
    "Khipu",
    { command, args: ["lsp"] },
    { documentSelector: [{ language: "khipu" }] }
  );
  client.start().catch((error) => {
    vscode.window.showWarningMessage(
      `Khipu language server did not start (${error.message}). Install khipu or set khipu.path.`
    );
  });
  context.subscriptions.push({ dispose: () => client && client.stop() });
}

function deactivate() {
  return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
