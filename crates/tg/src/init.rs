//! `tg init` — interactive config wizard.

use anyhow::Result;
use tg_core::config::TgConfig;

pub fn run() -> Result<()> {
    let path = TgConfig::config_path();
    if path.exists() {
        println!("⚠️  Config exists at {}", path.display());
        return Ok(());
    }

    println!("🚀  Telegram CLI — Initial Setup\n");

    let api_id: i32 = loop {
        let mut buf = String::new();
        print!("API ID (https://my.telegram.org/apps): ");
        std::io::Write::flush(&mut std::io::stdout())?;
        std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut buf)?;
        match buf.trim().parse() {
            Ok(n) => break n,
            Err(_) => println!("  Enter a valid number."),
        }
    };

    let mut api_hash = String::new();
    print!("API Hash: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut api_hash)?;

    let mut phone = String::new();
    print!("Phone (+86..., or blank): ");
    std::io::Write::flush(&mut std::io::stdout())?;
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut phone)?;

    let mut config = TgConfig::default();
    config.telegram.api_id = api_id;
    config.telegram.api_hash = api_hash.trim().to_string();
    config.telegram.phone = phone.trim().to_string();
    config.tdlib.application_version = env!("CARGO_PKG_VERSION").to_string();

    config.save()?;
    let _ = config.store_keyring();

    println!("\n✅  Saved to {}", path.display());
    println!("   Next: tgcd & && tg login");
    Ok(())
}
