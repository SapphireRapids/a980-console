!include "MUI2.nsh"
!include "FileFunc.nsh"

Name "A980 Console"
OutFile "a980-release\A980 Console Setup 1.0.0.exe"
InstallDir "$LOCALAPPDATA\A980 Console"
RequestExecutionLevel user

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  SetOutPath "$INSTDIR"
  File "target\release\a980-console.exe"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  
  CreateShortCut "$SMPROGRAMS\A980 Console.lnk" "$INSTDIR\a980-console.exe"
  CreateShortCut "$DESKTOP\A980 Console.lnk" "$INSTDIR\a980-console.exe"
  
  WriteRegStr HKCU "Software\A980 Console" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\A980 Console" "DisplayName" "A980 Console"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\A980 Console" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\A980 Console" "DisplayVersion" "1.0.0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\A980 Console" "Publisher" "SapphireRapids"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\a980-console.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  
  Delete "$SMPROGRAMS\A980 Console.lnk"
  Delete "$DESKTOP\A980 Console.lnk"
  
  DeleteRegKey HKCU "Software\A980 Console"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\A980 Console"
SectionEnd
