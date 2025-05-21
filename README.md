Thanks! I’ll prepare a comprehensive `README.md` for the 'Full Telegram Support for Spam Filtering' project. It will include installation instructions, usage examples, and reference that the project is licensed under Apache. I’ll also highlight that it is currently in the implementation phase. I’ll share it with you shortly.


# Full Telegram Support for Spam Filtering

**Full Telegram Support for Spam Filtering** is an open-source Telegram bot that integrates with the Rspamd spam filtering engine to provide real-time spam detection and automated moderation in Telegram chats. The bot is written in **Rust** for high-performance Telegram API integration, and it uses **Lua** scripting (within Rspamd) to define flexible spam detection rules. This project enables community moderators to combat spam in group chats more effectively by combining Rspamd’s powerful filtering capabilities with a user-friendly Telegram-based interface.

## Features

* **Real-time spam detection & reporting**: The bot scans incoming messages in Telegram **as soon as they are posted**. It leverages Rspamd to analyze content and detect spam in real time. Suspicious messages are flagged instantly – heavily spammy content is **automatically deleted** and the event is reported to moderators, while borderline cases may trigger warnings for review. This real-time reporting ensures spam is removed before it can disrupt the chat.

* **Rate limiting (anti-flood)**: The system includes rate limiting to thwart spammers who flood chats. If a user sends too many messages too quickly (or repeats the same text repeatedly), the bot’s Lua rules will identify this flood behavior. Such messages are then marked as spam (using a custom **TG\_FLOOD** or **TG\_REPEAT** rule) and removed. This protects chats from mass spam or annoying repeated content by throttling excessive message bursts automatically.

* **User reputation management**: Each user in the chat is assigned a dynamic **reputation score** that reflects their history of spam-like behavior. When the bot catches a user sending spam, it increments that user’s “spam reputation” score in a Redis-backed database. Users with a higher rep score (due to repeated offenses) will be treated as more suspicious – if they continue spamming, the bot can more aggressively flag their messages (e.g. marking them as **“suspicious”** immediately). Importantly, reputation **decays over time** (the bot periodically lowers rep scores for users who haven’t spammed recently), so users can redeem themselves with good behavior. This reputation system helps differentiate one-time offenders from chronic spammers.

* **Moderation UI (Telegram-based)**: All moderation and configuration happens through Telegram itself – there’s no need for an external dashboard. The bot provides a set of **admin commands** and interactive prompts to control its behavior:

    * Moderators can designate a private Telegram chat or channel as an **admin control panel** for the bot. Using a command (e.g. `/make_admin`), the bot will register that chat for administration and allow the admin to link it with the group(s) they want to moderate. The bot then sends spam alerts and reports to this admin chat, keeping the main group clutter-free.
    * The bot sends notifications to admins via Telegram messages. For example, when a message is deleted for spam, the bot can post a notice in the admin chat (or in the group, if no separate admin chat is set) explaining what was removed and why. Admins can also receive warnings for borderline spam cases, so they can manually review if needed.
    * An inline button interface is used for certain setup tasks (for instance, after using the `/make_admin` command, the bot presents a list of your groups to choose which ones to moderate, all within Telegram’s UI). This makes configuration intuitive and mobile-friendly.

* **Lua-based automation rule engine**: Under the hood, the spam detection logic is powered by Lua scripts integrated with **Rspamd**. The bot comes with a set of custom Lua rules that define what constitutes spam in a Telegram context – for example, rules to catch message flooding, repeated messages, or other suspicious patterns specific to chat behavior. Because these rules are written in Lua (Rspamd’s native scripting language), they are **highly customizable**. Developers or advanced admins can extend or modify the automation rules without changing the Rust code: simply update or add new Lua scripts to adjust spam detection criteria (e.g. add a regex pattern to catch certain phrases, or tweak thresholds for flags). This flexibility means the bot’s filtering logic can evolve to meet new spam tactics. The Rspamd engine evaluates these Lua rules and assigns spam scores to messages, which the bot then uses to decide on actions (warn, delete, etc.). By using Rspamd’s API and Lua extensibility, the project combines robust spam filtering algorithms with the ability to adapt to community-specific needs.

