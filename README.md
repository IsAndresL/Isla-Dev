# Isla Dev — Isla Dinámica Nativa para Proyectos de Desarrollo

Una **Isla Dinámica** moderna inspirada en Apple para Windows, diseñada específicamente para desarrolladores. 100% nativa en **Rust + Win32 + Direct2D**, sin tecnologías web, sin Electron y con consumo ultrabajo de recursos (menos de 1 MB de ejecutable y mínimo consumo de RAM).

---

## ⚡ Características Principales

- **Diseño Dinámico estilo Apple:** Flota en la parte superior central de tu pantalla con animaciones de resorte amortiguadas (*spring physics*), aro de luz animado en hover, esquinas redondeadas y sombras volumétricas.
- **Gestión de Servidores Locales y Puertos ("Live Dev Servers"):**
  - **Indicador en la gota en reposo:** Punto verde luminoso en la gota superior cuando tienes un servidor activo (ej. `:5173`, `:3000`, `:4321`, `:8000`).
  - **Botón Play / Stop (`▶` / `■`):** Arranca `npm run dev` o tu servidor Python directamente desde la fila del proyecto sin abrir consolas molestas.
  - **Acceso y Liberación de Puertos:** Clic en el badge `:5173` para abrir `http://localhost:5173` en el navegador, o clic en `×` en la barra de puertos para matar procesos colgados en puertos ocupados.
- **Estado de Git en Tiempo Real:**
  - Muestra la rama actual (`⎇ main`) leyendo `.git/HEAD` en 0 ms.
  - Indicador ámbar con conteo de archivos modificados sin guardar en commit (`● 3`).
  - Acciones rápidas en menú contextual para hacer `Git Pull` o abrir el repositorio remoto en GitHub.
- **Buscador Instantáneo ("Spotlight") y Favoritos (`📌`):**
  - Escribe directamente al abrir la isla para filtrar por nombre, tecnología o rama Git; usa las flechas y presiona `Enter` para abrir al instante.
  - Fija tus proyectos favoritos (`📌`) arriba del todo para que siempre aparezcan primero.
- **Acciones Secundarias Rápidas por Proyecto:**
  - Mini-botones en hover para iniciar/detener servidor (`▶`/`■`), abrir en Terminal (`>_`), abrir en Explorador (`📁`) y fijar (`📌`).
  - Detección automática de todos los scripts de `package.json` (`dev`, `build`, `lint`, `test`, etc.) en el menú de clic derecho.
- **Lanzadores Integrados:**
  - **Antigravity IDE** (por defecto)
  - **VS Code** (`code "<ruta>"`)
  - **Cursor** (`cursor "<ruta>"`)
  - **OpenCode** (Abre Windows Terminal con `opencode` en la carpeta exacta del proyecto)
  - **Gemini CLI** (Abre Windows Terminal con `gemini` en la carpeta exacta del proyecto)
  - **Terminal / Explorador de archivos**
- **Asistente de Creación de Proyectos:**
  - Proyecto limpio (solo carpeta).
  - Plantillas listas: **Astro**, **Vite + React (TypeScript)**, **Vite + Vue (TypeScript)**, **Next.js**, **Node.js**, **Python + venv**.
  - Selector de librerías opcionales (Tailwind CSS, React Router, Zustand, Motion, Zod, Flask, FastAPI, etc.).
  - Creación en segundo plano sin congelar la interfaz ni abrir molestas consolas negras.
- **Atajos de Teclado:**
  - `Ctrl + Alt + Espacio`: Mostrar / expandir la isla desde cualquier lugar de Windows.
  - `Ctrl + 1` .. `Ctrl + 9`: Abrir el proyecto correspondiente al instante.
  - `Ctrl + N`: Crear un nuevo proyecto.
  - `Esc`: Volver o cerrar el menú.
- **Configuración Editable:**
  - Archivo `%APPDATA%\IslaDev\config.toml` con recarga en caliente instantánea al guardar.
  - Opción de **Iniciar con Windows** (activable en Ajustes o en el instalador).
  - Icono en la bandeja del sistema (System Tray) para abrir, cambiar ajustes o salir.

---

## 📦 Instalador y Ejecutable

- **Instalador Oficial:** [`dist/IslaDev-Setup-0.1.0.exe`](file:///c:/Luna-Dev%20Compartido/Dev/Luna%20-%20Personal/Organizador%20de%20proyectos/dist/IslaDev-Setup-0.1.0.exe) (~2.3 MB)
  - No requiere permisos de administrador.
  - Permite activar la casilla **"Iniciar Isla Dev al encender el PC"**.
- **Ejecutable Portable:** [`target/x86_64-pc-windows-gnu/release/isla-dev.exe`](file:///c:/Luna-Dev%20Compartido/Dev/Luna%20-%20Personal/Organizador%20de%20proyectos/target/x86_64-pc-windows-gnu/release/isla-dev.exe) (~814 KB)

---

## 🛠️ Cómo compilarlo tú mismo

1. **Compilar el ejecutable:**
   ```powershell
   cargo build --release
   ```
2. **Generar el instalador (con Inno Setup):**
   ```powershell
   & "C:\Users\Andres\AppData\Local\Programs\Inno Setup 6\ISCC.exe" installer\isla.iss
   ```
