# zman

[English](README.md) · **Español**

Time tracker automático y local-first, escrito en Rust. Una alternativa open
source a RescueTime: registra qué app y qué sitio estás usando y por cuánto
tiempo, te muestra a dónde se va tu tiempo y deja que tu asistente de IA
responda preguntas sobre eso. Tus datos quedan en tu máquina salvo que elijas
sincronizarlos con tu propio servidor.

Sitio web y demo en vivo: https://malksof.github.io/zman/

> Estado: temprano. Desarrollado y probado en Windows. Ver
> [qué está verificado](#qué-está-verificado).

## Qué incluye

| Pieza | Qué hace |
|---|---|
| **App de escritorio** (`zman-app`) | Registra en segundo plano, vive en la bandeja del sistema, inicia con el sistema, muestra el dashboard y conecta asistentes de IA |
| **CLI** (`zman`) | `today`, `week`, `apps`, `watch`, `status`, `sync` |
| **Daemon** (`zman-daemon`) | Solo el tracker, para quien no quiera la app |
| **Servidor MCP local** (`zman-mcp`, o `zman-app --mcp`) | Herramientas de solo lectura para Claude y otros asistentes en esta computadora |
| **Extensión de navegador** (`extension/`) | Suma el sitio de la pestaña activa a tu registro |
| **Servidor de sync** (`zman-server`) | Junta todos tus dispositivos; MCP remoto con inicio de sesión para ChatGPT y Claude en la web |
| **App Android** (`android/`) | Suma el uso de apps de tu teléfono |

## Privacidad

- zman nunca registra teclas ni contenido de la pantalla. Lee el nombre de la
  app y el título de la ventana activa, y *hace cuánto* fue el último input.
- Las apps, títulos y sitios que excluyas nunca se escriben en disco.
- Las ventanas privadas del navegador nunca se informan. Los parámetros de las
  URLs nunca se guardan.
- Los asistentes de IA solo reciben datos agregados (tiempo por categoría, app
  y sitio). Los títulos de ventana nunca se exponen por MCP.
- Nada sale de tu computadora salvo que inicies sesión en un servidor de sync,
  y aun así podés dejar los títulos de ventana solo en tu equipo.

## Inicio rápido

Necesitás [Rust](https://rustup.rs). En Windows la app usa WebView2, que
Windows 10 y 11 ya traen.

```sh
cargo build --release
./target/release/zman-app
```

La app abre el dashboard y sigue registrando desde la bandeja cuando cerrás la
ventana. Activá **Iniciar con el sistema** en su menú de ajustes.

¿Preferís la terminal? Corré `./target/release/zman-daemon` y usá el CLI:

```sh
zman today            # horas, categorías, apps y sitios de hoy
zman week             # la semana, día por día
zman apps --week      # todas las apps
zman watch            # lo que se está registrando, en vivo
zman today --date 2026-11-19
```

Los mensajes salen en español si el sistema está en español, y en inglés en
cualquier otro caso. Para forzar un idioma, definí `ZMAN_LANG` (`es` o `en`).

## Conectá tu asistente de IA

Abrí la app y tocá **Conectar**.

- **Claude Desktop**: un clic, y después reiniciá Claude Desktop.
- **Claude Code**: ejecutá el comando que muestra el diálogo
  (`claude mcp add zman -- "<ruta>/zman-app" --mcp`).
- **ChatGPT, Claude en la web, o varios dispositivos**: iniciá sesión en un
  [servidor de sync](#servidor-de-sync) desde el mismo diálogo. Te da una
  dirección como `https://tu-servidor/mcp`; agregala como conector
  personalizado en tu asistente e iniciá sesión con el mismo email y
  contraseña.

Después preguntá cosas como *"¿en qué se me fue la semana?"*, *"compará esta
semana con la anterior"* o *"¿cuándo fueron mis bloques de foco ayer?"*.

Herramientas: `daily_summary`, `weekly_summary`, `time_by_category`,
`time_by_app`, `compare_periods`, `focus_blocks`.

## Extensión de navegador

Sin ella, un navegador cuenta como una sola app. Con ella, zman sabe el sitio.

1. Abrí `chrome://extensions` (o `edge://extensions`).
2. Activá el **Modo de desarrollador**.
3. **Cargar descomprimida** y elegí la carpeta `extension/`.

La extensión solo habla con zman en `127.0.0.1`. Su popup muestra si zman está
corriendo.

## Configuración

Opcional. `zman status` muestra dónde va `config.toml`
(`%APPDATA%\zman\config\config.toml` en Windows). Reiniciá el tracker después
de editarlo; los reportes toman los cambios de categorías al instante.

```toml
poll_interval_secs = 1     # cada cuánto se consulta la ventana activa
afk_threshold_secs = 180   # segundos sin input hasta considerarte ausente

[exclude]                  # nunca se registra; `*` es comodín, sin distinguir mayúsculas
apps = ["keepassxc.exe"]
titles = ["*InPrivate*", "*Private Browsing*", "*banco*"]
domains = ["mibanco.com"]

[browser]
port = 27184               # 0 apaga el soporte de la extensión
full_urls = true           # false guarda solo el sitio, no la ruta de la página

[sync]
titles = true              # false deja los títulos de ventana en este equipo

# Categorías, en orden de prioridad: gana la primera regla que coincide.
# Dentro de una regla, tienen que coincidir todos los tipos de patrón dados.
# score: de -2 (muy distractivo) a 2 (muy productivo).
[[categories]]
name = "Trabajo profundo"
score = 2
apps = ["code.exe", "windowsterminal.exe"]

[[categories]]
name = "Video"
score = -2
domains = ["youtube.com", "netflix.com"]

[[categories]]
name = "Video"
score = -2
apps = ["chrome.exe"]
titles = ["*youtube*"]
```

Sin `[[categories]]`, zman usa reglas incluidas (Desarrollo, Trabajo,
Comunicación, Navegación, Sistema, Entretenimiento). Si definís las tuyas,
reemplazan a esas. Las categorías se aplican al mirar los datos, así que
cambiar una regla recategoriza todo tu historial.

## Servidor de sync

Solo hace falta para juntar varios dispositivos, usar ChatGPT o Claude en la
web, o consultar tus datos desde cualquier lado. Necesita una máquina con
Docker y un dominio que apunte a ella.

```sh
cd deploy
cp .env.example .env     # poné tu dominio y una contraseña para la base
docker compose up -d
```

Eso levanta el servidor, Postgres y Caddy con HTTPS automático. Después, en
cada computadora:

```sh
zman sync login https://tu-dominio --register   # la primera vez
zman sync login https://tu-dominio              # los demás dispositivos
```

o usá **Conectar** en la app. Los dispositivos suben solos cada pocos minutos.
Cuando tu cuenta ya exista, poné `ZMAN_ALLOW_REGISTRATION=false` en `.env` y
volvé a correr `docker compose up -d` para que nadie más pueda registrarse.

Para probar el servidor sin Docker ni Postgres (los datos se pierden al salir):

```sh
DATABASE_URL=memory: cargo run -p zman-server
```

## Android

`android/` es un proyecto de Android Studio. La app pide *acceso de uso*,
muestra el tiempo de hoy por app y sube las sesiones a tu servidor de sync
cada 30 minutos. Abrí la carpeta en Android Studio y corrélo en un teléfono.

## Estructura

- `crates/core` — modelo de eventos, storage SQLite, config, categorías, reportes.
- `crates/daemon` — loop de tracking, receptor del navegador, cliente de sync;
  el código específico de cada sistema operativo está detrás del trait
  `Platform` en `src/platform`.
- `crates/cli` — el comando `zman`.
- `crates/mcp` — protocolo MCP y herramientas, compartidos por el servidor local y el remoto.
- `crates/desktop` — app Tauri; el dashboard es HTML/CSS/JS plano en `ui/`.
- `crates/server` — API de sync, OAuth y MCP remoto (Axum + Postgres).
- `extension/` — extensión de navegador. `android/` — app Android. `deploy/` — Docker Compose.

Datos de demo, para probar los reportes y el dashboard enseguida:

```sh
ZMAN_HOME=./demo cargo run -p zman-core --example seed_demo
ZMAN_HOME=./demo cargo run -p zman-cli -- week
```

## Qué está verificado

- **Probado en Windows 11**: tracker, CLI, MCP local, app de escritorio, el
  receptor del navegador (con pedidos simulados) y el servidor con
  almacenamiento en memoria, incluyendo todo el flujo OAuth y el MCP remoto.
  `cargo test` cubre la lógica del core, las herramientas MCP y la API HTTP
  del servidor.
- **Todavía sin correr**: el servidor contra un Postgres real, la imagen de
  Docker, la extensión dentro de un navegador, la app Android (escrita sin un
  SDK de Android a mano) y la conexión de ChatGPT o Claude en la web a un
  servidor desplegado. macOS y Linux necesitan una implementación de
  `Platform`.

## Licencia

MIT
