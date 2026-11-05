# Console: guía para trabajar en el código

Terminal para Windows en Rust (eframe/egui 0.29): pestañas con cmd/PowerShell (ConPTY),
árbol de comandos guardados, historial por pestaña, copia de texto que funciona aunque el
programa de dentro capture el ratón, ventana sin marco con bandeja del sistema, datos cifrados.
El `README.md` es para el repositorio público y va en inglés; esta guía, en español.

## Licencia

`LICENSE` es propia (código visible, no open source): uso personal gratis de los ejecutables
oficiales; uso comercial con contrato; el código no se puede redistribuir ni reutilizar. Los
textos de la app y el README dicen "gratuita para uso personal": mantenerlos coherentes con
ella si cambia.

## Convenciones

- **Arquitectura hexagonal.** La dependencia siempre apunta hacia dentro:
  `adapters` → `application` → `domain`. `main.rs` es la raíz de composición: el único sitio
  que decide qué adaptador implementa cada puerto.
- **Código legible y nombres declarativos**: identificadores en inglés que dicen lo que hacen;
  **comentarios en español**.
- **Ficheros con finales de línea LF.** No insertar `\r\n` a mano en ellos.
- **Textos de la interfaz siempre traducidos**: nunca literales en el código (ver Traducciones).
- **Nombres de lo que se guarda en disco, en inglés** (`commands.json`, `tabs\`...), para que no
  dependan del idioma. Si se renombra algo guardado, añadir la migración en
  `adapters/data_folder.rs` (`rename_files_of_older_versions`), sin pisar nunca un fichero nuevo.
- La lógica nueva va con tests, a ser posible en el dominio (puro, sin ventanas ni ficheros).

## Compilar y probar

```
cargo build                       # depuración
cargo test                        # versión normal
cargo build --features portable   # comprobar que la portable compila
cargo test --features portable
cargo build --release
cargo build --release --features portable --target-dir target\portable
```

- Comprobar **las dos variantes** (normal y portable): hay código con `#[cfg(feature = "portable")]`.
- Sin avisos: los avisos se tratan como errores a corregir.
- `cargo` está en `%USERPROFILE%\.cargo\bin` (añadirlo al `PATH` de cada shell si hace falta).
- **Si la release falla con "failed to remove file ... console.exe" es que el usuario tiene la
  app abierta.** No cerrarla por cuenta propia (puede tener pestañas sin guardar): pedirle que
  salga con ⏻ y que diga "compila".
- Para ver la interfaz: copiar el `.exe` a una carpeta de pruebas del scratchpad y capturar la
  ventana con `PrintWindow`. Antes de abrir la versión normal, comprobar que no haya ajustes del
  usuario con autoguardado (escribiría en sus pestañas). Los clics simulados no funcionan aquí;
  solo enviar teclas si `GetForegroundWindow()` es la ventana de pruebas, nunca si no.

## Publicar una versión

```
./scripts/release.ps1 1.2.0 -Notes "Lo que ha cambiado"
```

El script comprueba que no haya cambios sin confirmar, pasa los tests de las dos variantes,
sube la versión en `Cargo.toml`, crea el commit y la etiqueta `v1.2.0` (con las novedades
como mensaje) y lo sube. Al llegar la etiqueta, `.github/workflows/release.yml` compila en
GitHub cada tipo de programa, calcula `SHA256SUMS.txt` y publica la release. No hay
credenciales en el repositorio: el script usa el acceso normal a git y el flujo, el token
temporal de GitHub Actions.

Cada tipo de programa se publica con un nombre fijo (`BuildKind::release_file_name` en
`domain/updates.rs`): `console-windows-x64.exe`, `console-windows-x64-portable.exe` y, cuando
exista Linux, `console-linux-x64` y `console-linux-x64-portable` (están comentados en la
matriz del flujo). **Si se cambia un nombre, hay que cambiarlo en los dos sitios.**

Para que se pueda comprobar que un ejecutable es el oficial, el flujo firma una atestación de
procedencia de GitHub por fichero (`actions/attest-build-provenance`; se comprueba con
`gh attestation verify <fichero> --repo manuelmsni/console`) y pone las sumas SHA-256 en las
notas de la release. Las atestaciones son gratis en repositorios públicos; en privados
requieren GitHub Enterprise.

## Actualizaciones

- La app consulta `https://api.github.com/repos/manuelmsni/console/releases/latest`
  (`REPOSITORY` en `src/project.rs`) al abrir y una vez al día, en segundo plano.
- Solo ofrece una release más nueva que `CARGO_PKG_VERSION`, no saltada por el usuario, que
  traiga el ejecutable de **su** tipo (sistema + normal/portable) y `SHA256SUMS.txt`.
- Es discreta: un botón pequeño en la barra de pestañas; si la búsqueda automática falla,
  no avisa. Sin nada que actualizar, en ese sitio hay una taza de café pequeña que abre la
  misma ventana: versión actual, si es la última, enlace al repositorio y, debajo, la
  sección de apoyo voluntario ("Invítame a un café", GitHub Sponsors).
- Los enlaces del proyecto (repositorio y página de apoyo) están en `src/project.rs`. Ajustes → General permite desactivarla o buscar a mano.
- Al aceptar: descarga, comprueba la suma SHA-256, renombra el `.exe` actual a `.exe.old`
  (Windows deja renombrar un programa abierto, no borrarlo), pone el nuevo en su sitio y sale
  como siempre (preguntando por las pestañas sin guardar). Al salir arranca el nuevo con
  `--after-update <pid>`, que espera a que se cierre el anterior (por la instancia única) y
  borra el `.old`.
- Test contra GitHub de verdad (no se ejecuta por defecto): `cargo test -- --ignored github`.

## Estructura

```
src/
├── main.rs                 Raíz de composición (variante normal y portable).
├── i18n.rs                 tr()/tr_args() y los tests de cobertura de traducciones.
├── domain/                 Reglas puras, con tests.
│   ├── command_library.rs    Árbol de carpetas y comandos (plegadas, filtro, mover).
│   ├── app_settings.rs       Idioma, autoguardado y atajos de teclado.
│   ├── command_history.rs    Historial de comandos de una pestaña.
│   ├── local_time.rs         Fecha y hora local.
│   ├── master_password.rs    Reglas de la contraseña de la portable.
│   ├── tab_journal.rs        Entradas del diario de una pestaña.
│   ├── ssh.rs                Servidores, claves y comandos de OpenSSH.
│   ├── shell_prompt.rs       Reconocer el prompt de cmd y PowerShell.
│   ├── terminal_grid.rs      Tamaños, posiciones y selección de texto.
│   ├── terminal_keys.rs      Qué bytes recibe la terminal por cada tecla.
│   ├── screen_snapshot.rs    Foto de la pantalla lista para dibujar.
│   ├── web_links.rs          Enlaces web en pantalla (Ctrl + clic), también partidos en filas.
│   ├── updates.rs            Versiones, qué fichero de la release es de este programa, sumas.
│   └── shell_profile.rs      Tipos de terminal que se pueden abrir.
├── application/            Casos de uso y puertos (traits).
│   ├── ports.rs              Lo que la aplicación necesita del exterior.
│   ├── terminal_session.rs   Una pestaña: proceso, pantalla, selección, historial y diario.
│   ├── terminal_tabs.rs      Abrir, cerrar, renombrar, reabrir y guardar pestañas.
│   ├── app_settings_service.rs  Cambiar los ajustes generales y guardarlos.
│   ├── command_library_service.rs  Cambiar la biblioteca y guardarla.
│   ├── ssh_settings.rs       Casos de uso de la pestaña "Claves SSH".
│   └── update_service.rs     Buscar, descargar comprobando la suma e instalar versiones nuevas.
└── adapters/               Implementaciones concretas.
    ├── pty_shell.rs          Procesos en una pseudoterminal (ConPTY en Windows).
    ├── vt100_emulator.rs     Interpreta colores, cursor, borrados...
    ├── encrypted_tab_journal_store.rs  Diarios de las pestañas (registros cifrados por separado).
    ├── json_command_library_store.rs   Comandos guardados.
    ├── json_ssh_server_store.rs        Servidores SSH.
    ├── json_app_settings_store.rs      Ajustes generales.
    ├── data_folder.rs        Carpeta de datos, lectura/escritura cifrada y migración de nombres.
    ├── file_cipher.rs        Trait del cifrado de ficheros.
    ├── password_cipher.rs    Cifrado con contraseña de la portable (Argon2id + XChaCha20-Poly1305).
    ├── system_clipboard.rs   Portapapeles.
    ├── github_releases.rs    Releases de GitHub (API pública, sin credenciales).
    ├── self_replacing_program.rs  Cambiar el ejecutable por la versión nueva.
    ├── platform/             Lo que depende del sistema operativo (ver abajo).
    │   └── windows/            Hora, idioma, DPAPI, instancia única, OpenSSH, ventana,
    │                           navegador y terminales disponibles.
    └── egui_ui/              La ventana: pestañas, menú lateral, terminal, ajustes, diálogos,
                              pantalla de contraseña de la portable...
```

Para añadir otra terminal (PowerShell 7, WSL...): `available_shell_profiles()` en
`adapters/platform/windows/shell_profiles.rs` (la primera es la principal). Para añadir una
sección a los ajustes: un valor más en `SettingsTab` (`settings_window.rs`) y su módulo.

## Datos y cifrado

- **Normal**: `%APPDATA%\console\`, cifrado con DPAPI (cuenta de Windows, sin contraseña).
- **Portable** (`--features portable`): carpeta `data` junto al `.exe`, cifrado con contraseña.
  La clave sale de Argon2id (64 MiB, 3 pasadas; parámetros guardados en `key.json`) y cada
  fichero va con XChaCha20-Poly1305. La contraseña no se guarda nunca. Cada carpeta portable
  usa su propia instancia única (puede convivir con la normal).
- Cada fichero cifrado empieza por una marca (`CONSOLA-DPAPI-1`, `CONSOLA-CLAVE-1`): no
  cambiarlas, o los ficheros ya guardados dejarían de leerse. Por lo mismo, tampoco
  `APP_ENTROPY` (DPAPI) ni `CHECK_TEXT` (`key.json`), aunque digan "consola": el programa se
  llamaba así antes de ser `console`. La carpeta antigua `%APPDATA%\consola` se renombra sola.
- Ficheros: `commands.json`, `settings.json`, `ssh_servers.json`, `tabs\index.json`,
  `tabs\<id>.journal` (se compacta al pasar de 4 MB), `key.json` (solo portable).
- Las claves SSH están en `%USERPROFILE%\.ssh` (las usa el `ssh` del sistema).

## Otros sistemas operativos

Solo funciona en Windows, pero está preparada: todo lo que depende del sistema está en
`src/adapters/platform/`, y cada sistema ofrece lo mismo con los mismos nombres (tabla en
`platform/mod.rs`). Para Linux: crear `platform/linux/` con lo de la tabla y elegirla con
`#[cfg(target_os = "linux")]`. `windows-sys` solo se usa al compilar para Windows.

## Traducciones

- Un fichero por idioma en `locales/` (`en.json` es la base). Se incluyen al compilar (`build.rs`).
- En el código: `tr("clave")` o `tr_args("clave", &[("nombre", &valor)])`.
- Los tests de `src/i18n.rs` exigen que toda clave usada exista en todos los idiomas, que todos
  tengan las mismas claves y marcadores que `en.json` y que no sobre ninguna sin usar. Al añadir
  o quitar un texto, tocar **todos** los ficheros de `locales/`.
