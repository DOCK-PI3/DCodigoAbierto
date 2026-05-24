@echo off
setlocal EnableExtensions

rem DCA-IA-IMPROVEMENT: Launcher Windows con instalacion de Rust, build condicional y ejecucion.
cd /d "%~dp0"

set "CARGO_BIN=%USERPROFILE%\.cargo\bin"
set "PATH=%CARGO_BIN%;%PATH%"
set "BINARY=target\release\dca.exe"

call :ensure_cargo || exit /b 1

if not exist "%BINARY%" (
    echo [DCA] Binario no encontrado. Compilando en release...
    cargo build --release --bin dca || exit /b 1
)

if "%DCA_SKIP_RUN%"=="1" (
    echo [DCA] Validacion completada. Ejecucion omitida por DCA_SKIP_RUN=1.
    exit /b 0
)

echo [DCA] Ejecutando %BINARY%
"%BINARY%" %*
exit /b %ERRORLEVEL%

:ensure_cargo
where cargo >nul 2>&1 && exit /b 0

if exist "%CARGO_BIN%\cargo.exe" (
    exit /b 0
)

echo [DCA] Rust no esta instalado. Instalando...

where winget >nul 2>&1
if %ERRORLEVEL%==0 (
    winget install --id Rustlang.Rustup --source winget --accept-package-agreements --accept-source-agreements --disable-interactivity || exit /b 1
) else (
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-WebRequest -UseBasicParsing https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile rustup-init.exe" || exit /b 1
    rustup-init.exe -y --profile minimal || exit /b 1
    del /q rustup-init.exe >nul 2>&1
)

set "PATH=%CARGO_BIN%;%PATH%"
where cargo >nul 2>&1 || exit /b 1
exit /b 0