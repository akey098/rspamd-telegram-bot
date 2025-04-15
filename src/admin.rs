use teloxide::Bot;
use teloxide::prelude::{Message, Requester, ResponseResult};
use crate::commands::AdminCommand;
async fn handle_admin_command(bot: Bot, msg: Message, cmd: AdminCommand) -> ResponseResult<()> {
    let admin_ids = vec![12345678, 87654321]; // allowed admins
    if let Some(user) = msg.from() {
        if !admin_ids.contains(&user.id.0) {
            bot.send_message(msg.chat.id, "Unauthorized").await?;
            return Ok(());
        }
    }
    match cmd {
        AdminCommand::Enable { feature } => { /* enable logic */ },
        // ... other commands ...
        AdminCommand::AddRegex { pattern } => { /* add regex logic */ },
        _ => { /* ... */ },
    }
    Ok(())
}
