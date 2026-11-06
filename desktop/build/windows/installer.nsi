Unicode true
!include "MUI2.nsh"
Name "Meron"
OutFile "..\bin\meron-${ARCH}-installer.exe"
; Preserve the Wails v2 installer identity so upgrades replace existing installs.
InstallDir "$PROGRAMFILES64\meron\Meron"
RequestExecutionLevel admin
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Meron"
VIAddVersionKey "FileDescription" "Meron Installer"
VIAddVersionKey "FileVersion" "${VERSION}"
!define MUI_ICON "icon.ico"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Section
  SetShellVarContext all
  SetRegView 64
  SetOutPath $INSTDIR
  File "..\bin\meron.exe"
  File "MicrosoftEdgeWebview2Setup.exe"
  ExecWait '"$INSTDIR\MicrosoftEdgeWebview2Setup.exe" /silent /install'
  Delete "$INSTDIR\MicrosoftEdgeWebview2Setup.exe"
  CreateShortcut "$SMPROGRAMS\Meron.lnk" "$INSTDIR\meron.exe"
  WriteRegStr HKLM "Software\Classes\mailto" "" "URL:MailTo Protocol"
  WriteRegStr HKLM "Software\Classes\mailto" "URL Protocol" ""
  WriteRegStr HKLM "Software\Classes\mailto\shell\open\command" "" '$\"$INSTDIR\meron.exe$\" $\"%1$\"'
  WriteRegStr HKLM "Software\Classes\Meron.EML" "" "Email message"
  WriteRegStr HKLM "Software\Classes\Meron.EML\shell\open\command" "" '$\"$INSTDIR\meron.exe$\" $\"%1$\"'
  WriteRegStr HKLM "Software\Classes\.eml\OpenWithProgids" "Meron.EML" ""
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\meronMeron" "DisplayName" "Meron"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\meronMeron" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\meronMeron" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
SectionEnd
Section "Uninstall"
  SetShellVarContext all
  SetRegView 64
  ReadRegStr $0 HKLM "Software\Classes\mailto\shell\open\command" ""
  StrCmp $0 '$\"$INSTDIR\meron.exe$\" $\"%1$\"' 0 protocol_cleanup_done
  DeleteRegKey HKLM "Software\Classes\mailto"
protocol_cleanup_done:
  DeleteRegValue HKLM "Software\Classes\.eml\OpenWithProgids" "Meron.EML"
  DeleteRegKey HKLM "Software\Classes\Meron.EML"
  Delete "$SMPROGRAMS\Meron.lnk"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\meronMeron"
  Delete "$INSTDIR\meron.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