## Technology Stack

* **Rust** – The core bot backend is implemented in Rust. Rust’s safety and concurrency features ensure the bot runs efficiently and reliably even under high message throughput. The bot uses the async Rust ecosystem and the [Teloxide](https://github.com/teloxide/teloxide) framework to interact with the Telegram Bot API. Rust was chosen for its performance (important when analyzing messages in real time) and stability for long-running services.

* **Lua** – Lua is used for scripting spam filtering rules within Rspamd. The project includes Lua scripts that Rspamd executes for each message, applying heuristic rules (like detecting floods or repeats). Lua provides a lightweight, flexible way to express custom logic. By offloading spam detection to Lua scripts in Rspamd, the bot can be easily tuned or extended by editing these scripts, without recompiling the Rust code. This separation of concerns (Rust for bot logic vs. Lua for filtering rules) makes the system both powerful and adaptable.

* **Rspamd (HTTP API)** – The bot relies on the **Rspamd** spam filtering engine to perform content analysis. Rspamd is a fast C-based spam filter typically used for email; here it’s repurposed for Telegram messages. Each message is formatted as an email and sent to a local Rspamd instance via its HTTP API (default port 11333) for scanning. Rspamd processes the message through its rules (including the custom Telegram-specific Lua rules) and returns a spam **score** and symbols. The bot then uses this result to decide if the message is spam. By using Rspamd, the project benefits from a mature spam detection framework (including text classifiers, regex rules, blacklists, etc.) and can integrate with features like Redis for caching counters. **Redis** is used in this stack as well, as a backing store for counters and reputation data (e.g. counting messages per user, storing rep scores). The combination of Rspamd + Redis allows for efficient tracking of spam metrics across messages and even across restarts.

*(In summary, the stack is: Rust for the Telegram bot logic, Lua within Rspamd for custom spam rules, Rspamd’s scanning service (with its machine-learning and rule engine), and Redis for state storage. This multi-component architecture is packaged to work seamlessly via the bot.)*

## Installation

### Prerequisites

Before installing, make sure you have the following:

* **Rust** – Rust toolchain (stable version). You need Rust and Cargo to build the bot from source. (Alternatively, use the provided Docker setup which includes Rust for building.)
* **Rspamd** – Install Rspamd on your system. The bot expects a local Rspamd instance running (by default on `http://localhost:11333`). Make sure you have Rspamd **2.0+** (or a recent stable version) so that it supports the Lua API and symbols used by this project.
* **Lua** – (Usually comes with Rspamd, no separate Lua installation needed for the bot’s rules. Rspamd includes LuaJIT internally. However, if writing custom rules, familiarity with Lua is helpful.)
* **Redis** – A Redis server (for storing user reputations and counters). By default the bot connects to Redis at `127.0.0.1:6379`. Ensure Redis is installed and running on your host.
* **Telegram Bot Token** – You’ll need a Telegram bot API token from [@BotFather](https://t.me/BotFather). If you haven’t created a bot yet, talk to BotFather to generate a token. (Also, during setup with BotFather, consider **disabling the privacy mode** for your bot, so it can see all messages in group chats. See **Usage** below for why this is important.)
* **Permissions in Telegram** – The bot must be added to your Telegram group chats and made an **administrator** with the ability to read and delete messages. This allows it to monitor all messages and remove spam. (You can add the bot to a group and then promote it to admin status.)

### Setup (from Source)

Follow these steps to build and run the bot from source:

1. **Get the bot code** – Clone the repository from GitHub and enter the project directory:

   ```bash
   git clone https://github.com/akey098/rspamd-telegram-bot.git
   cd rspamd-telegram-bot
   ```
2. **Build the bot** – Use Cargo to compile the Rust project:

   ```bash
   cargo build --release
   ```

   This will produce the executable (in `target/release/`) named **`rspamd-telegram-bot`**.
3. **Install and configure Rspamd** – Ensure you have Rspamd running locally. Copy the provided Lua configuration file for the bot into your Rspamd configuration:

    * File: `rspamd-config/lua.local.d/telegram.lua` (from this repository) contains the Lua rules for Telegram spam detection.
    * Copy this file into Rspamd’s local configuration directory (usually `/etc/rspamd/local.d/` or specifically `/etc/rspamd/lua.local.d/` on Linux systems). You may need to create the `lua.local.d` directory if it doesn’t exist. For example:

      ```bash
      sudo mkdir -p /etc/rspamd/lua.local.d
      sudo cp rspamd-config/lua.local.d/telegram.lua /etc/rspamd/lua.local.d/
      ```
    * After copying, restart the Rspamd service to load the new rules:

      ```bash
      sudo service rspamd restart   # or use systemctl restart rspamd
      ```
    * *(The Docker setup in the next section automates this configuration, but for manual installation you must place the Lua file yourself.)*
4. **Run Redis** – Make sure Redis server is running on localhost. If it’s not already running, you can start it (on many systems, `redis-server` or using your OS service management). The bot will attempt to connect to `redis://127.0.0.1:6379` by default.
5. **Set the Telegram token** – Export your Telegram Bot API token as an environment variable so the bot can use it. The bot uses Teloxide’s convention of reading the token from the `TELOXIDE_TOKEN` env variable. For example, in your shell:

   ```bash
   export TELOXIDE_TOKEN="123456:ABC-DEF_yourTelegramBotToken"
   ```

   (Replace the value with your actual token string from BotFather.)
6. **Launch the bot** – Now run the compiled bot program:

   ```bash
   ./target/release/rspamd-telegram-bot
   ```

   The bot will start up, connect to Telegram, and begin listening for messages. You should see logs indicating it’s running (e.g. “Starting the spam detection bot...”).
7. **Invite the bot to chats** – Add your bot to the desired Telegram group chats. For each group, promote the bot to **administrator**. It needs at least the *Delete Messages* permission (and generally, give it permission to read all messages; if your bot’s privacy mode is enabled, turn it off via BotFather so the bot can actually see messages that are not commands).

That’s it! The bot should now be active in the group, scanning messages for spam. You might want to test it by sending a known spammy message (like repeated text) to see if it reacts (be mindful if testing in a real group).

### Using Docker (All-in-One Container)

For convenience, the project provides a Docker setup that bundles everything (the bot, Rspamd, and Redis) into a single container. This is an easy way to get started without manually installing dependencies. You can use Docker to build and run the bot as follows:

1. **Build the Docker image** (from the project root directory, where the `Dockerfile` is located):

   ```bash
   docker build -t rspamd-telegram-bot .
   ```

   This Docker build will compile the Rust bot and set up a runtime environment with Rspamd and Redis on a slim Debian base.
2. **Run the container**:

   ```bash
   docker run --rm -e TELOXIDE_TOKEN="123456:ABC-DEF_yourTelegramBotToken" rspamd-telegram-bot
   ```

   Replace the `TELOXIDE_TOKEN` value with your actual bot token. The container will start Redis, start Rspamd (with the `telegram.lua` rules automatically placed in its config), and then launch the Telegram bot. Ports 11333 (Rspamd HTTP) and 6379 (Redis) are exposed, though you typically don’t need to access them from outside the container.

With the Docker approach, your bot should be up and running immediately. You’d still need to add the bot to your Telegram groups as described in step 7 above. The advantage of Docker is that it isolates all components; you don’t have to install Rspamd/Redis on your host. This is great for testing or deployment in a containerized environment.

*Note:* If you stop and remove the container, you may lose the Redis-stored data (like user reputation scores) unless you persist it. In a production setup, consider mounting a volume for Redis data or using an external Redis instance to preserve state across restarts.

## Usage

Once the bot is running and added to a group chat, it will automatically begin monitoring messages in that chat (assuming it has admin rights and privacy mode off). Here’s what you can expect and how to interact with the bot:

* **Automatic Spam Monitoring:** The bot scans every message using Rspamd’s filters. This happens in the background without any user commands. If a message is identified as spammy, the bot will take action:

    * **Spam deletion:** For messages that exceed the spam threshold (for example, mass advertising, repeated text, or known malicious links), the bot **deletes** the message from the chat immediately. It will then log a notification about this action. For instance, the bot might send a message to the admin chat (or the group, if no admin chat is set) such as:
      *“Deleting message 42 from user 123456 in chat **MyGroup** for spam.”*
      This lets moderators know a spam message was removed (including which message ID and which user). The spam content is gone from the group, minimizing disruption.
    * **Spam warning:** If a message appears somewhat suspicious but not blatantly malicious (e.g. it trips some filters but isn’t conclusively spam), the bot can issue a **warning** instead of immediate deletion. In the group or admin chat, you might see a warning like:
      *“Warning: message 43 from user 123456 looks like spam.”*
      This warns the user and alerts admins, giving humans a chance to review the content. The message itself is still in the chat (not deleted) when only a warning is issued. This two-level response (warn vs delete) helps avoid false positives being too destructive, while still notifying everyone of potential spam.

* **User Reputation Effects:** The more spam a user sends, the higher their internal “spam score” (reputation) becomes. If a user keeps spamming after warnings, the system will quickly ramp up to deletion for any new messages from that user. In fact, once a user’s reputation crosses a certain threshold, the bot’s Lua rules mark all their messages with a **high spam score** automatically (they are considered *“suspicious”* users). At that point, even a normal-looking message from that user could be auto-removed as spam because their reputation precedes them. Conversely, if a user stops sending spam, their reputation score will tick back down over time (approximately reducing by 1 point each hour by default). This means a user who inadvertently tripped the filter can return to good standing if they behave for a while, whereas a persistent spammer will continue to be flagged. This dynamic is entirely automatic – as an admin you don’t have to manage it, but you can check reputation scores manually (see below).

* **Admin Commands and Control:** The bot offers a set of **commands** for administrators to configure and query the system. These commands are issued as standard Telegram messages (usually in the bot’s **private chat** or in a group where the bot is present). For security, most admin commands require the user to be a chat admin or the bot’s owner. Here are the key commands:

    * **`/make_admin`** – Run this command in a private chat with the bot (or in a group that you want to use as an admin hub). It registers the current chat as an **admin control chat** for the bot. This is typically done in a one-on-one chat between you (the admin) and the bot, turning that chat into your control panel. After you send `/make_admin`, the bot will respond with **“Admin chat registered! Please select chats to moderate:”** and present an inline keyboard listing the titles of Telegram groups where the bot is a member. Simply tap the button for each group you want this admin chat to manage. When you click a group name, the bot will link that group to your admin chat and confirm with **“Chat assigned for moderation!”**. From then on, any spam alerts or warnings from that group will be relayed to your admin chat (instead of cluttering the group). You can use one admin chat to oversee multiple groups.
    * **`/help`** – Displays a list of available commands and a brief description of each. For example, it will list commands like `/make_admin`, `/stats`, etc., along with what they do. This is useful to see all bot capabilities at a glance (especially as new features might be added).
    * **`/stats`** – Shows overall statistics collected by the bot. This might include information such as the number of messages scanned, how many were flagged or deleted as spam, how many warnings issued, etc. (This feature is under active development – in future updates it will provide detailed metrics to help you evaluate how effective the filter is and see spam trends in your chats.)
    * **`/reputation <user_id>`** – Query the reputation score of a particular user. You can use the numeric Telegram user ID (as the bot sees internally) or possibly the @username (depending on implementation, user\_id is safest). The bot will reply with the user’s current reputation points (for example, *“Reputation for user 123456: 5”*). A higher number means the user has sent spam frequently. This command is handy if you want to manually check on a specific member’s status. *(In the future, this might be extended to allow using a reply or mention instead of numeric ID for convenience.)*
    * **`/recent`** – Retrieves a list of recent messages that were flagged as spam by the bot. This allows admins to review what content was caught and removed. For instance, the bot might show the last few spam messages (perhaps with snippets or IDs) so you can verify there were no mistakes or get context on spam attacks. This feature is planned to include message details and timestamps for transparency.
    * **`/addregex <pattern>`** – Adds a custom regex pattern to the spam detection rules on-the-fly. This command is intended for advanced use: if you notice a particular spam phrase or URL frequently appearing, an admin could add a regex via this command to immediately start flagging messages that match it. The bot would incorporate this pattern into its Lua rule set (or Redis store) so that future messages containing the pattern are caught. *(This feature is in development; ultimately it will enable quick custom rule updates without restarting the bot.)*

  **Using the Commands:** To use these commands, you can either message the bot directly (in a private chat, which is recommended for admin commands like `/make_admin` to avoid revealing admin actions in a public group), or if in a group, you might need to prefix the command with the bot’s username (e.g., `/stats@YourBotUsername`) depending on whether the group allows bot commands without mention. Remember that for the bot to recognize you as an admin in a group context, you must be an administrator of that group. The bot checks the sender’s status for certain commands to ensure only authorized people can use them. In a private one-on-one chat with the bot, all commands are accepted (since by default, if you’re talking directly, you’re assumed to be an allowed user, especially after using `/make_admin`).

* **Moderator Notifications:** Once you have an admin control chat set (via `/make_admin`), the bot will use that chat to send you moderation events. For example, if spam is detected in one of your monitored groups and a message gets deleted, you will see a notification in the admin chat like:
  *“Deleting message 87 from user 99887766 in chat **SalesGroup** for spam.”*
  This message tells you which group had spam, which user (ID) was responsible, and which message was removed. If an admin chat is **not** configured, the bot will post these notifications in the same group where the spam occurred (visible to everyone). Configuring an admin chat is recommended to keep the group chat clean and only inform the moderators.

**Note:** After setting up, always double-check that the bot has the right permissions in your groups:

* The bot should have **Admin** rights with *Delete Messages* and *Ban Users* (optional, for future capabilities like auto-banning repeat offenders) permissions. Without the Delete permission, the bot will be unable to remove spam messages.
* It’s also important to disable the bot’s privacy mode via BotFather (send `/setprivacy` to BotFather and choose “Disable” for your bot). Privacy mode off allows the bot to receive all messages in a group, not just those that start with a `/` command or mention. Since spam messages are usually not commands, privacy mode must be off for the bot to see them. If privacy mode is on, the bot will **not** see spam messages and therefore cannot filter them.

With the bot properly configured, it runs autonomously – you can let it work in the background to keep your chats clean. As an admin, you’ll occasionally interact via the provided commands to check on things or adjust settings, but the day-to-day spam fighting is automatic.

## Project Status

**Status:** This project is currently under active development as part of a Google Summer of Code (GSoC) initiative. It is a work in progress, meaning that while the core functionality (Telegram integration, message scanning, spam deletion, basic admin commands, etc.) is up and running, there are still features being refined and added. For example, some admin commands like viewing detailed stats or adding custom rules are in early stages (placeholders exist in the code for these features). The project is evolving rapidly: expect frequent updates, improvements in detection accuracy, and new moderation features over time.

Being a GSoC project, “Full Telegram Support for Spam Filtering” is developed in the open with mentorship and community feedback. **Contributions and feedback are welcome** – if you encounter issues or have ideas for enhancements, you can open issues or pull requests on the GitHub repository. The goal is to polish this bot to a production-ready state by the end of the GSoC period, and potentially merge it or integrate it with the wider Rspamd ecosystem. Keep an eye on the repository for updates, and feel free to try it out and get involved!

*(This project was initiated in 2023 as part of GSoC, under the Rspamd organization, and continues to be improved beyond the summer program.)*

## License

This project is licensed under the **Apache License 2.0**. You are free to use, modify, and distribute the code under the terms of this license. See the [LICENSE](./LICENSE) file for the full license text.

## Contact and Credits

**Author:** Alpatskaia Elizaveta – *Project developer (GSoC Student)*.

* GitHub: [@akey098](https://github.com/akey098)

**Mentors:** Andrew Lewis and Anton Yuzhaninov – *Project mentors and advisors*. Many thanks to them for guidance and support throughout the development of this bot.

**Acknowledgments:** This project is carried out with the support of the Rspamd community. Special thanks to the Rspamd team for the powerful spam filtering infrastructure that made this integration possible, and to Google Summer of Code for providing the opportunity to develop this project.
