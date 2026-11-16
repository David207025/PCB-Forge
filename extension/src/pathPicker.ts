import * as path from 'path';
import * as vscode from 'vscode';

// Keys that expect a file or directory path
const FILE_KEYS = new Set(['path', 'template', 'schematic', 'pcb', 'bom', 'logo', 'input']);
const FOLDER_KEYS = new Set(['outputDir', 'assetsDir']);

const PCBF_SELECTOR: vscode.DocumentSelector = [{ language: 'pcbf' }];

interface PickPathArgs {
  uri: vscode.Uri;
  valueRange: vscode.Range; // Includes surrounding quotes
  currentValue: string;
  selectFolder: boolean;
}

interface MatchedPathField {
  valueRange: vscode.Range;
  currentValue: string;
  selectFolder: boolean;
}

export function registerJsonPathPicker(context: vscode.ExtensionContext): void {
  // 1. Command triggered by either the inline 📂 badge or the Quick Fix menu
  const pickCommand = vscode.commands.registerCommand(
    'pcb-forge.pickJsonPath',
    async ({ uri, valueRange, currentValue, selectFolder }: PickPathArgs) => {
      const baseDir = path.dirname(uri.fsPath);

      // Preserve optional ";layer1,layer2" suffix used by .kicad_pcb entries
      const semiIdx = currentValue.indexOf(';');
      const rawPathPart = semiIdx >= 0 ? currentValue.slice(0, semiIdx).trim() : currentValue.trim();
      const layerSuffix = semiIdx >= 0 ? currentValue.slice(semiIdx) : '';

      const initialUri = rawPathPart
        ? vscode.Uri.file(path.resolve(baseDir, rawPathPart))
        : vscode.Uri.file(baseDir);

      const selected = await vscode.window.showOpenDialog({
        canSelectFiles: !selectFolder,
        canSelectFolders: selectFolder,
        canSelectMany: false,
        defaultUri: initialUri,
        openLabel: selectFolder ? 'Select Folder' : 'Select File',
        filters: selectFolder
          ? undefined
          : {
              'PCB Forge Assets': ['kicad_sch', 'kicad_pcb', 'md', 'csv', 'svg', 'png', 'jpg', 'jpeg'],
              'All Files': ['*'],
            },
      });

      if (!selected || selected.length === 0) {
        return;
      }

      // Keep paths relative to the .pcbf file and normalized to forward slashes
      let relativePath = path
        .relative(baseDir, selected[0].fsPath)
        .split(path.sep)
        .join('/');

      if (!relativePath.startsWith('.') && relativePath !== '') {
        relativePath = `./${relativePath}`;
      }

      if (!selectFolder && relativePath.toLowerCase().endsWith('.kicad_pcb') && layerSuffix) {
        relativePath += layerSuffix;
      }

      const edit = new vscode.WorkspaceEdit();
      edit.replace(uri, valueRange, JSON.stringify(relativePath));
      await vscode.workspace.applyEdit(edit);
    }
  );

  // 2. Inline hint provider (renders the clickable 📂 icon before the string value)
  const hintProvider = vscode.languages.registerInlayHintsProvider(
    PCBF_SELECTOR,
    {
      provideInlayHints(document, range) {
        if (!isPcbForgeConfig(document)) {
          return [];
        }

        const hints: vscode.InlayHint[] = [];
        for (let lineNum = range.start.line; lineNum <= range.end.line; lineNum++) {
          for (const field of findPathFieldsOnLine(document, lineNum)) {
            const part = new vscode.InlayHintLabelPart('📂');
            part.tooltip = field.selectFolder ? 'Browse folder...' : 'Browse file...';
            part.command = {
              title: 'Select Path',
              command: 'pcb-forge.pickJsonPath',
              arguments: [{ uri: document.uri, ...field } satisfies PickPathArgs],
            };

            const hint = new vscode.InlayHint(field.valueRange.start, [part]);
            hint.paddingRight = true;
            hints.push(hint);
          }
        }
        return hints;
      },
    }
  );

  // 3. Code action provider (shows "Browse file..." lightbulb / Cmd+. when cursor is on the line)
  const codeActionProvider = vscode.languages.registerCodeActionsProvider(
    PCBF_SELECTOR,
    {
      provideCodeActions(document, range) {
        if (!isPcbForgeConfig(document)) {
          return [];
        }

        const actions: vscode.CodeAction[] = [];
        for (const field of findPathFieldsOnLine(document, range.start.line)) {
          const title = field.selectFolder ? 'Browse folder...' : 'Browse file...';
          const action = new vscode.CodeAction(title, vscode.CodeActionKind.QuickFix);
          action.isPreferred = true;
          action.command = {
            title,
            command: 'pcb-forge.pickJsonPath',
            arguments: [{ uri: document.uri, ...field } satisfies PickPathArgs],
          };
          actions.push(action);
        }
        return actions;
      },
    },
    { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] }
  );

  context.subscriptions.push(pickCommand, hintProvider, codeActionProvider);
}

function findPathFieldsOnLine(document: vscode.TextDocument, lineNum: number): MatchedPathField[] {
  const results: MatchedPathField[] = [];
  const lineText = document.lineAt(lineNum).text;
  const keyValueRegex = /"([^"]+)"\s*:\s*("(?:\\.|[^"\\])*")/g;
  let match: RegExpExecArray | null;

  while ((match = keyValueRegex.exec(lineText)) !== null) {
    const key = match[1];
    const rawStringLiteral = match[2];
    const isFile = FILE_KEYS.has(key);
    const isFolder = FOLDER_KEYS.has(key);

    if (!isFile && !isFolder) {
      continue;
    }

    const valueStartCol = match.index + match[0].lastIndexOf(rawStringLiteral);
    const valueEndCol = valueStartCol + rawStringLiteral.length;
    const valueRange = new vscode.Range(lineNum, valueStartCol, lineNum, valueEndCol);

    let currentValue = '';
    try {
      currentValue = JSON.parse(rawStringLiteral);
    } catch {
      // Ignore malformed intermediate string while typing
    }

    results.push({ valueRange, currentValue, selectFolder: isFolder });
  }

  return results;
}

function isPcbForgeConfig(document: vscode.TextDocument): boolean {
  return path.extname(document.fileName).toLowerCase() === '.pcbf';
}