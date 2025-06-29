--[[
  Advanced Heuristics Telegram Rules
  
  This module handles advanced spam detection patterns:
  - Telegram invite links
  - Phone number spam
  - URL shorteners
  - Gibberish text patterns
]]

local utils = require "telegram.utils"
local settings = _G.telegram_settings

-- Initialize Redis connection
if not utils.init_redis('telegram') then
    return
end

local redis_params = utils.get_redis_params()

-- TG_INVITE_LINK: Detect Telegram invite links
local function tg_invite_link_cb(task)
    local text = utils.get_message_text(task)
    
    for _, pattern in ipairs(settings.invite_link_patterns) do
        if text:lower():find(pattern, 1, true) then
            task:insert_result('TG_INVITE_LINK', 4.0)
            break
        end
    end
end

-- TG_PHONE_SPAM: Detect phone number patterns (promo spam)
local function tg_phone_spam_cb(task)
    local text = utils.get_message_text(task)
    
    if text:match(settings.phone_regex) then
        task:insert_result('TG_PHONE_SPAM', 3.0)
    end
end

-- TG_SPAM_CHAT: Detect spam chat links
local function tg_spam_chat_cb(task)
    local text = utils.get_message_text(task)
    
    if text:match(settings.spam_chat_regex) then
        task:insert_result('TG_SPAM_CHAT', 3.0)
    end
end

-- TG_SHORTENER: Detect URL shortener links
local function tg_shortener_cb(task)
    local text = utils.get_message_text(task)
    
    for _, shortener in ipairs(settings.shorteners) do
        if text:lower():find(shortener) then
            task:insert_result('TG_SHORTENER', 2.0)
            break
        end
    end
end

-- TG_GIBBERISH: Detect long sequences of random consonants
local function tg_gibberish_cb(task)
    local text = utils.get_message_text(task)
    
    -- Pattern for 5+ consecutive consonants
    if text:match('[bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ][bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ][bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ][bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ][bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ]') then
        task:insert_result('TG_GIBBERISH', 2.0)
    end
end

-- Register heuristic symbols (scores defined in groups.conf)
rspamd_config.TG_INVITE_LINK = {
    callback = tg_invite_link_cb,
    description = 'Telegram invite link detected',
    group = 'telegram_heuristics'
}

rspamd_config.TG_PHONE_SPAM = {
    callback = tg_phone_spam_cb,
    description = 'Contains phone number spam',
    group = 'telegram_heuristics'
}

rspamd_config.TG_SPAM_CHAT = {
    callback = tg_spam_chat_cb,
    description = 'Contains spam chat',
    group = 'telegram_heuristics'
}

rspamd_config.TG_SHORTENER = {
    callback = tg_shortener_cb,
    description = 'Contains URL shortener link',
    group = 'telegram_heuristics'
}

rspamd_config.TG_GIBBERISH = {
    callback = tg_gibberish_cb,
    description = 'Gibberish consonant sequences',
    group = 'telegram_heuristics'
}

-- Note: TG_MIXED_SCRIPTS and TG_ZALGO are disabled until robust UTF-8 
-- pattern support is added to avoid syntax issues with Lua 5.1 