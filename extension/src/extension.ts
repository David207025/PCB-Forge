import * as vscode from 'vscode';
import {PcbEditorPanel} from './editorProvider';

export function activate(context: vscode.ExtensionContext) {
  console.log('PCB Forge extension is active!');

  // Ensure global cache/storage directory exists
  const cacheUri = context.globalStorageUri;
  vscode.workspace.fs.createDirectory(cacheUri).then(() => {
    console.log('Extension cache storage ready at:', cacheUri.fsPath);
  });

  // Register command to open the custom webview panel
  const disposable = vscode.commands.registerCommand('pcb-forge.openEditor', () => {
    PcbEditorPanel.createOrShow(context.extensionUri, cacheUri);
  });

  context.subscriptions.push(disposable);
}

export function deactivate() {
}