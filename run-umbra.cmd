@echo off
setlocal
pushd "%~dp0" || exit /b 1

set "VSDEVCMD=%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
if not exist "%VSDEVCMD%" (
    echo Visual Studio 2022 Build Tools were not found at:
    echo   "%VSDEVCMD%"
    echo Install the MSVC x64/x86 build tools and Windows SDK, then try again.
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

rem Free community builds do not require the optional local license service.
set "UMBRA_RANK_API_URL=http://127.0.0.1:8788/"
set "UMBRA_DISCORD_LARGE_IMAGE_KEY=umbra_logo"
"%CARGO%" run -p cinnabar --bin umbra --locked -- %*
set "RESULT=%ERRORLEVEL%"
popd
exit /b %RESULT%
