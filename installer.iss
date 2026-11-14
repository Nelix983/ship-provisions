; Optional Inno Setup script to create a proper Windows installer (.exe setup file).
; Requires Inno Setup 6 (https://jrsoftware.org/isinfo.php).
; First build the app with build_exe.bat, then compile this script.

#define MyAppName "Ship Provisions Tracker"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "Ship office"
#define MyAppExeName "ShipProvisions.exe"

[Setup]
AppId={{8F3E2A1B-9C4D-4E5A-A6B7-1D2C3E4F5A6B}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=dist
OutputBaseFilename=ShipProvisions_Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"

[Files]
Source: "dist\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent
