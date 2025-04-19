use dotenv::*;
use std::{env, fs, io};
use std::path::Path;
use teloxide::prelude::*;

mod admin_handlers;
mod handlers;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    dotenv().ok();
    log::info!("Starting the spam detection bot...");

    if let Err(e) = sync_rspamd_config() {
        eprintln!("Failed to sync Rspamd config: {}", e);
        // Depending on your needs, you might want to exit here:
        // std::process::exit(1);
    }

    let bot_token = env::var("BOT_TOKEN").expect("BOT_TOKEN must be set in .env file");

    let bot = Bot::new(bot_token);

    admin_handlers::run_dispatcher(bot).await;
}

fn sync_rspamd_config() -> io::Result<()> {
    // 1. Define source & destination directories
    let src_dir = Path::new("../rspamd-config/lua_local.d/");
    println!("{}", src_dir.to_str().unwrap().to_string());
    let dst_dir = Path::new("/etc/rspamd/lua.local.d/");
    println!("{}", dst_dir.to_str().unwrap().to_string());

    // 2. Copy each file
    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let file_name = entry.file_name();
            let dest_path = dst_dir.join(&file_name);
            // Overwrite the old file
            fs::copy(&path, &dest_path)?;
            println!("Copied {:?} → {:?}", path, dest_path);
        }
    }
    Ok(())
}
