use std::time::Duration;
use std::{fs, io};
use std::path::Path;
use std::process::Command;
use std::error::Error; 
use redis::Commands;
use teloxide::prelude::*;
use tokio::time;

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

    tokio::spawn({
        async move {
            let mut interval = time::interval(Duration::from_secs(3600));
            loop {
                interval.tick().await;
                if let Err(err) = do_periodic().await {
                    log::error!("Periodic task failed: {:?}", err);
                }
            }
        }
    });

    admin_handlers::run_dispatcher(bot).await;
}

async fn do_periodic() -> Result<(), Box<dyn Error + Send + Sync>> {
    let redis_client = redis::Client::open("redis://127.0.0.1/")
        .expect("Failed to connect to Redis");
    let mut redis_conn = redis_client
        .get_connection()
        .expect("Failed to get Redis connection");

    let keys: Vec<String> = redis_conn
        .keys("tg:users:*")
        .expect("Failed to get users keys");

    for key in keys {
        let rep: i64 = redis_conn
            .hget(&key, "rep")
            .expect("Failed to get user's reputation");
        if rep > 0 {
            let _: () = redis_conn
                .hincr(&key, "rep", -1)
                .expect("Failed to decrease user's reputation");
        }
    }

    Ok(())
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
    let src_lua = Path::new("../../rspamd-config/lua.local.d");
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

    let reload = Command::new("service")
        .arg("rspamd")
        .arg("restart")
        .status()?;
    if !reload.success() {
        eprintln!("rspamd restart failed");
    }

    Ok(())
}
