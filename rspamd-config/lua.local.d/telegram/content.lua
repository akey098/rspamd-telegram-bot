--[[
  Content-based Telegram Rules
  
  This module handles content-based spam detection:
  - Link spam detection
  - User mention spam
  - Excessive capitalization
]]

local utils = require "telegram.utils"
local settings = _G.telegram_settings

-- Initialize Redis connection
if not utils.init_redis('telegram') then
    return
end

local redis_params = utils.get_redis_params()

-- TG_LINK_SPAM: Detect excessive URLs in a single message
local function tg_link_spam_cb(task)
    local _, chat_id = utils.get_user_chat_ids(task)
    if chat_id == "" then return end

    -- Count URLs in the message
    local urls = task:get_urls() or {}
    if #urls >= settings.link_spam then
        task:insert_result('TG_LINK_SPAM', 2.5)
    end
end

-- TG_MENTIONS: Detect excessive user mentions (mass ping)
local function tg_mentions_cb(task)
    local text = utils.get_message_text(task)
    
    -- Count occurrences of @username patterns
    -- Telegram usernames are 5-32 chars of letters, digits and underscores
    local n = 0
    for _ in text:gmatch("@[%w_]+") do 
        n = n + 1 
    end
    
    if n >= settings.mentions then
        task:insert_result('TG_MENTIONS', 2.5)
    end
end

-- TG_CAPS: Detect excessive capital letters (shouting)
local function tg_caps_cb(task)
    local text = utils.get_message_text(task)
    
    -- Ignore very short messages
    if #text < 20 then return end

    local letters, caps = 0, 0
    for ch in text:gmatch("%a") do
        letters = letters + 1
        if ch:match("%u") then 
            caps = caps + 1 
        end
    end
    
    if letters > 0 and (caps / letters) >= settings.caps_ratio then
        task:insert_result('TG_CAPS', 1.5)
    end
end

-- TG_EMOJI_SPAM: Detect excessive emoji usage
local function tg_emoji_spam_cb(task)
    local text = utils.get_message_text(task)
    local count = 0
    
    -- Count emoji characters (simplified Unicode ranges)
    for _ in text:gmatch('[600-64f300-5ff680-6ff1e0-1ff]') do
        count = count + 1
        if count > settings.emoji_limit then
            task:insert_result('TG_EMOJI_SPAM', 2.5)
            break
        end
    end
end

-- Register content-based symbols (scores defined in groups.conf)
rspamd_config.TG_LINK_SPAM = {
    callback = tg_link_spam_cb,
    description = 'Message contains excessive number of links',
    group = 'telegram_content'
}

rspamd_config.TG_MENTIONS = {
    callback = tg_mentions_cb,
    description = 'Message mentions too many users',
    group = 'telegram_content'
}

rspamd_config.TG_CAPS = {
    callback = tg_caps_cb,
    description = 'Message is written almost entirely in capital letters',
    group = 'telegram_content'
}

rspamd_config.TG_EMOJI_SPAM = {
    callback = tg_emoji_spam_cb,
    description = 'Excessive emoji usage',
    group = 'telegram_content'
} 