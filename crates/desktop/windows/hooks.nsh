; Tu Hora Vale used to be installed under the name "zman". It is the same app
; reading the same data, and only one of the two can run, so the old one is
; removed here, silently, before the new one is copied. What it tracked is
; not in its folder and stays where it was.
!macro NSIS_HOOK_PREINSTALL
  Push $R7
  Push $R8
  Push $R9
  ReadRegStr $R8 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\zman" "UninstallString"
  ${If} $R8 != ""
    ; Whether it started with Windows: its uninstaller forgets that.
    ReadRegStr $R7 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "zman"
    ; Where it was installed.
    ReadRegStr $R9 HKCU "Software\zman\zman" ""
    ${If} $R9 != ""
      ; In place (`_?=`), so that this waits until it is done. Run like that,
      ; it cannot delete itself or its folder: that is done right after.
      ExecWait '$R8 /S _?=$R9'
      Delete "$R9\uninstall.exe"
      RMDir "$R9"
    ${Else}
      ExecWait '$R8 /S'
    ${EndIf}
    ${If} $R7 != ""
      ; The same entry the app writes when "start with Windows" is turned on.
      WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}" "$INSTDIR\${MAINBINARYNAME}.exe --hidden"
    ${EndIf}
  ${EndIf}
  Pop $R9
  Pop $R8
  Pop $R7
!macroend
