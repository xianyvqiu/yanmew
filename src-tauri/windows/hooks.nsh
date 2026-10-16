; 当前用户安装；路径固定，不接受插件、备份或注册表提供的删除目标。
!macro YanMewWaitForExit
  !define YanMewCheckId ${__LINE__}
  yanmew_wait_${YanMewCheckId}:
    nsis_tauri_utils::FindProcessCurrentUser "${MAINBINARYNAME}.exe"
    Pop $R0
    ${If} $R0 = 0
      IfSilent yanmew_cancel_${YanMewCheckId} 0
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "颜喵正在运行。请先通过托盘或软件设置正常退出，再点击重试。" IDRETRY yanmew_wait_${YanMewCheckId}
      yanmew_cancel_${YanMewCheckId}:
      SetErrorLevel 1
      Quit
    ${EndIf}
  !undef YanMewCheckId
!macroend

; 调用处只传入固定的当前用户应用目录，不存在时视为已经清理。
!macro YanMewRemoveDataDirectory directory
  System::Call 'kernel32::GetFileAttributesW(w "${directory}") i.r0'
  ${If} $0 != -1
    IntOp $R1 $0 & 0x400
    ${If} $R1 <> 0
      MessageBox MB_OK|MB_ICONSTOP "应用数据目录是重解析点，停止清理，请检查目录。"
      SetErrorLevel 2
      Abort
    ${EndIf}
    ClearErrors
    RMDir /r "${directory}"
    ${If} ${Errors}
      MessageBox MB_OK|MB_ICONSTOP "应用数据未能完整删除。请检查文件占用及目录权限后再次卸载。"
      SetErrorLevel 2
      Abort
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro MUI_STARTMENU_GETFOLDER Application $AppStartMenuFolder
  CreateShortcut "$SMPROGRAMS\$AppStartMenuFolder\卸载颜喵.lnk" "$INSTDIR\uninstall.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; 清理在程序删除前执行；失败保留可再次卸载的程序和登记。
  !insertmacro YanMewWaitForExit
  ${If} $UpdateMode <> 1
    IfFileExists "$INSTDIR\yanmew-installed" +3 0
      MessageBox MB_OK|MB_ICONSTOP "安装标记缺失，无法确认数据清理范围。请修复安装后重试。"
      Abort
    ; 防止把数据目录的重解析点作为递归删除目标。
    System::Call 'kernel32::GetFileAttributesW(w "$LOCALAPPDATA\YanMew") i.r0'
    IntOp $R1 $0 & 0x400
    ${If} $0 != -1
    ${AndIf} $R1 <> 0
      MessageBox MB_OK|MB_ICONSTOP "颜喵数据目录是重解析点，停止清理，请检查目录。"
      Abort
    ${EndIf}
    !insertmacro YanMewRemoveDataDirectory "$LOCALAPPDATA\YanMew\data"
    !insertmacro YanMewRemoveDataDirectory "$APPDATA\${BUNDLEID}"
    !insertmacro YanMewRemoveDataDirectory "$LOCALAPPDATA\${BUNDLEID}"
    RMDir "$LOCALAPPDATA\YanMew"

    ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "YanMew"
    ${If} $R0 == '$\"$INSTDIR\${MAINBINARYNAME}.exe$\"'
    ${OrIf} $R0 == "$INSTDIR\${MAINBINARYNAME}.exe"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "YanMew"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "YanMew"
    ${EndIf}
    !insertmacro MUI_STARTMENU_GETFOLDER Application $AppStartMenuFolder
    Delete "$SMPROGRAMS\$AppStartMenuFolder\卸载颜喵.lnk"
    ClearErrors
  ${EndIf}
!macroend
