; Inno Setup script for HyprTile
; Compiled by GitHub Actions (.github/workflows/release.yml).
; Locally: install Inno Setup 6, then `iscc installer/hyprtile.iss` from `hyprtile/`.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif

[Setup]
AppId={{C2E3B1F4-9B4D-4F4E-9C7B-2D8B0A1E7C8F}
AppName=HyprTile
AppVersion={#MyAppVersion}
AppPublisher=HyprTile Contributors
AppPublisherURL=https://github.com/ghively/hyprwin
AppSupportURL=https://github.com/ghively/hyprwin/issues
DefaultDirName={autopf}\HyprTile
DefaultGroupName=HyprTile
DisableProgramGroupPage=yes
OutputBaseFilename=HyprTile-{#MyAppVersion}-Setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
WizardStyle=modern
LicenseFile=..\..\LICENSE
SetupIconFile=..\resources\icon.ico
UninstallDisplayIcon={app}\hyprtile.exe
; Compiled output goes alongside the .iss for the workflow to pick up.
OutputDir=.

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked
Name: "startuponlogon"; Description: "&Start HyprTile when I log in"; GroupDescription: "Autostart:"; Flags: unchecked

[Files]
Source: "..\target\release\hyprtile.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\docs\CONFIGURATION.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "..\docs\IPC_PROTOCOL.md"; DestDir: "{app}\docs"; Flags: ignoreversion

[Icons]
Name: "{group}\HyprTile"; Filename: "{app}\hyprtile.exe"
Name: "{group}\HyprTile (verbose)"; Filename: "{app}\hyprtile.exe"; Parameters: "--verbose --foreground"
Name: "{group}\Edit configuration"; Filename: "notepad.exe"; Parameters: "%APPDATA%\hyprtile\hyprtile.toml"
Name: "{group}\Uninstall HyprTile"; Filename: "{uninstallexe}"
Name: "{commondesktop}\HyprTile"; Filename: "{app}\hyprtile.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
    ValueType: string; ValueName: "HyprTile"; ValueData: """{app}\hyprtile.exe"""; \
    Flags: uninsdeletevalue; Tasks: startuponlogon

[Run]
Filename: "{app}\hyprtile.exe"; Description: "Launch HyprTile now"; \
    Flags: nowait postinstall skipifsilent
