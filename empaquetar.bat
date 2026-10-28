@echo off
title Isla Dev - Empaquetar
cls
echo ==========================================================
echo        ISLA DEV - EMPAQUETAR INSTALADOR Y PORTABLE
echo ==========================================================
echo.
echo [1/3] Cerrando instancias de Isla Dev...
taskkill /F /IM isla-dev.exe >nul 2>&1
taskkill /F /IM isla-dev-portable.exe >nul 2>&1
ping 127.0.0.1 -n 2 >nul

echo [2/3] Compilando binario final optimizado...
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cargo build --release
if %errorlevel% neq 0 (
    echo [ERROR] La compilacion ha fallado.
    pause
    exit /b %errorlevel%
)

if not exist "%~dp0dist" mkdir "%~dp0dist"
copy /y "%~dp0target\x86_64-pc-windows-gnu\release\isla-dev.exe" "%~dp0dist\isla-dev-portable.exe" >nul
echo [OK] Version portable copiada en dist\isla-dev-portable.exe

echo [3/3] Generando instalador de Windows (Inno Setup)...
set "ISCC=C:\Users\Andres\AppData\Local\Programs\Inno Setup 6\ISCC.exe"
if exist "%ISCC%" (
    "%ISCC%" "%~dp0installer\isla.iss"
    echo.
    echo ==========================================================
    echo   EXITO: Instalador y Portable listos en la carpeta dist\
    echo   - dist\isla-dev-portable.exe (portable sin instalacion)
    echo   - dist\IslaDev-Setup-0.1.0.exe (instalador oficial)
    echo ==========================================================
) else (
    echo [AVISO] Inno Setup no encontrado en %ISCC%.
    echo La version portable esta lista en dist\isla-dev-portable.exe
)
pause
