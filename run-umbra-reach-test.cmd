@echo off
setlocal
pushd "%~dp0" || exit /b 1

if not exist "%~dp0.local\mods\reach-test-set.json" (
    echo Reach-test mod is not built yet. Run build-reach-test-mod.cmd first.
    popd
    exit /b 1
)

set "CINNABAR_MOD_SET=%~dp0.local\mods\reach-test-set.json"
for /f "usebackq delims=" %%S in ("%~dp0.local\mods\reach-test-server.txt") do set "UMBRA_REACH_TEST_SERVER=%%S"
if not defined UMBRA_REACH_TEST_SERVER (
    echo The local test server allowlist is missing or empty.
    popd
    exit /b 1
)

call "%~dp0run-umbra.cmd" %*
set "RESULT=%ERRORLEVEL%"
popd
exit /b %RESULT%
