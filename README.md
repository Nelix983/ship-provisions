# Ship Provisions Tracker

A **Windows desktop application** (not a browser app) to track provisions inventory
on board a ship:

- **Import proforma invoice PDFs** (SHIPKO-style, but any supplier PDF with
  `ref | description | qty | unit | price | total` lines works). One PDF can contain
  several invoices (Provisions, Cabin, Bonded, ...); you pick which to import.
- **Inventory checklist for the crew**: export a fillable Excel/CSV checklist, the
  crew fills in the counted quantities on board, you import the file back as a
  stocktake.
- **Crew on board log**: enter how many people were on board per date.
- **Insights**: consumption per person and cost per person (per period between
  stocktakes, per person-day, and per delivery).
- **Export everything to Excel** (one workbook: Summary, Consumption per person,
  Deliveries, Delivery lines, Stocktakes, Stocktake lines, Crew, Current inventory).

Data is stored locally in a SQLite database
(`%APPDATA%\ShipProvisions\provisions.db` on Windows), so the program works fully
offline.

## Install on a Windows computer

You need the built executable `ShipProvisions.exe`. Two ways to get it:

1. **Ask the developer for `dist\ShipProvisions.exe`** — copy it to the ship's
   computer and double-click. No installation, no admin rights needed.
2. **Build it yourself** on a Windows 10/11 PC with Python 3.10+ installed:
   double-click `build_exe.bat` (or run `pyinstaller ship_provisions.spec
   --noconfirm --clean`). The result is `dist\ShipProvisions.exe`.

Optionally, compile `installer.iss` with [Inno Setup 6](https://jrsoftware.org/isinfo.php)
to produce a classic installer `ShipProvisions_Setup.exe`.

### Get a ready-made .exe without building yourself

Push this folder to a GitHub repository and the included workflow
(`.github/workflows/build-windows-exe.yml`) builds `ShipProvisions.exe` on a
Windows runner automatically:

- repo → **Actions** → latest "Build Windows exe" run → download the
  `ShipProvisions-windows` artifact, or
- push a version tag (`git tag v1.0.0 && git push origin v1.0.0`) → the exe is
  attached to a GitHub **Release** for direct download.

## Using the program

1. **Deliveries tab** → *Import PDF invoice(s)...* → select one or more supplier
   PDFs → review the parsed invoices (toggle *Import?*) → *Import selected*.
   Re-importing the same PDF does not create duplicates.
2. **Crew on board tab** → add date + number of persons for each change of crew.
3. **Inventory tab** → *Export checklist (Excel)...* → give the file to the crew
   → crew fills the *Counted qty* column → *Import filled checklist...* (choose the
   filled file, stocktake date and name) → recorded as a stocktake.
4. **Insights tab** → choose *All time* or a *stocktake → stocktake* period →
   *Compute insights* to see consumption and cost per person → *Export Excel
   report...* for a full workbook.

## Notes

- PDFs must have a text layer (normal e-mailed invoices do). Scanned image-only
  PDFs need OCR: install Tesseract OCR and `pip install pytesseract` when building.
- Numbers in Dutch format (`1.200,00`) are understood.
- Run tests with: `python -m pytest tests/` (needs `pip install pytest`).

## Development

```
ship-provisions/
├── src/ship_provisions/   # application code (parser, db, checklist, reports, GUI)
├── tests/                 # pytest suite (uses the sample PDFs when present)
├── requirements.txt
├── ship_provisions.spec   # PyInstaller spec for the Windows .exe
├── build_exe.bat          # one-click Windows build
└── installer.iss          # optional Inno Setup installer script
```

Run from source (any OS with Python 3.10+):

```
pip install -r requirements.txt
PYTHONPATH=src python -m ship_provisions
```
