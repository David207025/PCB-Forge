"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = require("vscode");
const editorProvider_1 = require("./editorProvider");
function activate(context) {
    console.log('PCB Forge extension is active!');
    // Ensure global cache/storage directory exists
    const cacheUri = context.globalStorageUri;
    vscode.workspace.fs.createDirectory(cacheUri).then(() => {
        console.log('Extension cache storage ready at:', cacheUri.fsPath);
    });
    // Register command to open the custom webview panel
    const disposable = vscode.commands.registerCommand('pcb-forge.openEditor', () => {
        editorProvider_1.PcbEditorPanel.createOrShow(context.extensionUri, cacheUri);
    });
    context.subscriptions.push(disposable);
}
function deactivate() {
}
//# sourceMappingURL=extension.js.map