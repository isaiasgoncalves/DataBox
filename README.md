# DataBox

**English** · [Português](README.pt-BR.md)

**DataBox** is a free, open-source data toolbox for developers, data scientists and data engineers. It is a Windows
desktop app that opens, shows and edits everyday data files: **SQLite** databases, whole folders with their data
files, **CSV**, **JSON** and text files, and **Excel workbooks** (read-only). It ships as a single executable,
`DataBox.exe`: no installer, no runtime, no browser and no server. The UI is drawn straight on the GPU with
[egui](https://github.com/emilk/egui), in Rust.

It started as a SQLite viewer and editor and is growing into a toolbox with many tools for people who work with
data. Current version: **0.1.1**.

> The interface, the code and the in-depth docs are in **Brazilian Portuguese** for now. English UI and docs are on
> the roadmap; contributions are welcome.

| Type | Extensions | How it opens |
|---|---|---|
| SQLite database | `.db`, `.sqlite` (and any file that is SQLite inside) | full editing, or read-only on request |
| Folder | any folder | the Explorer lists its databases and data files |
| CSV | `.csv`, `.tsv` | table editor |
| JSON and text | `.json`, `.txt`, `.md`, `.log` | text editor (read-only above 0.5 MiB) |
| Excel workbook | `.xlsx`, `.xlsm` | read-only |

---

## Download

Grab `DataBox.exe` from the latest [release](../../releases/latest) and double-click it. Nothing to install. Requires
Windows 10 or 11, 64-bit.

> The executable is not code-signed, so Windows SmartScreen may warn you the first time ("Windows protected your
> PC"). Click **More info › Run anyway**, or build it yourself (below).

---

## Features

**Everywhere**
- Document tabs with a single **Back/Forward** history across tabs (Alt+←/→ or the mouse side buttons).
- Custom window: title bar with menus, one toolbar for every tab, Explorer and query History on the left,
  Properties on the right, status bar at the bottom; resizable side panels.
- Frosted glass on menus, lists and dialogs; Light, Dark or Follow Windows theme.
- Preferences (Ctrl+,): theme, code colors and font, export options; changes preview live and persist on **Save**.
- Drag and drop a folder, database or data file onto the window to open it.
- Asks before closing a tab, folder or the app with unsaved changes.

**Folders**
- File › Open folder (Ctrl+Shift+O). The Explorer shows subfolders, databases (detected by content, not by
  extension), CSV, workbooks, JSON and text files; hidden files, Office lock files and tool folders (`.git`,
  `node_modules`…) are skipped. Right-click for Open, Refresh, Show in folder, Copy path, Close folder.

**SQLite**
- Database overview with counters of tables, views, indexes and triggers.
- **Data** grid with search, filters, sorting, in-place editing, marked rows, a row form (BLOB, NULL, generated
  columns), New row, Duplicate and Delete.
- **Structure**, **Indexes and triggers** and **SQL definition** for each table; New table, New index and Alter
  column, always showing the SQL that will run before it runs.
- **SQL Query**: as many queries as you like at once, syntax colors, query History and **Stop** (Esc).
- **Export** to Excel, CSV or JSON (a table, only what search and filters show, marked rows, a query result or the
  whole database), with progress. 1 million rows to Excel in about 7 s.
- Open databases **read-only** to look without risk.

**CSV**
- Opens files with millions of rows instantly (paged, never fully loaded), with the detected type of each column.
- Edit cells, add/duplicate/delete rows and columns, undo/redo, filter with conditions and sort by column.
- Copy and paste columns, paste matching by key (Ctrl+Shift+V), and **Bring columns** from another file (CSV,
  Excel sheet, clipboard) by a common key, with preview and a single undo step.
- **Save** keeps the file's encoding, delimiter and line endings and does not touch unchanged lines.

**JSON and text**
- Line numbers; for JSON, syntax colors, valid/invalid indicator with line, column and reason, **Format** (keeps key
  order and numbers) and a document outline.
- Editable up to 0.5 MiB; larger files open read-only up to 32 MiB. Encoding and line endings round-trip.

**Excel**
- Read-only, with sheet tabs (including hidden sheets on request), column widths, merged cells and frozen panes.
- Large sheets show the first page immediately and keep indexing in the background.
- Copy a cell or a column, or bring a sheet's columns into the open CSV.

---

## Command line

```text
DataBox.exe [path] [--somente-leitura] [--renderizador glow|wgpu] [--criar-atalho] [-h | --ajuda] [--versao]
```

- `DataBox.exe C:\data` opens the folder; add `--somente-leitura` to open a database read-only.
- `--criar-atalho` creates Desktop and Start menu shortcuts and exits.
- Settings live in `%APPDATA%\DataBox\preferencias.json`; the log is `%LOCALAPPDATA%\DataBox\databox.log`.

---

## Building from source

**Tools** (once, no admin rights needed):

- **Rust** stable via [rustup](https://rustup.rs), with the **GNU** toolchain (no Visual Studio required; the MSVC
  toolchain is not supported yet):

  ```powershell
  rustup-init.exe --default-host x86_64-pc-windows-gnu --profile minimal -c clippy -c rustfmt
  ```

- **MinGW-w64 from [WinLibs](https://winlibs.com)** (GCC 16, MSVCRT, without LLVM), extracted to
  `%LOCALAPPDATA%\Programs\winlibs-gcc16` (the `mingw64` folder goes inside it), or anywhere else with the
  `DATABOX_MINGW` environment variable pointing to its `bin` folder.

**Always build through `cargo.ps1`**, a drop-in for `cargo` with the same arguments (quote `--` as `'--'`, or
PowerShell swallows it). It puts Rust and MinGW on the
PATH and wires a small linker shim ([`ferramentas/vincular.rs`](ferramentas/vincular.rs)) that rewrites accented paths
to their 8.3 short names; without it, MinGW's `ld` fails in user folders with accents.

```powershell
.\cargo.ps1 run -p amostras '--' exemplo.db       # create a sample database
.\cargo.ps1 run -p editor '--' exemplo.db         # open DataBox with it
.\cargo.ps1 build --release -p editor           # optimized build in target\release\DataBox.exe
.\cargo.ps1 test --workspace                    # all tests
.\cargo.ps1 clippy --workspace --all-targets '--' -D warnings
.\cargo.ps1 fmt --all '--' --check
```

Or double-click [`compilar.bat`](compilar.bat), which builds the release binary and copies it to `dist\DataBox.exe`.
Keep the project in a shallow folder: paths over 260 characters break the Windows build.

**Linux (experimental):** plain `cargo`, no `cargo.ps1`. On Debian/Ubuntu, install the build packages once:

```bash
sudo apt-get install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev
cargo build --release -p editor        # target/release/DataBox
./target/release/DataBox --criar-atalho  # adds DataBox to the applications menu (~/.local/share/applications)
```

At run time it uses the desktop's file dialogs (`xdg-desktop-portal`), `xdg-open` (package `xdg-utils`) and the Noto
Sans or DejaVu fonts. The Linux build is checked by CI but has not been tried on many desktops yet: reports are
welcome.

More: [`README.pt-BR.md`](README.pt-BR.md) (full guide), [`ARQUITETURA.md`](ARQUITETURA.md) (architecture and crate
contracts), [`design/README.md`](design/README.md) (visual guide), [`RELEASE.md`](RELEASE.md) (how to publish a
release).

---

## Roadmap

DataBox aims to be a data toolbox, not only a SQLite editor. Ideas in the queue (suggestions welcome in
[issues](../../issues)): DuckDB, PostgreSQL, MySQL/MariaDB and MongoDB; charts; Parquet; search inside CSV and text;
Save as with format conversion; `.xls`/`.xlsb`; English UI; Linux and macOS builds.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

[MIT](LICENSE). The UI uses the fonts that ship with Windows (Segoe UI and Consolas) and DataBox's own logo. Direct
dependencies are MIT and/or Apache-2.0; transitive ones include other permissive licenses (Unicode-3.0, BSL-1.0, Zlib,
ISC, MPL-2.0), and the executable embeds egui's default fallback fonts (SIL OFL 1.1 and Ubuntu Font Licence). The list
of libraries with each license is inside the app (Help › About › Third-party licenses) and, with the license texts, in
the `THIRD-PARTY-NOTICES.txt` of each release `.zip`.
