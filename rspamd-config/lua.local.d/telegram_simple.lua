--[[
  Simple Telegram Bot Rspamd Rules
  
  This is a simplified version that loads all telegram rules directly
  without complex module dependencies.
]]

-- Load lua_redis module
local lua_redis = require "lua_redis"
if not lua_redis then
    return
end

-- Load logger for debugging
local rspamd_logger = require "rspamd_logger"

-- Shared settings
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
    join_fast  = 10,
    join_slow  = 86400,
    silence    = 2592000,
    
    -- Pattern matching
    invite_link_patterns = {'t.me/joinchat', 't.me/+', 'telegram.me/joinchat'},
    phone_regex = '%+?%d[%d%-%s%(%)]%d%d%d%d',
    spam_chat_regex = 't.me/joinchat',
    shorteners = {'bit%.ly', 't%.co', 'goo%.gl', 'tinyurl%.com', 'is%.gd', 'ow%.ly'},
}

-- Initialize Redis connection
local redis_params = lua_redis.parse_redis_server('telegram')
if not redis_params then
    rspamd_logger.errx(nil, 'Failed to parse Redis server for telegram module')
    return
end

-- Utility functions
local function safe_str(val)
    return tostring(val or "")
end

local function safe_num(val, default)
    return tonumber(val) or (default or 0)
end

local function get_user_chat_ids(task)
    local user_id = safe_str(task:get_header('X-Telegram-User', true))
    local chat_id = safe_str(task:get_header('X-Telegram-Chat', true))
    return user_id, chat_id
end

local function get_message_text(task)
    return safe_str(task:get_rawbody())
end

-- TG_FLOOD: Detect message flooding
local function tg_flood_cb(task)
    local user_id, chat_id = get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    local function flood_cb(err, data)
        if err then 
            rspamd_logger.errx(task, 'flood_cb error: %1', err)
            return 
        end
        
        local count = safe_num(data)
        lua_redis.redis_make_request(task,
            redis_params,
            user_key,
            true, -- is write
            function() end,
            'HEXPIRE',
            {user_key, settings.exp_flood, 'NX', 'FIELDS', 1, 'flood'}
        )
        
        if count > settings.flood then
            local chat_key = settings.chat_prefix .. chat_id
            lua_redis.redis_make_request(task,
                redis_params,
                chat_key,
                true, -- is write
                function() end,
                'HINCRBY',
                {chat_key, 'spam_count', '1'}
            )
            lua_redis.redis_make_request(task,
                redis_params,
                user_key,
                true, -- is write
                function() end,
                'HINCRBY',
                {user_key, 'rep', '1'}
            )
            task:insert_result('TG_FLOOD')
            rspamd_logger.infox(task, 'TG_FLOOD triggered for user %1, count: %2', user_id, count)
        end
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        true, -- is write
        flood_cb,
        'HINCRBY',
        {user_key, 'flood', 1}
    )
end

-- TG_LINK_SPAM: Detect excessive URLs
local function tg_link_spam_cb(task)
    local _, chat_id = get_user_chat_ids(task)
    if chat_id == "" then return end

    local urls = task:get_urls() or {}
    if #urls >= settings.link_spam then
        task:insert_result('TG_LINK_SPAM')
        rspamd_logger.infox(task, 'TG_LINK_SPAM triggered, URLs: %1', #urls)
    end
end

-- TG_MENTIONS: Detect excessive user mentions
local function tg_mentions_cb(task)
    local text = get_message_text(task)
    
    local n = 0
    for _ in text:gmatch("@[%w_]+") do 
        n = n + 1 
    end
    
    if n >= settings.mentions then
        task:insert_result('TG_MENTIONS')
        rspamd_logger.infox(task, 'TG_MENTIONS triggered, mentions: %1', n)
    end
end

-- TG_CAPS: Detect excessive capital letters
local function tg_caps_cb(task)
    local text = get_message_text(task)
    
    rspamd_logger.infox(task, 'TG_CAPS: Processing message, length: %1', #text)
    
    if #text < 20 then 
        rspamd_logger.infox(task, 'TG_CAPS: Message too short, skipping')
        return 
    end

    local letters, caps = 0, 0
    for ch in text:gmatch("%a") do
        letters = letters + 1
        if ch:match("%u") then 
            caps = caps + 1 
        end
    end
    
    rspamd_logger.infox(task, 'TG_CAPS: Letters: %1, Caps: %2, Ratio: %3', letters, caps, letters > 0 and (caps/letters) or 0)
    
    if letters > 0 and (caps / letters) >= settings.caps_ratio then
        task:insert_result('TG_CAPS', 1.5)
        rspamd_logger.infox(task, 'TG_CAPS triggered, caps ratio: %1', caps/letters)
    else
        rspamd_logger.infox(task, 'TG_CAPS: Not triggered, ratio %1 < threshold %2', caps/letters, settings.caps_ratio)
    end
end

-- Register symbols
rspamd_config.TG_FLOOD = {
    callback = tg_flood_cb,
    score = 1.2,
    description = 'User is flooding',
    group = 'telegram_core'
}

rspamd_config.TG_LINK_SPAM = {
    callback = tg_link_spam_cb,
    score = 2.5,
    description = 'Message contains excessive number of links',
    group = 'telegram_content'
}

rspamd_config.TG_MENTIONS = {
    callback = tg_mentions_cb,
    score = 2.5,
    description = 'Message mentions too many users',
    group = 'telegram_content'
}

rspamd_config.TG_CAPS = {
    callback = tg_caps_cb,
    score = 1.5,
    description = 'Message is written almost entirely in capital letters',
    group = 'telegram_content'
}

-- TEST_RULE: Simple test rule to verify Lua loading
rspamd_config.TEST_RULE = {
    callback = function(task)
        rspamd_logger.infox(task, 'TEST_RULE: This is a test rule - Lua modules are loaded!')
        task:insert_result('TEST_RULE', 1.0)
    end,
    score = 1.0,
    description = 'Test rule to verify Lua loading',
    group = 'telegram_content'
}

-- Log that symbols are registered
rspamd_logger.infox(rspamd_config, 'Telegram symbols registered: TG_FLOOD, TG_LINK_SPAM, TG_MENTIONS, TG_CAPS, TEST_RULE') 