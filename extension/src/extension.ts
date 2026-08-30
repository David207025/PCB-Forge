import * as vscode from 'vscode';
import * as path from "path";
import * as fs from "fs";
import { spawn, ChildProcess } from 'child_process';

let activeWebviewPanel: vscode.WebviewPanel | undefined = undefined;
let currentActiveRoute: string = '/';
let treeProviderInstance: PcbForgeTreeProvider | undefined = undefined;
let cliProcess: ChildProcess | undefined = undefined;
let isCliAvailable: boolean = true;

export function activate(context: vscode.ExtensionContext) {
  checkCliAvailability();
  startCliProcess();

  treeProviderInstance = new PcbForgeTreeProvider();
  vscode.window.registerTreeDataProvider('pcbForgeControlPanel', treeProviderInstance);

  let openDashboardCommand = vscode.commands.registerCommand('pcb-forge.openDashboard', (routePath?: string) => {
    const targetPath = routePath || '/';
    currentActiveRoute = targetPath;
    treeProviderInstance?.refresh();

    const panel = ensureWebviewLoaded(context);
    if (panel) {
      panel.reveal(vscode.ViewColumn.One);
      panel.webview.postMessage({ command: 'navigate', path: targetPath });
    }
  });

  context.subscriptions.push(openDashboardCommand);
}

export function deactivate() {
  if (cliProcess) {
    console.log('🛑 Stopping PCB Forge background CLI process...');
    cliProcess.kill();
    cliProcess = undefined;
  }
}

function checkCliAvailability() {
  const cliBinaryName = 'pcbfapi';
  const checkCommand = process.platform === 'win32' ? `where ${cliBinaryName}` : `which ${cliBinaryName}`;
  execAsyncCommand(checkCommand, (success) => {
    isCliAvailable = success;
    treeProviderInstance?.refresh();
  });
}

function execAsyncCommand(command: string, callback: (exists: boolean) => void) {
  const { exec } = require('child_process');
  exec(command, (error: any) => {
    callback(!error);
  });
}

function startCliProcess() {
  try {
    console.log('🚀 Attempting to start background CLI process...');

    // Replace 'agy-ide' with your actual background CLI command or daemon binary if it's separate
    cliProcess = spawn('pcbfapi', [], {
      detached: false,
      shell: true // Uses system shell so it resolves PATH variables correctly
    });

    cliProcess.stdout?.on('data', (data) => {
      console.log(`[CLI STDOUT]: ${data.toString()}`);
    });

    cliProcess.stderr?.on('data', (data) => {
      console.error(`[CLI STDERR]: ${data.toString()}`);
    });

    cliProcess.on('error', (err) => {
      console.error('❌ Failed to start CLI process spawn:', err);
    });

    cliProcess.on('exit', (code, signal) => {
      console.log(`⚠️ CLI process exited with code ${code} and signal ${signal}`);
    });

  } catch (e) {
    console.error('❌ Exception starting CLI process:', e);
  }
}

function ensureWebviewLoaded(context: vscode.ExtensionContext): vscode.WebviewPanel {
  if (activeWebviewPanel) {
    activeWebviewPanel.reveal(vscode.ViewColumn.One);
    return activeWebviewPanel;
  }

  activeWebviewPanel = vscode.window.createWebviewPanel(
    'pcbForgeWebview',
    'PCB Forge Dashboard',
    vscode.ViewColumn.One,
    {
      enableScripts: true,
      retainContextWhenHidden: true
    }
  );

  activeWebviewPanel.webview.html = getWebviewHtml(context, activeWebviewPanel.webview);

  activeWebviewPanel.onDidDispose(() => {
    activeWebviewPanel = undefined;
    currentActiveRoute = '';
    treeProviderInstance?.refresh();
  }, null, context.subscriptions);

  return activeWebviewPanel;
}

