import * as path from 'path';
import * as vscode from 'vscode';

// Keys that expect a file or directory path
const FILE_KEYS = new Set(['template', 'schematic', 'pcb', 'bom', 'logo', 'input']);
const FOLDER_KEYS = new Set(['outputDir', 'assetsDir']);

interface PickPathArgs {
  uri: vscode.Uri;
  valueRange: vscode.Range; // Includes surrounding quotes
  currentValue: string;
  selectFolder: boolean;
}

export function registerJsonPathPicker(context: vscode.ExtensionContext): void {
  // 1. Command triggered when the user clicks the inline 📂 badge
  const pickCommand = vscode.commands.registerCommand(
    'pcb-forge.pickJsonPath',
    async ({ uri, valueRange, currentValue, selectFolder }: PickPathArgs) => {
      const baseDir = path.dirname(uri.fsPath);
      const initialUri = currentValue
        ? vscode.Uri.file(path.resolve(baseDir, currentValue))
        : vscode.Uri.file(baseDir);

      const selected = await vscode.window.showOpenDialog({
        canSelectFiles: !selectFolder,
        canSelectFolders: selectFolder,
        canSelectMany: false,
        defaultUri: initialUri,
        openLabel: selectFolder ? 'Select Folder' : 'Select File',
      });

      if (!selected || selected.length === 0) {
        return;
      }

      // Keep paths relative to the JSON file and normalized to forward slashes
      let relativePath = path
        .relative(baseDir, selected[0].fsPath)
        .split(path.sep)
        .join('/');

      if (!relativePath.startsWith('.') && relativePath !== '') {
        relativePath = `./${relativePath}`;
      }

      const edit = new vscode.WorkspaceEdit();
      edit.replace(uri, valueRange, JSON.stringify(relativePath));
      await vscode.workspace.applyEdit(edit);
    }
  );

  // 2. Inline hint provider (renders the clickable 📂 icon before the string value)
  const hintProvider = vscode.languages.registerInlayHintsProvider(
    [{ language: 'json' }, { language: 'jsonc' }],
    {
      provideInlayHints(document, range) {
        // Scope only to PCB-Forge config files so package.json/tsconfig.json stay untouched
        if (!isPcbForgeConfig(document)) {
          return [];
        }

        const hints: vscode.InlayHint[] = [];
        const keyValueRegex = /"([^"]+)"\s*:\s*("(?:\\.|[^"\\])*")/g;

        for (let lineNum = range.start.line; lineNum <= range.end.line; lineNum++) {
          const lineText = document.lineAt(lineNum).text;
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

            const part = new vscode.InlayHintLabelPart('📂');
            part.tooltip = isFolder ? 'Browse folder...' : 'Browse file...';
            part.command = {
              title: 'Select Path',
              command: 'pcb-forge.pickJsonPath',
              arguments: [
                {
                  uri: document.uri,
                  valueRange,
                  currentValue,
                  selectFolder: isFolder,
                } satisfies PickPathArgs,
              ],
            };

            const hint = new vscode.InlayHint(valueRange.start, [part]);
            hint.paddingRight = true;
            hints.push(hint);
          }
        }

        return hints;
      },
    }
  );

  context.subscriptions.push(pickCommand, hintProvider);
}

function isPcbForgeConfig(document: vscode.TextDocument): boolean {
  const fileName = path.basename(document.fileName).toLowerCase();
  if (fileName.includes('pcb-forge') || fileName.includes('pcbf')) {
    return true;
  }
  // Fallback: check top of file for a schema or project marker
  const header = document.getText(new vscode.Range(0, 0, Math.min(20, document.lineCount), 0));
  return header.includes('pcb-forge');
}