import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { spawn, ChildProcess } from 'child_process';

const API_BASE = 'http://127.0.0.1:47210';

let activeWebviewPanel: vscode.WebviewPanel | undefined = undefined;
let cliProcess: ChildProcess | undefined = undefined;
let isCliAvailable: boolean = true;

export function activate(context: vscode.ExtensionContext) {
  checkCliAvailability();
  startCliProcess();

  // Primary command: opens the root home screen inside the webview panel
  context.subscriptions.push(
    vscode.commands.registerCommand('pcb-forge.openDashboard', (routePath?: string) => {
      const targetPath = routePath || '/';
      const panel = ensureWebviewLoaded(context);
      panel.reveal(vscode.ViewColumn.One);
      panel.webview.postMessage({ command: 'navigate', path: targetPath });
    })
  );

  // Command: Create Template (/init-template)
  context.subscriptions.push(
    vscode.commands.registerCommand('pcb-forge.createTemplate', async () => {
      const templateName = await vscode.window.showInputBox({
        prompt: 'Enter new template name',
        placeHolder: 'e.g., default-a4'
      });
      if (!templateName) return;

      try {
        const res = await fetch(`${API_BASE}/init-template`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ name: templateName })
        });
        const body = await res.json() as { status: string; message: string };

        if (res.ok && body.status === 'success') {
          vscode.window.showInformationMessage(body.message);
          notifyWebviewTemplateUpdate();
        } else {
          vscode.window.showErrorMessage(body.message || 'Failed to initialize template.');
        }
      } catch (err: any) {
        vscode.window.showErrorMessage(`API Connection Error: ${err.message}`);
      }
    })
  );

  // Command: Create Project (/init-project)
  context.subscriptions.push(
    vscode.commands.registerCommand('pcb-forge.createProject', async () => {
      const templateName = await vscode.window.showInputBox({
        prompt: 'Enter target template name',
        placeHolder: 'e.g., standard-template'
      });
      if (!templateName) return;

      const folderUri = await vscode.window.showOpenDialog({
        canSelectFiles: false,
        canSelectFolders: true,
        canSelectMany: false,
        openLabel: 'Select Project Directory'
      });
      if (!folderUri || folderUri.length === 0) return;

      const targetPath = path.join(folderUri[0].fsPath, `${templateName}.json`);

      try {
        const res = await fetch(`${API_BASE}/init-project`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ template: templateName, path: targetPath })
        });
        const body = await res.json() as { status: string; message: string };

        if (res.ok && body.status === 'success') {
          vscode.window.showInformationMessage(body.message);
        } else {
          vscode.window.showErrorMessage(body.message || 'Failed to initialize project.');
        }
      } catch (err: any) {
        vscode.window.showErrorMessage(`API Connection Error: ${err.message}`);
      }
    })
  );

  // Command: Generate Schemas for All Templates (/gen-templates)
  context.subscriptions.push(
    vscode.commands.registerCommand('pcb-forge.generateTemplates', async () => {
      try {
        const res = await fetch(`${API_BASE}/gen-templates`, { method: 'POST' });
        const body = await res.json() as { status: string; message: string };

        if (res.ok && body.status === 'success') {
          vscode.window.showInformationMessage(body.message);
          notifyWebviewTemplateUpdate();
        } else {
          vscode.window.showErrorMessage(body.message || 'Failed to generate templates.');
        }
      } catch (err: any) {
        vscode.window.showErrorMessage(`API Connection Error: ${err.message}`);
      }
    })
  );

  // Command: Generate Project PDF (/gen-project)
  context.subscriptions.push(
    vscode.commands.registerCommand('pcb-forge.generateProject', async () => {
      const workspaceFolders = vscode.workspace.workspaceFolders;
      if (!workspaceFolders) {
        vscode.window.showErrorMessage('No workspace open in VS Code.');
        return;
      }

      const rootPath = workspaceFolders[0].uri.fsPath;
      const envPath = path.join(rootPath, '.env');
      let targetFile: string | null = null;

      if (fs.existsSync(envPath)) {
        const envContent = fs.readFileSync(envPath, 'utf8');
        const match = envContent.match(/^PROJECT_PATH=(.+)$/m);
        if (match) targetFile = match[1].trim();
      }

      if (!targetFile) {
        const fileUri = await vscode.window.showOpenDialog({
          canSelectFiles: true,
          canSelectFolders: false,
          filters: { 'JSON Project': ['json'] },
          openLabel: 'Select Project File'
        });
        if (!fileUri || fileUri.length === 0) return;
        targetFile = fileUri[0].fsPath;
      }

      try {
        const res = await fetch(`${API_BASE}/gen-project`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ path: targetFile })
        });
        const body = await res.json() as { status: string; message: string };

        if (res.ok && body.status === 'success') {
          vscode.window.showInformationMessage(body.message);
        } else {
          vscode.window.showErrorMessage(body.message || 'Failed to generate project.');
        }
      } catch (err: any) {
        vscode.window.showErrorMessage(`API Connection Error: ${err.message}`);
      }
    })
  );
}

