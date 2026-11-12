import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { exec } from 'child_process';
import { promisify } from 'util';
import {registerJsonPathPicker} from "./pathPicker";

const execAsync = promisify(exec);
const API_BASE = 'http://127.0.0.1:47210';

let activeWebviewPanel: vscode.WebviewPanel | undefined = undefined;
let isCliAvailable: boolean = false;
let statusBarItem: vscode.StatusBarItem | undefined = undefined;
let healthCheckTimer: NodeJS.Timeout | undefined = undefined;

export function activate(context: vscode.ExtensionContext) {
  statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  context.subscriptions.push(statusBarItem);
  statusBarItem.text = '$(circuit-board) PCB Forge: Checking...';
  statusBarItem.show();

  // Initial daemon health check
  checkDaemonHealth(true).then((online) => {
    if (online) {
      // Check KiCad access report
      fetch(`${API_BASE}/status`)
        .then((r) => r.json())
        .then((body: any) => {
          if (body?.data && !body.data.has_access) {
            vscode.window.showWarningMessage(
              `PCB Forge: KiCad application access required (${body.data.message})`,
              'Grant Access'
            ).then((sel) => {
              if (sel === 'Grant Access') {
                checkKiCadAccess();
              }
            });
          }
        })
        .catch(() => {});
    }
  });

  // Periodic health check every 30 seconds
  healthCheckTimer = setInterval(() => {
    checkDaemonHealth(false);
  }, 30000);

  // Register daemon management commands
  context.subscriptions.push(
    vscode.commands.registerCommand('pcb-forge.startDaemon', () => startDaemonService()),
    vscode.commands.registerCommand('pcb-forge.stopDaemon', () => stopDaemonService()),
    vscode.commands.registerCommand('pcb-forge.restartDaemon', () => restartDaemonService()),
    vscode.commands.registerCommand('pcb-forge.checkAccess', () => checkKiCadAccess()),
    vscode.commands.registerCommand('pcb-forge.installDaemon', () => installDaemonService())
  );

  context.subscriptions.push(
    vscode.window.registerWebviewViewProvider('pcbForgeSidebarView', {
      resolveWebviewView(webviewView) {
        vscode.commands.executeCommand('pcb-forge.openDashboard');
        webviewView.webview.html = `
          <!DOCTYPE html>
          <html>
            <body style="padding: 10px; font-family: sans-serif;">
              <p>PCB Forge Dashboard opened in editor tab.</p>
              <button onclick="tsvscode.postMessage({ command: 'open' })">Reopen Dashboard</button>
              <script>
                const tsvscode = acquireVsCodeApi();
              </script>
            </body>
          </html>
        `;
        webviewView.webview.onDidReceiveMessage((msg) => {
          if (msg.command === 'open') {
            vscode.commands.executeCommand('pcb-forge.openDashboard');
          }
        });
      }
    })
  );

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

  registerJsonPathPicker(context);
}

export function deactivate() {
  if (healthCheckTimer) {
    clearInterval(healthCheckTimer);
    healthCheckTimer = undefined;
  }
}

async function checkDaemonHealth(notifyIfDown: boolean = false): Promise<boolean> {
  try {
    const res = await fetch(`${API_BASE}/health`, { signal: AbortSignal.timeout(1500) });
    if (res.ok) {
      isCliAvailable = true;
      updateStatusBar(true);
      return true;
    }
  } catch {}

  isCliAvailable = false;
  updateStatusBar(false);

  if (notifyIfDown) {
    const action = await vscode.window.showWarningMessage(
      'PCB Forge API background daemon is not running.',
      'Start Service',
      'Check KiCad Access',
      'Install Service'
    );
    if (action === 'Start Service') {
      await vscode.commands.executeCommand('pcb-forge.startDaemon');
    } else if (action === 'Check KiCad Access') {
      await vscode.commands.executeCommand('pcb-forge.checkAccess');
    } else if (action === 'Install Service') {
      await vscode.commands.executeCommand('pcb-forge.installDaemon');
    }
  }
  return false;
}

function updateStatusBar(online: boolean) {
  if (!statusBarItem) return;
  if (online) {
    statusBarItem.text = '$(circuit-board) PCB Forge: Online';
    statusBarItem.tooltip = 'PCB Forge background daemon is active on 127.0.0.1:47210. Click to verify KiCad access.';
    statusBarItem.command = 'pcb-forge.checkAccess';
  } else {
    statusBarItem.text = '$(warning) PCB Forge: Offline';
    statusBarItem.tooltip = 'PCB Forge background daemon is stopped. Click to start service.';
    statusBarItem.command = 'pcb-forge.startDaemon';
  }
  statusBarItem.show();
}

async function startDaemonService() {
  await vscode.window.withProgress({
    location: vscode.ProgressLocation.Notification,
    title: 'Starting PCB Forge background service...',
    cancellable: false
  }, async () => {
    try {
      if (process.platform === 'darwin') {
        try {
          await execAsync('pcbfapi service start');
        } catch {
          await execAsync('brew services start pcbfapi');
        }
      } else if (process.platform === 'win32') {
        await execAsync('pcbfapi service start');
      } else {
        await execAsync('pcbfapi service start || systemctl --user start pcbfapi.service');
      }
    } catch (err: any) {
      console.warn('Service start command warning:', err);
    }

    // Poll for readiness
    for (let i = 0; i < 6; i++) {
      await new Promise((r) => setTimeout(r, 800));
      const healthy = await checkDaemonHealth(false);
      if (healthy) {
        vscode.window.showInformationMessage('✓ PCB Forge background service started successfully.');
        return;
      }
    }

    vscode.window.showErrorMessage('Failed to connect to PCB Forge daemon. Run "pcbfapi service start" or "brew services start pcbfapi".');
  });
}

async function stopDaemonService() {
  try {
    await execAsync('pcbfapi service stop || brew services stop pcbfapi');
    await checkDaemonHealth(false);
    vscode.window.showInformationMessage('PCB Forge background service stopped.');
  } catch (err: any) {
    vscode.window.showErrorMessage(`Failed to stop service: ${err.message}`);
  }
}

async function restartDaemonService() {
  await stopDaemonService();
  await new Promise((r) => setTimeout(r, 1000));
  await startDaemonService();
}

async function installDaemonService() {
  try {
    const { stdout } = await execAsync('pcbfapi service install');
    vscode.window.showInformationMessage(`PCB Forge Service Installed: ${stdout.trim()}`);
    await checkDaemonHealth(false);
  } catch (err: any) {
    vscode.window.showErrorMessage(`Service installation failed: ${err.message}`);
  }
}

async function checkKiCadAccess() {
  await vscode.window.withProgress({
    location: vscode.ProgressLocation.Notification,
    title: 'Checking KiCad permissions and access...',
    cancellable: false
  }, async () => {
    try {
      const res = await fetch(`${API_BASE}/check-access`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prompt: true }),
        signal: AbortSignal.timeout(10000)
      });
      if (res.ok) {
        const body = await res.json() as any;
        const report = body.data;
        if (report && report.has_access) {
          vscode.window.showInformationMessage(`✓ KiCad Access OK: ${report.message}`);
        } else {
          vscode.window.showWarningMessage(`KiCad Access Warning: ${report?.message || 'Access issues detected'}`);
        }
        return;
      }
    } catch {}

    // Fall back to CLI execution
    try {
      const { stdout } = await execAsync('pcbfapi check-access');
      vscode.window.showInformationMessage(`KiCad Access:\n${stdout.trim()}`);
    } catch (err: any) {
      vscode.window.showErrorMessage(`KiCad Access Check failed: ${err.message}`);
    }
  });
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
