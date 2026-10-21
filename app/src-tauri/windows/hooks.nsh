; Optional installer. The portable tunelito.exe is the file people open.
; Per-machine, no reboot. Uninstall removes the tunelito account and our sshd block.

!macro NSIS_HOOK_POSTINSTALL
  CreateShortcut "$DESKTOP\Tunelito.lnk" "$INSTDIR\tunelito.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog '"$INSTDIR\tunelito.exe" --uninstall'
!macroend
