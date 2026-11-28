# PCB Forge — Example & Test Workspace

This directory contains a complete, ready-to-run example project (featuring a clone of the **Venturi** PCB hardware files) so you can test **PCB Forge** out of the box.

## Prerequisites

Before running this example, ensure the following are installed on your system:

1. **PCB Forge Daemon (`pcbfapi`)**: Must be installed and running on `127.0.0.1:47210`. See the [root README.md](../README.md) for Homebrew and binary installation instructions.
2. **KiCad (7.0, 8.0, or 9.0)**: `kicad-cli` must be installed and accessible (`pcbfapi check-access`).
3. **VS Code Extension** *(optional, for interactive editing)*: Install the **PCB Forge** extension in VS Code to edit `.pcbf` files with syntax highlighting, inline file pickers, and one-click PDF generation.

## What's Included

- **Sample `.pcbf` Project**: Pre-configured project definition linking schematic, PCB layout, and documentation pages using relative paths.
- **Venturi Hardware Assets**: Reference `.kicad_pcb` and `.kicad_sch` files used to test automated SVG layer exports.
- **Template & Setup Script**: Copies the required document template into `~/.pcb-forge/templates/src/`, generates its JSON schema, and compiles the example PDF via the `pcbfapi` daemon.

## Running the Example

From inside the `test/` directory, run the included setup and generation script:

```bash
chmod +x run-test.sh   # if not already executable
./run-test.sh
```

Once complete, the compiled multi-page PDF is written directly into this folder alongside the .pcbf project file. You can also open the .pcbf file in VS Code and run PCB Forge: Generate Project from the Command Palette (Cmd+Shift+P / Ctrl+Shift+P).

---

### 4. Root `README.md` Updates

Add the **VS Code Extension Installation** section under **Service & Daemon Management**, update the `retag.sh` entry, and expand the **Testing & Examples** section at the bottom:

```markdown
### VS Code Extension Installation

You can install the **PCB Forge** extension in either of two ways:

1. **From the VS Code Marketplace (Recommended):**
   - Open VS Code, go to the **Extensions** view (`Cmd+Shift+X` / `Ctrl+Shift+X`), search for **PCB Forge**, and click **Install**.
   - Or install via CLI:
     ```bash
     code --install-extension David207025.pcb-forge
     ```

2. **From a `.vsix` Release Package (or for VS Code Forks such as Antigravity):**
   - Download `pcb-forge-<version>.vsix` from the [GitHub Releases](../../releases) page (or build it locally with `./build-vsix.sh`).
   - In VS Code / Antigravity, open **Extensions** -> `...` (More Actions) -> **Install from VSIX...** and select the downloaded file, or install from the terminal:
     ```bash
     code --install-extension build/pcb-forge-<version>.vsix --force
     # Or for Antigravity IDE:
     agy-ide --install-extension build/pcb-forge-<version>.vsix --force
     ```