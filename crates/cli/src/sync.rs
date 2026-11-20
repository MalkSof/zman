//! `zman sync`: sign this device in to a zman server and upload events.

use std::error::Error;
use std::io::Write;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use zman_core::{Config, Lang, Paths, Store};
use zman_daemon::sync;

fn open() -> Result<(Store, Config), Box<dyn Error>> {
    let paths = Paths::resolve()?;
    Ok((Store::open(&paths.db_file)?, Config::load(&paths.config_file)?))
}

fn prompt(label: &str) -> Result<String, Box<dyn Error>> {
    print!("{label}: ");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

pub fn login(lang: Lang, server: &str, email: Option<String>, register: bool) -> Result<(), Box<dyn Error>> {
    let (store, config) = open()?;
    let server = sync::normalize_server(server)?;
    let email = match email {
        Some(email) => email,
        None => prompt("Email")?,
    };
    // Typed without echo and never stored on this device. Scripts can pass
    // it in the ZMAN_PASSWORD environment variable instead.
    let from_env = std::env::var("ZMAN_PASSWORD").ok().filter(|p| !p.is_empty());
    let scripted = from_env.is_some();
    let password = match from_env {
        Some(password) => password,
        None => rpassword::prompt_password(lang.pick("Password: ", "Contraseña: "))?,
    };
    if register && !scripted {
        let again = rpassword::prompt_password(lang.pick("Repeat password: ", "Repetí la contraseña: "))?;
        if password != again {
            return Err(lang.pick("the passwords do not match", "las contraseñas no coinciden").into());
        }
    }
    let account = sync::login(&store, &server, &email, &password, register)?;
    println!(
        "{} {} ({})",
        lang.pick("Signed in as", "Sesión iniciada como"),
        account.email,
        account.server
    );
    now(lang, &store, &config)?;
    println!();
    println!(
        "{}",
        lang.pick(
            "To query all your devices from Claude or ChatGPT, add this MCP server there:",
            "Para consultar todos tus dispositivos desde Claude o ChatGPT, agregá ahí este servidor MCP:",
        )
    );
    println!("  {}/mcp", account.server);
    Ok(())
}

fn now(lang: Lang, store: &Store, config: &Config) -> Result<(), Box<dyn Error>> {
    match sync::run(store, config)? {
        Some(count) => println!("{} {count}", lang.pick("Events uploaded:", "Eventos subidos:")),
        None => println!(
            "{}",
            lang.pick(
                "Not signed in. Use `zman sync login <server>` first.",
                "Sin sesión. Usá primero `zman sync login <servidor>`.",
            )
        ),
    }
    Ok(())
}

pub fn run_now(lang: Lang) -> Result<(), Box<dyn Error>> {
    let (store, config) = open()?;
    now(lang, &store, &config)
}

pub fn status(lang: Lang) -> Result<(), Box<dyn Error>> {
    let (store, _) = open()?;
    let status = sync::status(&store)?;
    let Some(account) = status.account else {
        println!("{}", lang.pick("Sync is off: this device is not signed in.", "Sync apagado: este dispositivo no tiene sesión."));
        return Ok(());
    };
    let last = match status.last_sync_ms.and_then(|ms| Timestamp::from_millisecond(ms).ok()) {
        Some(ts) => ts.to_zoned_datetime(TimeZone::system()).strftime("%Y-%m-%d %H:%M").to_string(),
        None => lang.pick("never", "nunca").to_string(),
    };
    println!("{} {}", lang.pick("Server:   ", "Servidor: "), account.server);
    println!("{} {}", lang.pick("Account:  ", "Cuenta:   "), account.email);
    println!("{} {last}", lang.pick("Last sync:", "Último:   "));
    println!("{} {}/mcp", lang.pick("MCP:      ", "MCP:      "), account.server);
    Ok(())
}

pub fn logout(lang: Lang) -> Result<(), Box<dyn Error>> {
    let (store, _) = open()?;
    sync::logout(&store)?;
    println!("{}", lang.pick("Signed out. Local data was not touched.", "Sesión cerrada. Los datos locales no se tocaron."));
    Ok(())
}
