; Installer-Hooks für den Tauri-NSIS-Bundler.
; - Nach der Installation optional Autostart einrichten (minimiert ins Tray).
; - Beim Deinstallieren: Einstellungen nur löschen, wenn die Tauri-Checkbox
;   „App-Daten löschen“ gesetzt ist; Clips nur nach ausdrücklicher Nachfrage (Standard: behalten).

!macro NSIS_HOOK_POSTINSTALL
  ${IfNot} ${Silent}
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "0x-clipper beim Windows-Start automatisch starten (minimiert im Tray)?$\n$\nStart 0x-clipper automatically with Windows (minimized to tray)?" IDNO clipper_no_autostart
      WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "0x-clipper" '"$INSTDIR\0x-clipper.exe" --minimized'
    clipper_no_autostart:
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Laufende Instanz beenden, damit Dateien nicht gesperrt sind.
  nsExec::Exec 'taskkill /IM 0x-clipper.exe /F'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "0x-clipper"

  ${If} $DeleteAppDataCheckboxState = 1
    RMDir /r "$APPDATA\0x-clipper"
    RMDir /r "$LOCALAPPDATA\0x-clipper"
  ${EndIf}

  ReadRegStr $R0 HKCU "Software\0x-clipper" "ClipsDir"
  ${If} $R0 != ""
  ${AndIf} ${FileExists} "$R0\*.*"
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "Sollen auch alle gespeicherten Clips gelöscht werden?$\n$R0$\n$\nAlso delete all saved clips?" /SD IDNO IDNO clipper_keep_clips
      RMDir /r "$R0"
    clipper_keep_clips:
  ${EndIf}
  DeleteRegKey HKCU "Software\0x-clipper"
!macroend
