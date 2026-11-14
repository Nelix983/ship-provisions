# Ship Provisions Tracker (Rust)

Windows desktop app (native, single `.exe`, no Python needed) to track provisions
inventory on board a ship. Same features as the Python version:

- **Import proforma invoice PDFs** (SHIPKO layout; multi-invoice PDFs like
  Provisions/Cabin/Bonded; Dutch number formats `1.200,00`). PDF text is extracted
  with a position-aware extractor (lopdf) that rebuilds table rows by coordinates,
  so columns parse correctly.
- **Inventory checklist** for the crew: export a fillable Excel/CSV, crew fills
  "Counted qty", import the file back as a stocktake.
- **Crew on board log** (persons per date).
- **Insights**: consumption per person, cost per person, per person-day, per
  delivery, per period between stocktakes.
- **Excel export** of the full report (8 sheets) via rust_xlsxwriter.
- Data in a local SQLite DB (`%APPDATA%\ShipProvisions\provisions.db`), fully offline.

## Build the Windows .exe

On any Windows 10/11 PC with Rust installed ([rustup.rs](https://rustup.rs)):

```bat
build_exe.bat
```
→ produces `target\release\ShipProvisions.exe` (single file, double-click to run).

Or manually: `cargo build --release`.

## Get a ready-made .exe from CI

Push this folder into a GitHub repo (e.g. under `ship-provisions-rust/`) — the
workflow `.github/workflows/build-windows-exe-rust.yml` runs `cargo test` and builds
`ShipProvisions.exe` on a `windows-latest` runner on every push:

- repo → **Actions** → latest run → artifact `ShipProvisions-windows-rust`, or
- push a tag `v1.0.0` → the exe is attached to the GitHub **Release**.

## Run from source / tests

```bash
cargo run          # GUI (needs a desktop)
cargo test         # parser tests use sample PDFs when SHIP_PROVISIONS_SAMPLES points to a dir with them
```

## Layout

```
src/lib.rs         library: models, pdf, db, categories, checklist, reports
src/main.rs        egui/eframe GUI (binary "ShipProvisions")
tests/             parser (real sample PDFs), reports, checklist, end-to-end
```

## Linux RPM package

`packaging/build_rpm.py` builds a binary RPM (pure Python, no rpmbuild needed):

```bash
cargo build --release
python3 packaging/build_rpm.py target/release/ShipProvisions dist/ship-provisions-1.0.0-1.x86_64.rpm
sudo rpm -i dist/ship-provisions-1.0.0-1.x86_64.rpm     # or: sudo dnf install ./dist/...
```

The RPM installs `/usr/bin/ShipProvisions` and a desktop entry
(`/usr/share/applications/ship-provisions.desktop`). CI builds it automatically on
every push via `.github/workflows/build-linux-rpm.yml` (artifact `ShipProvisions-linux-rpm`,
attached to Releases on `v*` tags). A prebuilt RPM is also committed under `dist/`.
