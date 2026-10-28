@echo off
title Isla Dev - Previsualizar
cls
echo ==========================================================
echo        ISLA DEV - MODO PREVISUALIZACION EN VIVO
echo ==========================================================
echo.
echo [1/3] Cerrando instancias anteriores de Isla Dev...
taskkill /F /IM isla-dev.exe >nul 2>&1
taskkill /F /IM isla-dev-portable.exe >nul 2>&1
ping 127.0.0.1 -n 2 >nul

echo [2/3] Compilando cambios en Rust...
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
cargo build --release
if %errorlevel% neq 0 (
    echo.
    echo [ERROR] La compilacion ha fallado. Revisa los errores arriba.
    pause
    exit /b %errorlevel%
)

echo.
echo [3/3] Lanzando Isla Dev en modo portable directo...
start "" "%~dp0target\x86_64-pc-windows-gnu\release\isla-dev.exe"

echo.
echo ==========================================================
echo   EXITO: La gota / notch ya esta visible en tu pantalla!
echo   Ubicacion: Centro superior (pegado exactamente arriba).
echo.
echo   Como interactuar (100%% mouse):
echo     - Pasa el mouse: la camara/gota reacciona con brillo.
echo     - Clic en la gota: despliega el panel de proyectos.
echo     - Clic en [X] o fuera del panel: regresa a la gota.
echo     - Clic derecho en la gota: menu con opciones y Salir.
echo.
echo   Para hacer mas cambios y volver a verlos:
echo     Solo vuelve a ejecutar este archivo (previsualizar.bat).
echo ==========================================================
pause
