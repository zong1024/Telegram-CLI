//! `tg init` — interactive config wizard.

use anyhow::Result;
use tg_common::config::TgConfig;

pub fn run() -> Result<()> {
    let path = TgConfig::config_path();
    if path.exists() {
        println!("⚠️  Config already exists at {}", path.display());
        println!("   Edit it directly, or delete and re-run `tg init`.");
        return Ok(());
    }

    println!("🚀  Telegram CLI — Initial Setup\n");

    let api_id: i32 = loop {
        let mut buf = String::new();
        print!("Enter API ID (from https://my.telegram.org/apps): ");
        std::io::Write::flush(&mut std::io::stdout())?;
        std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut buf)?;
        match buf.trim().parse() {
            Ok(n) => break n,
            Err(_) => println!("  Please enter a valid number."),
        }
    };

    let mut api_hash = String::new();
    print!("Enter API Hash: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut api_hash)?;

    let mut phone = String::new();
    print!("Enter phone (international, e.g. +86..., or leave blank to set later): ");
    std::io::Write::flush(&mut std::io::stdout())?;
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut phone)?;

    let config = TgConfig {
        api_id,
        api_hash: api_hash.trim().to_string(),
        phone: phone.trim().to_string(),
        socket_path: tg_common::config::default_socket_path(),
        database_dir: tg_common::config::default_database_dir(),
        verbosity: 0,
        test: false,
    };

    config.save()?;

    println!("\n✅  Config saved to {}", path.display());
    println!("   Next: start the daemon with `tg-daemon`, then log in with `tg login`.");
    Ok(())
}
