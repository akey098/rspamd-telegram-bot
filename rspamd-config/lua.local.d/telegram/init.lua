--[[
  Telegram Bot Rspamd Rules - Modular Structure - DISABLED
  
  This module loads all telegram-related rules in a modular fashion.
  Each submodule handles a specific functional area:
  
  - core.lua: Core user tracking and reputation system
  - content.lua: Content-based spam detection (links, mentions, caps)
  - timing.lua: Timing-based heuristics (join timing, silence)
  - lists.lua: Whitelist/blacklist functionality
  - heuristics.lua: Advanced spam detection patterns
  
  DISABLED: This modular system has been disabled in favor of telegram_simple.lua
  to prevent duplicate symbol registration.
]]

-- DISABLED: All modular loading commented out to prevent conflicts
--[[
-- Shared settings across all modules
local settings = {
    -- Core settings
    flood = 30,
    repeated = 6,
    suspicious = 10,
    ban = 20,
    user_prefix = 'tg:users:',
    chat_prefix = 'tg:chats:',
    exp_flood = '60',
    exp_ban = '3600',
    banned_q = 3,
    
    -- Content thresholds
    link_spam = 3,
    mentions = 5,
    caps_ratio = 0.7,
    emoji_limit = 10,
    
    -- Timing heuristics (seconds)
    join_fast  = 10,          -- first message within 10 s of join → spammy
    join_slow  = 86400,       -- first message after 24 h of join → suspicious bot
    silence    = 2592000,     -- 30 days without message → dormant bot
    
    -- Pattern matching
    invite_link_patterns = {'t.me/joinchat', 't.me/+', 'telegram.me/joinchat'},
    phone_regex = '%+?%d[%d%-%s%(%)]%d%d%d%d',
    spam_chat_regex = 't.me/joinchat',
    shorteners = {'bit%.ly', 't%.co', 'goo%.gl', 'tinyurl%.com', 'is%.gd', 'ow%.ly'},
    
    -- Feature flags
    features_key = 'tg:enabled_features'
}

-- Export settings globally for other modules
_G.telegram_settings = settings

-- Load all submodules using dofile for proper path resolution
dofile("/etc/rspamd/lua/telegram/utils.lua")
dofile("/etc/rspamd/lua/telegram/core.lua")
dofile("/etc/rspamd/lua/telegram/content.lua")
dofile("/etc/rspamd/lua/telegram/timing.lua")
dofile("/etc/rspamd/lua/telegram/lists.lua")
dofile("/etc/rspamd/lua/telegram/heuristics.lua")

-- Export common utilities for other modules
local M = {}

-- Shared Redis utilities
M.redis_utils = require "telegram.utils"

return M
]]-- 