function getWebviewHtml(context: vscode.ExtensionContext, webview: vscode.Webview): string {
  const htmlPath = path.join(context.extensionPath, 'web', 'index.html');

  if (!fs.existsSync(htmlPath)) {
    return `<!DOCTYPE html><html><body><h2>Web UI Build Not Found</h2></body></html>`;
  }

  let htmlContent = fs.readFileSync(htmlPath, 'utf8');

  const scriptUri = webview.asWebviewUri(
    vscode.Uri.file(path.join(context.extensionPath, 'web', 'assets', 'index.js'))
  );
  const stylesUri = webview.asWebviewUri(
    vscode.Uri.file(path.join(context.extensionPath, 'web', 'assets', 'index.css'))
  );

  htmlContent = htmlContent
    .replace(/<script.*?src="([^"]*?)".*?>.*?<\/script>/is, `<script type="module" src="${scriptUri}"></script>`)
    .replace(/<link rel="stylesheet".*?href="([^"]*?)".*?>/is, `<link rel="stylesheet" href="${stylesUri}">`);

  if (!htmlContent.includes(scriptUri.toString())) {
    htmlContent = htmlContent
      .replace(/"assets\/index\.js"/g, `"${scriptUri}"`)
      .replace(/"assets\/index\.css"/g, `"${stylesUri}"`)
      .replace(/\/assets\/index\.js/g, scriptUri.toString())
      .replace(/\/assets\/index\.css/g, stylesUri.toString());
  }

  const cliStatusScript = `<script>window.IS_CLI_AVAILABLE = ${isCliAvailable};</script>`;
  const cspMeta = `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src ${webview.cspSource} 'unsafe-eval'; img-src ${webview.cspSource} https:; connect-src ${webview.cspSource};">`;
  htmlContent = htmlContent.replace('<head>', `<head>\n    ${cspMeta}\n    ${cliStatusScript}`);

  return htmlContent;
}

class TreeCategoryItem extends vscode.TreeItem {
  constructor(public readonly label: string, collapsibleState: vscode.TreeItemCollapsibleState, icon: string) {
    super(label, collapsibleState);
    this.iconPath = new vscode.ThemeIcon(icon);
  }
}

class PcbForgeTreeProvider implements vscode.TreeDataProvider<vscode.TreeItem> {
  private _onDidChangeTreeData: vscode.EventEmitter<vscode.TreeItem | undefined | void> = new vscode.EventEmitter<vscode.TreeItem | undefined | void>();
  readonly onDidChangeTreeData: vscode.Event<vscode.TreeItem | undefined | void> = this._onDidChangeTreeData.event;

  refresh(): void {
    this._onDidChangeTreeData.fire();
  }

  getTreeItem(element: vscode.TreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: vscode.TreeItem): Thenable<vscode.TreeItem[]> {
    if (!element) {
      const rootItems: vscode.TreeItem[] = [];

      const tabsFolder = new TreeCategoryItem('Tabs', vscode.TreeItemCollapsibleState.Expanded, 'folder');
      rootItems.push(tabsFolder);

      const actionsFolder = new TreeCategoryItem('Actions', vscode.TreeItemCollapsibleState.Expanded, 'folder');
      rootItems.push(actionsFolder);

      if (!isCliAvailable) {
        const warningItem = new TreeCategoryItem('CLI Missing!', vscode.TreeItemCollapsibleState.None, 'warning');
        warningItem.description = 'Not found on PATH';
        rootItems.push(warningItem);
      }

      return Promise.resolve(rootItems);
    }

    if (element.label === 'Tabs') {
      const isPanelOpen = activeWebviewPanel !== undefined;
      const isHomeActive = isPanelOpen && currentActiveRoute === '/';
      const isSettingsActive = isPanelOpen && currentActiveRoute === '/settings';

      const homeItem = new vscode.TreeItem('Home Dashboard', vscode.TreeItemCollapsibleState.None);
      homeItem.iconPath = new vscode.ThemeIcon('home'); // Icon preserved unchanged
      homeItem.description = isHomeActive ? '● Active' : '';
      homeItem.command = {
        command: 'pcb-forge.openDashboard',
        title: 'Open Home',
        arguments: ['/']
      };

      const settingsItem = new vscode.TreeItem('Settings', vscode.TreeItemCollapsibleState.None);
      settingsItem.iconPath = new vscode.ThemeIcon('settings'); // Icon preserved unchanged
      settingsItem.description = isSettingsActive ? '● Active' : '';
      settingsItem.command = {
        command: 'pcb-forge.openDashboard',
        title: 'Open Settings',
        arguments: ['/settings']
      };

      return Promise.resolve([homeItem, settingsItem]);
    }

    if (element.label === 'Actions') {
      const preCheckItem = new vscode.TreeItem('Run Pre-check', vscode.TreeItemCollapsibleState.None);
      preCheckItem.iconPath = new vscode.ThemeIcon('play');
      preCheckItem.command = {
        command: 'pcb-forge.openDashboard',
        title: 'Run Pre-check',
        arguments: ['/pre-check'] // Fixed: uses its own dedicated route instead of triggering '/'
      };

      return Promise.resolve([preCheckItem]);
    }

    return Promise.resolve([]);
  }
}