export function deactivate() {
  if (cliProcess) {
    cliProcess.kill();
    cliProcess = undefined;
  }
}

function checkCliAvailability() {
  const checkCommand = process.platform === 'win32' ? 'where pcbfapi' : 'which pcbfapi';
  const { exec } = require('child_process');
  exec(checkCommand, (error: any) => {
    isCliAvailable = !error;
  });
}

function startCliProcess() {
  try {
    cliProcess = spawn('pcbfapi', [], { detached: false, shell: true });
    cliProcess.stdout?.on('data', (d) => console.log(`[CLI STDOUT]: ${d}`));
    cliProcess.stderr?.on('data', (d) => console.error(`[CLI STDERR]: ${d}`));
  } catch (e) {
    console.error('Failed to launch pcbfapi:', e);
  }
}

function ensureWebviewLoaded(context: vscode.ExtensionContext): vscode.WebviewPanel {
  if (activeWebviewPanel) return activeWebviewPanel;

  activeWebviewPanel = vscode.window.createWebviewPanel(
    'pcbForgeWebview',
    'PCB Forge',
    vscode.ViewColumn.One,
    { enableScripts: true, retainContextWhenHidden: true }
  );

  activeWebviewPanel.webview.html = getWebviewHtml(context, activeWebviewPanel.webview);

  // Handle messages sent from React frontend to extension
  activeWebviewPanel.webview.onDidReceiveMessage(async (message) => {
    switch (message.command) {
      case 'requestTemplatesState':
        sendTemplatesState();
        break;
      case 'runVSCodeCommand':
        vscode.commands.executeCommand(message.commandName);
        break;
    }
  });

  activeWebviewPanel.onDidDispose(() => {
    activeWebviewPanel = undefined;
  }, null, context.subscriptions);

  return activeWebviewPanel;
}

/** Reads ~/.pcb-forge/templates to determine generated vs non-generated status */
function sendTemplatesState() {
  if (!activeWebviewPanel) return;

  const homeDir = os.homedir();
  const srcDir = path.join(homeDir, '.pcb-forge', 'templates', 'src');
  const genDir = path.join(homeDir, '.pcb-forge', 'templates', 'generated');

  const templates: Array<{ name: string; isGenerated: boolean }> = [];

  if (fs.existsSync(srcDir)) {
    const folders = fs.readdirSync(srcDir, { withFileTypes: true });
    for (const folder of folders) {
      if (folder.isDirectory()) {
        const schemaPath = path.join(genDir, `${folder.name}.schema.json`);
        templates.push({
          name: folder.name,
          isGenerated: fs.existsSync(schemaPath)
        });
      }
    }
  }

  activeWebviewPanel.webview.postMessage({
    command: 'setTemplatesState',
    templates
  });
}

function notifyWebviewTemplateUpdate() {
  setTimeout(() => sendTemplatesState(), 500);
}

function getWebviewHtml(context: vscode.ExtensionContext, webview: vscode.Webview): string {
  const htmlPath = path.join(context.extensionPath, 'dist', 'index.html');
  if (!fs.existsSync(htmlPath)) {
    return `<!DOCTYPE html><html><body><h2>Web UI Build Not Found</h2></body></html>`;
  }

  let html = fs.readFileSync(htmlPath, 'utf8');
  const scriptUri = webview.asWebviewUri(vscode.Uri.file(path.join(context.extensionPath, 'dist', 'assets', 'index.js')));
  const stylesUri = webview.asWebviewUri(vscode.Uri.file(path.join(context.extensionPath, 'dist', 'assets', 'index.css')));

  html = html
    .replace(/<script.*?src="([^"]*?)".*?>.*?<\/script>/is, `<script type="module" src="${scriptUri}"></script>`)
    .replace(/<link rel="stylesheet".*?href="([^"]*?)".*?>/is, `<link rel="stylesheet" href="${stylesUri}">`);

  const cspMeta = `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src ${webview.cspSource} 'unsafe-eval'; img-src ${webview.cspSource} https:; connect-src ${webview.cspSource} http://127.0.0.1:47210;">`;
  const cliScript = `<script>window.IS_CLI_AVAILABLE = ${isCliAvailable};</script>`;

  return html.replace('<head>', `<head>\n    ${cspMeta}\n    ${cliScript}`);
}