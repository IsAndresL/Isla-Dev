; Instalador de Isla Dev (Inno Setup 6)
; Compilar:  ISCC installer\isla.iss   (después de cargo build --release)

#define AppName "Isla Dev"
#define AppVersion "0.2.0"
#define AppExe "isla-dev.exe"

[Setup]
AppId={{6B1F2E4C-9A7D-4E1B-8C3A-1D5F7A2B9E01}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Ing. Andres Luna
DefaultDirName={localappdata}\Programs\IslaDev
DefaultGroupName={#AppName}
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=IslaDev-Setup-0.1.0
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
InfoBeforeFile=requisitos.txt
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#AppExe}
CloseApplications=yes

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "startup"; Description: "Iniciar Isla Dev al encender el PC"; GroupDescription: "Opciones:"
Name: "desktopicon"; Description: "Crear acceso directo en el escritorio"; GroupDescription: "Opciones:"; Flags: unchecked
Name: "ghlogin"; Description: "Abrir terminal para iniciar sesión en GitHub CLI (gh auth login)"; GroupDescription: "Configuración de GitHub (requerido para subir repositorios):"; Flags: unchecked

[Files]
Source: "..\target\x86_64-pc-windows-gnu\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "requisitos.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{group}\Requisitos y Guía de Configuración"; Filename: "{app}\requisitos.txt"
Name: "{group}\Desinstalar {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "IslaDev"; ValueData: """{app}\{#AppExe}"""; Tasks: startup; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "IslaDev"; Flags: uninsdeletevalue dontcreatekey

[Run]
Filename: "powershell.exe"; Parameters: "-NoExit -Command ""Write-Host '=== Configuración de GitHub CLI para Isla Dev ===' -ForegroundColor Cyan; if (Get-Command gh -ErrorAction SilentlyContinue) {{ gh auth login }} else {{ Write-Host 'GitHub CLI (gh) no está instalado. Instálalo con: winget install GitHub.cli' -ForegroundColor Yellow }}"""; Description: "Iniciar sesión en GitHub CLI (gh auth login)"; Tasks: ghlogin; Flags: nowait postinstall skipifsilent
Filename: "{app}\{#AppExe}"; Description: "Abrir {#AppName} ahora"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "taskkill"; Parameters: "/F /IM {#AppExe}"; Flags: runhidden; RunOnceId: "KillIsla"
