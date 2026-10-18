@echo off
rem Compila o DataBox.exe otimizado (release) e copia para dist\.
rem Precisa do Rust (rustup, toolchain GNU) e do MinGW da WinLibs; ver README.md.
setlocal
set "RAIZ=%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%RAIZ%cargo.ps1" build --release -p editor
if errorlevel 1 (
    echo.
    echo A compilacao falhou; veja as mensagens acima.
    pause
    exit /b 1
)
if not exist "%RAIZ%dist" mkdir "%RAIZ%dist"
copy /y "%RAIZ%target\release\DataBox.exe" "%RAIZ%dist\DataBox.exe" >nul
echo.
echo Pronto: %RAIZ%dist\DataBox.exe
pause
