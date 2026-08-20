import * as vscode from 'vscode';

export function activate(context: vscode.ExtensionContext) {
  // 1. Register the native tree data provider
  const treeDataProvider = new PcbForgeTreeProvider();
  vscode.window.registerTreeDataProvider('pcbForgeControlPanel', treeDataProvider);

  // 2. Register the command that opens your HTML/React page (in an Editor tab or panel)
  let openDashboardCommand = vscode.commands.registerCommand('pcb-forge.openDashboard', () => {
    openWebviewPanel(context);
  });

  context.subscriptions.push(openDashboardCommand);
}

// Native Sidebar List Provider
class PcbForgeTreeProvider implements vscode.TreeDataProvider<vscode.TreeItem> {
  getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(_element?: vscode.TreeItem): Thenable<vscode.TreeItem[]> {
    const items: vscode.TreeItem[] = [];

    // Native item acting as a button/action link
    const actionItem = new vscode.TreeItem('Run Pre-check', vscode.TreeItemCollapsibleState.None);
    actionItem.iconPath = new vscode.ThemeIcon('play');
    actionItem.command = {
      command: 'pcb-forge.openDashboard',
      title: 'Run Pre-check'
    };
    items.push(actionItem);

    return Promise.resolve(items);
  }
}

// Function to open your HTML-based React UI when requested
function openWebviewPanel(context: vscode.ExtensionContext) {
  const panel = vscode.window.createWebviewPanel(
    'pcbForgeWebview',
    'PCB Forge Dashboard',
    vscode.ViewColumn.One,
    {enableScripts: true}
  );

  panel.webview.html = `<!DOCTYPE html>
    <html>
      <head><meta charset="UTF-8"></head>
      <body>
        <h1>Interactive React UI Loaded Here</h1>
      </body>
    </html>`;
}