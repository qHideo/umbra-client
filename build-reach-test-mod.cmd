@echo off
setlocal
pushd "%~dp0" || exit /b 1

set "VSDEVCMD=%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
if not exist "%VSDEVCMD%" (
    echo Visual Studio 2022 Build Tools were not found at:
    echo   "%VSDEVCMD%"
    echo Install MSVC x64/x86 build tools and the Windows SDK, then try again.
    popd
    exit /b 1
)

call "%VSDEVCMD%" -arch=amd64 -host_arch=amd64
if errorlevel 1 (
    echo Failed to initialize the Visual Studio build environment.
    popd
    exit /b 1
)

set "CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
if not exist "%CARGO%" set "CARGO=cargo.exe"
set "MOD_DIR=%~dp0.local\mods"
if not exist "%MOD_DIR%" mkdir "%MOD_DIR%"

"%CARGO%" build -p reach-test-mod --target wasm32-unknown-unknown --locked
if errorlevel 1 (
    popd
    exit /b 1
)

"%CARGO%" run -p mod-host --bin mod-host --locked -- pack "%~dp0target\wasm32-unknown-unknown\debug\reach_test_mod.wasm" "%MOD_DIR%\reach-test.component.wasm"
if errorlevel 1 (
    popd
    exit /b 1
)

powershell -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference='Stop'; $dir='%MOD_DIR%'; $component=(Join-Path $dir 'reach-test.component.wasm'); $set=@{version=1;mods=@(@{component=$component;grants=@{interaction=$true}})} | ConvertTo-Json -Depth 5 -Compress; [IO.File]::WriteAllText((Join-Path $dir 'reach-test-set.json'),$set,[Text.UTF8Encoding]::new($false)); [IO.File]::WriteAllText((Join-Path $dir 'reach-test-server.txt'),'fallentech.tk:19134',[Text.UTF8Encoding]::new($false))"
if errorlevel 1 (
    echo Failed to write the local reach-test configuration.
    popd
    exit /b 1
)

echo Built the reach test mod and configured the local server opt-in.
popd
exit /b 0
