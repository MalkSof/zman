# zman

[English](README.md) · **Español**

Time tracker automático y local-first, escrito en Rust. Una alternativa open
source a RescueTime: registra qué ventana estás usando y por cuánto tiempo, y
guarda los datos en tu máquina.

> Estado: desarrollo temprano. Por ahora solo Windows.

## Privacidad

- zman nunca registra teclas ni contenido de la pantalla. Solo lee el nombre
  de la app y el título de la ventana activa, y *hace cuánto* fue el último
  input.
- Las apps y títulos que excluyas nunca se escriben en disco.
- Todos los datos quedan en un archivo SQLite local.

## Compilar y correr

```sh
cargo build --release
```

Iniciá el tracker y dejalo corriendo:

```sh
./target/release/zman-daemon
```

En otra terminal:

```sh
./target/release/zman watch    # ver en vivo lo que se está registrando
./target/release/zman status   # ubicación de archivos, id de dispositivo, cantidad de eventos
```

Los mensajes salen en español si el sistema está en español, y en inglés en
cualquier otro caso. Para forzar un idioma, definí la variable de entorno
`ZMAN_LANG` (`es` o `en`).

## Configuración

Opcional. `zman status` muestra dónde va el archivo
(`%APPDATA%\zman\config\config.toml` en Windows). Reiniciá el daemon después
de editarlo.

```toml
# Cada cuánto se consulta la ventana activa.
poll_interval_secs = 1
# Segundos sin input de teclado/mouse hasta considerarte ausente.
# El tiempo ausente no se registra.
afk_threshold_secs = 180

[exclude]
# Sin distinguir mayúsculas; `*` es un comodín.
apps = ["keepassxc.exe"]
titles = ["*InPrivate*", "*Private Browsing*", "*Navegación privada*", "*banco*"]
```

Si omitís `titles`, por defecto se excluyen las ventanas de navegación privada
de Edge y Firefox.

## Estructura

- `crates/core` — modelo de eventos, storage SQLite, config y lógica de
  tracking.
- `crates/daemon` — proceso en segundo plano; el código específico de cada
  sistema operativo está detrás del trait `Platform` en `src/platform`.
- `crates/cli` — el comando `zman`.

## Licencia

MIT
