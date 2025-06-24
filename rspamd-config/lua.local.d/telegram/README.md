# Telegram Rspamd Rules - Modular Structure

This directory contains a modular implementation of Telegram bot spam detection rules for Rspamd. The rules are organized by functional areas for better maintainability and extensibility.

## Structure

```
telegram/
├── init.lua          # Main entry point and shared settings
├── utils.lua         # Shared utilities and Redis helpers
├── core.lua          # Core user tracking and reputation system
├── content.lua       # Content-based spam detection
├── timing.lua        # Timing-based heuristics
├── lists.lua         # Whitelist/blacklist functionality
├── heuristics.lua    # Advanced spam detection patterns
└── README.md         # This file
```

## Modules

### Core (`core.lua`)
Handles the fundamental user tracking and reputation system:
- **TG_FLOOD**: Detect message flooding
- **TG_REPEAT**: Detect repeated messages
- **TG_SUSPICIOUS**: Detect suspicious activity patterns
- **TG_BAN**: Temporary ban system
- **TG_PERM_BAN**: Permanent ban system

### Content (`content.lua`)
Content-based spam detection rules:
- **TG_LINK_SPAM**: Excessive URLs in messages
- **TG_MENTIONS**: Mass user mentions
- **TG_CAPS**: Excessive capitalization
- **TG_EMOJI_SPAM**: Excessive emoji usage

### Timing (`timing.lua`)
Timing-based heuristics:
- **TG_FIRST_FAST**: First message immediately after join
- **TG_FIRST_SLOW**: First message long after join
- **TG_SILENT**: Long periods of user silence

### Lists (`lists.lua`)
Whitelist and blacklist functionality:
- **WHITELIST_USER**: User whitelist checking
- **BLACKLIST_USER**: User blacklist checking
- **WHITELIST_WORD**: Word whitelist checking
- **BLACKLIST_WORD**: Word blacklist checking

### Heuristics (`heuristics.lua`)
Advanced spam detection patterns:
- **TG_INVITE_LINK**: Telegram invite links
- **TG_PHONE_SPAM**: Phone number patterns
- **TG_SPAM_CHAT**: Spam chat links
- **TG_SHORTENER**: URL shortener detection
- **TG_GIBBERISH**: Gibberish text patterns

## Configuration

### Settings
All settings are centralized in `init.lua` and exported globally as `_G.telegram_settings`. This includes:
- Core thresholds (flood, repeated, suspicious, ban limits)
- Content thresholds (link spam, mentions, caps ratio, emoji limits)
- Timing heuristics (join timing, silence periods)
- Pattern matching (invite links, phone regex, shorteners)

### Symbol Groups
Symbols are organized into functional groups in `groups.conf`:
- `telegram_core`: Core user tracking symbols
- `telegram_content`: Content-based detection symbols
- `telegram_timing`: Timing-based heuristics symbols
- `telegram_lists`: Whitelist/blacklist symbols
- `telegram_heuristics`: Advanced pattern detection symbols

### Redis Configuration
The modular structure uses separate Redis connections:
- `telegram`: For core, content, timing, and heuristics modules
- `whiteblacklist`: For whitelist/blacklist functionality

## Usage

### Loading the Modular Rules
To use the modular structure, load `telegram_modular.lua` instead of the monolithic `telegram.lua`:

```lua
-- In your Rspamd configuration
require "lua.local.d.telegram_modular"
```

### Feature Flags
The system supports per-chat feature flags stored in Redis:
- `flood`: Enable flood detection
- `repeat`: Enable repeated message detection
- `whitelist`: Enable whitelist checking
- `blacklist`: Enable blacklist checking

### Adding New Rules
To add new rules:

1. **Choose the appropriate module** based on the rule's functionality
2. **Add the rule function** following the existing pattern
3. **Register the symbol** with the appropriate group
4. **Add the symbol definition** to `groups.conf` with score and description

### Example: Adding a New Content Rule
```lua
-- In content.lua
local function tg_new_rule_cb(task)
    local text = utils.get_message_text(task)
    if text:match("spam_pattern") then
        task:insert_result('TG_NEW_RULE', 2.0)
    end
end

rspamd_config.TG_NEW_RULE = {
    callback = tg_new_rule_cb,
    description = 'New spam detection rule',
    group = 'telegram_content'
}
```

```conf
# In groups.conf
group "telegram_content" {
    symbol "TG_NEW_RULE" {
        description = "New spam detection rule";
        score = 2.0;
    }
}
```

## Benefits of Modular Structure

1. **Maintainability**: Rules are organized by functional area
2. **Extensibility**: Easy to add new rules to appropriate modules
3. **Testability**: Individual modules can be tested in isolation
4. **Configuration**: Centralized settings and symbol definitions
5. **Documentation**: Clear separation of concerns with documentation

## Migration from Monolithic Structure

The monolithic `telegram.lua` file has been refactored into this modular structure. The functionality remains the same, but the code is now better organized and more maintainable.

To migrate:
1. Replace `require "lua.local.d.telegram"` with `require "lua.local.d.telegram_modular"`
2. Update any custom configurations to use the new group names
3. Test thoroughly to ensure all rules are working as expected

## Notes

- All scores are now defined in `groups.conf` rather than in Lua files
- The modular structure follows Rspamd best practices for rule organization
- UTF-8 pattern support for mixed scripts and Zalgo text is disabled until robust support is available
- Redis connections are shared within modules for efficiency 