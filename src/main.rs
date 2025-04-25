use std::{fs, io};
use std::path::Path;
use std::process::Command;
use teloxide::prelude::*;

mod admin_handlers;
mod handlers;
mod utils;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting the spam detection bot...");

    if let Err(e) = deploy_settings() {
        eprintln!("Failed to sync Rspamd config: {}", e);
    }

    let bot = Bot::from_env();

    admin_handlers::run_dispatcher(bot).await;
}



fn deploy_settings() -> io::Result<()> {
    let ip_cidr = utils::detect_local_ipv4().unwrap_or_else(|| "127.0.0.1/32".to_string());
    println!("Detected local IPv4: {}", ip_cidr);

    let settings = format!(r#"
    internal_hosts {{
    priority = 10;
    ip = "{ip}";
    apply {{
        HFILTER_HOSTNAME_UNKNOWN = 0.0;
    }}
    }}
    "#, ip = ip_cidr);

    let settings_dst = Path::new("/etc/rspamd/local.d/settings.conf");
    if let Some(parent) = settings_dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(settings_dst, settings.trim_start())?;
    println!("Wrote dynamic settings to {}", settings_dst.display());

    println!("Current dir: {}", std::env::current_dir()?.display());
    let src_lua = Path::new("../../rspamd-config/lua_local.d");
    let dst_lua = Path::new("/etc/rspamd/lua.local.d");
    if src_lua.exists() {
        fs::create_dir_all(&dst_lua)?;
        for entry in fs::read_dir(src_lua)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let file_name = entry.file_name();
                let dst_path = dst_lua.join(&file_name);
                fs::copy(&path, &dst_path)?;
                println!("Copied {} → {}", path.display(), dst_path.display());
            }
        }
    } else {
        println!("No ../../rspamd-config/lua_local.d directory found, skipping Lua copy");
    }

    let test = Command::new("rspamadm")
        .arg("configtest")
        .output()?;
    if !test.status.success() {
        eprintln!("configtest failed:\n{}",
                  String::from_utf8_lossy(&test.stderr));
    }

    let reload = Command::new("rspamadm")
        .arg("control")
        .arg("reload")
        .status()?;
    if !reload.success() {
        eprintln!("rspamadm control reload failed");
    }

    Ok(())
}
