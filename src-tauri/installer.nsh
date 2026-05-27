; Tauri NSIS installer hooks — v0.22.4.
;
; Two explicit goals:
;   1. Guarantee Start Menu + Desktop shortcuts on every install
;      (silent OR GUI). Tauri's default template usually creates these,
;      but documenting them as explicit hooks makes the behavior
;      auditable and bypasses any Components-page toggle quirks.
;   2. Drop a README.txt explaining the taskbar-pin limitation. Windows
;      10 1809+ blocks programmatic taskbar pinning (anti-spam UX
;      measure by Microsoft); no installer can flip that bit. The
;      user-facing workaround is two clicks: right-click the Start
;      Menu Keepr entry → "Pin to taskbar".
;
; Both hooks are idempotent — re-running the installer just overwrites
; the existing shortcut and README.

!macro NSIS_HOOK_POSTINSTALL
  ; Start Menu shortcut at the top level of Programs (not in a folder
  ; — matches Tauri's default placement and what most user-mode
  ; installers do for single-app products).
  CreateShortCut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0

  ; Desktop shortcut.
  CreateShortCut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\${MAINBINARYNAME}.exe" 0

  ; Drop a README.txt explaining the taskbar-pin limitation + how to
  ; do it manually. ASCII only (NSIS FileWrite is byte-oriented; we
  ; keep it locale-independent).
  FileOpen $0 "$INSTDIR\README.txt" w
  FileWrite $0 "Keepr is installed.$\r$\n$\r$\n"
  FileWrite $0 "Shortcuts created:$\r$\n"
  FileWrite $0 "  Start Menu : Start -> Keepr$\r$\n"
  FileWrite $0 "  Desktop    : Desktop\Keepr.lnk$\r$\n$\r$\n"
  FileWrite $0 "Pin to taskbar (manual, two clicks):$\r$\n"
  FileWrite $0 "  1. Open Start, find Keepr.$\r$\n"
  FileWrite $0 "  2. Right-click -> Pin to taskbar.$\r$\n$\r$\n"
  FileWrite $0 "Windows 10 1809+ and Windows 11 disable programmatic$\r$\n"
  FileWrite $0 "taskbar pinning by design (anti-spam UX measure), so$\r$\n"
  FileWrite $0 "this last step is unavoidably manual. See$\r$\n"
  FileWrite $0 "https://github.com/SysAdminDoc/Keepr for context.$\r$\n$\r$\n"
  FileWrite $0 "To uninstall: Settings -> Apps, or run uninstall.exe$\r$\n"
  FileWrite $0 "in this folder.$\r$\n$\r$\n"
  FileWrite $0 "Your notes are stored in:$\r$\n"
  FileWrite $0 "  %APPDATA%\com.sysadmindoc.keepr\keepr.db$\r$\n"
  FileWrite $0 "Uninstalling Keepr DOES NOT delete that database.$\r$\n"
  FileWrite $0 "Remove that folder manually if you want a clean wipe.$\r$\n"
  FileClose $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  Delete "$DESKTOP\${PRODUCTNAME}.lnk"
  Delete "$INSTDIR\README.txt"
  ; v0.22.10 — also clean the portable-mode sentinel if a user dropped
  ; it next to the EXE. Leaving it behind would silently flip a future
  ; reinstall into portable mode (data dir = $INSTDIR) which is rarely
  ; what someone wants from a per-user installer.
  Delete "$INSTDIR\portable.flag"
!macroend
