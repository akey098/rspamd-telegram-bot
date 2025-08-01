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
    exp_flood = '30',
    exp_ban = '3600',
    banned_q = 3,
    ban_reduction_interval = 172800, -- 48 hours in seconds
    
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

-- TG_REPEAT: Detect repeated messages
local function tg_repeat_cb(task)
    local user_id, chat_id = get_user_chat_ids(task)
    if user_id == "" then return end
    
    rspamd_logger.infox(task, 'TG_REPEAT: Processing message for user %1', user_id)
    
    local user_key = settings.user_prefix .. user_id
    local msg = get_message_text(task)
    
    local function last_msg_cb(err, data)
        if err then 
            rspamd_logger.errx(task, 'last_msg_cb error: %1', err)
            return 
        end
        
        local function get_count_cb(_err, _data)
            if _err then return end
            local count = safe_num(_data)
            rspamd_logger.infox(task, 'TG_REPEAT: Current count for user %1 is %2', user_id, count)
            if count > settings.repeated then
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
                task:insert_result('TG_REPEAT', 2.0)
                rspamd_logger.infox(task, 'TG_REPEAT triggered for user %1, count: %2', user_id, count)
            end
        end
        
        if safe_str(data) == msg then
            rspamd_logger.infox(task, 'TG_REPEAT: Message matches previous for user %1', user_id)
            lua_redis.redis_make_request(task,
                redis_params,
                user_key,
                true, -- is write
                get_count_cb,
                'HINCRBY',
                {user_key, 'eq_msg_count', 1}
            )
        else
            rspamd_logger.infox(task, 'TG_REPEAT: Message is different for user %1', user_id)
            lua_redis.redis_make_request(task,
                redis_params,
                user_key,
                true, -- is write
                function() end,
                'HSET',
                {user_key, 'eq_msg_count', 0}
            )
        end
        
        lua_redis.redis_make_request(task,
            redis_params,
            user_key,
            true, -- is write
            function() end,
            'HSET',
            {user_key, 'last_msg', msg}
        )
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        false, -- is write
        last_msg_cb,
        'HGET',
        {user_key, 'last_msg'}
    )
end

-- TG_SUSPICIOUS: Detect suspicious activity
local function tg_suspicious_cb(task)
    local user_id, chat_id = get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    local function spam_cb(err, data)
        if err then 
            rspamd_logger.errx(task, 'spam_cb error: %1', err)
            return 
        end
        
        local total = safe_num(data)
        if total > settings.suspicious then
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
            task:insert_result('TG_SUSPICIOUS', 5.0)
            rspamd_logger.infox(task, 'TG_SUSPICIOUS triggered for user %1, rep: %2', user_id, total)
        end
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        false, -- is write
        spam_cb,
        'HGET',
        {user_key, 'rep'}
    )
end

-- TG_BAN: Temporary ban system
local function tg_ban_cb(task)
    local user_id, chat_id = get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    local function ban_cb(err, data)
        if err then 
            rspamd_logger.errx(task, 'ban_cb error: %1', err)
            return 
        end
        
        local total = safe_num(data)
        if total > settings.ban then
            local chat_key = settings.chat_prefix .. chat_id
            lua_redis.redis_make_request(task,
                redis_params,
                chat_key,
                true, -- is write
                function() end,
                'HINCRBY',
                {chat_key, 'banned', '1'}
            )
            
            -- Get current ban count
            local function get_banned_q_cb(_err, _data)
                if _err then
                    rspamd_logger.errx(task, 'get_banned_q_cb error: %1', _err)
                    return
                end
                
                local banned_q = safe_num(_data)
                
                -- Increment ban counter
                lua_redis.redis_make_request(task,
                    redis_params,
                    user_key,
                    true, -- is write
                    function() end,
                    'HINCRBY',
                    {user_key, 'banned_q', '1'}
                )
                
                -- Set ban flag with expiration
                local function banned_cb(__err, __data)
                    if __err or not __data then return end
                    lua_redis.redis_make_request(task,
                        redis_params,
                        user_key,
                        true, -- is write
                        function() end,
                        'HEXPIRE',
                        {user_key, settings.exp_ban, 'FIELDS', 1, 'banned'}
                    )
                end
                
                lua_redis.redis_make_request(task,
                    redis_params,
                    user_key,
                    true, -- is write
                    banned_cb,
                    'HSET',
                    {user_key, 'banned', '1'}
                )
                
                -- Reduce reputation
                lua_redis.redis_make_request(task,
                    redis_params,
                    user_key,
                    true, -- is write
                    function() end,
                    'HINCRBY',
                    {user_key, 'rep', '-5'}
                )
                
                -- Set ban reduction time for automatic counter reduction
                local current_time = os.time()
                local reduction_time = current_time + settings.ban_reduction_interval
                lua_redis.redis_make_request(task,
                    redis_params,
                    user_key,
                    true, -- is write
                    function() end,
                    'HSET',
                    {user_key, 'ban_reduction_time', tostring(reduction_time)}
                )
                
                task:insert_result('TG_BAN', 10.0)
                rspamd_logger.infox(task, 'TG_BAN triggered for user %1, rep: %2, ban count: %3', user_id, total, banned_q + 1)
            end
            
            lua_redis.redis_make_request(task,
                redis_params,
                user_key,
                false, -- is write
                get_banned_q_cb,
                'HGET',
                {user_key, 'banned_q'}
            )
        end
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        false, -- is write
        ban_cb,
        'HGET',
        {user_key, 'rep'}
    )
end

-- TG_PERM_BAN: Permanent ban system
local function tg_perm_ban_cb(task)
    local user_id, chat_id = get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    local function perm_ban_cb(err, data)
        if err then 
            rspamd_logger.errx(task, 'perm_ban_cb error: %1', err)
            return 
        end
        
        local banned_q = safe_num(data)
        if banned_q >= 3 then -- Changed from > to >= to trigger on 3rd ban
            local chat_key = settings.chat_prefix .. chat_id
            lua_redis.redis_make_request(task,
                redis_params,
                chat_key,
                true, -- is write
                function() end,
                'HINCRBY',
                {chat_key, 'perm_banned', '1'}
            )
            task:insert_result('TG_PERM_BAN', 15.0)
            rspamd_logger.infox(task, 'TG_PERM_BAN triggered for user %1, banned_q: %2', user_id, banned_q)
        end
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        false, -- is write
        perm_ban_cb,
        'HGET',
        {user_key, 'banned_q'}
    )
end

-- WHITELIST_USER: Check if user is whitelisted
local function whitelist_user_cb(task)
    local user_id, chat_id = get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    local function whitelist_cb(err, data)
        if err then
            rspamd_logger.errx(task, 'whitelist_cb error: %1', err)
            return
        end
        if data then
            task:insert_result('WHITELIST_USER', 1.0)
            rspamd_logger.infox(task, 'WHITELIST_USER triggered for user %1', user_id)
        end
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        'tg:whitelist:users',
        false, -- is write
        whitelist_cb,
        'SISMEMBER',
        {'tg:whitelist:users', user_key}
    )
end

-- BLACKLIST_USER: Check if user is blacklisted
local function blacklist_user_cb(task)
    local user_id, chat_id = get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    local function blacklist_cb(err, data)
        if err then
            rspamd_logger.errx(task, 'blacklist_cb error: %1', err)
            return
        end
        if data then
            task:insert_result('BLACKLIST_USER', 1.0)
            rspamd_logger.infox(task, 'BLACKLIST_USER triggered for user %1', user_id)
        end
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        'tg:blacklist:users',
        false, -- is write
        blacklist_cb,
        'SISMEMBER',
        {'tg:blacklist:users', user_key}
    )
end

-- WHITELIST_WORD: Check for whitelisted words
local function whitelist_word_cb(task)
    local user_id, chat_id = get_user_chat_ids(task)
    if user_id == "" then return end
    
    local msg = get_message_text(task)
    local words = {}
    for word in msg:gmatch("%w+") do
        words[#words + 1] = word
    end
    
    local count = 0
    local function if_member_cb(err, data)
        if err then
            rspamd_logger.errx(task, 'whitelist_word_cb error: %1', err)
            return
        end
        if data then
            count = count + 1
        end
    end
    
    for _, word in ipairs(words) do
        lua_redis.redis_make_request(task,
            redis_params,
            'tg:whitelist:words',
            false, -- is write
            if_member_cb,
            'SISMEMBER',
            {'tg:whitelist:words', word}
        )
    end
    
    if count > 0 then
        task:insert_result('WHITELIST_WORD', count)
        rspamd_logger.infox(task, 'WHITELIST_WORD triggered for user %1, count: %2', user_id, count)
    end
end

-- BLACKLIST_WORD: Check for blacklisted words
local function blacklist_word_cb(task)
    local user_id, chat_id = get_user_chat_ids(task)
    if user_id == "" then return end
    
    local msg = get_message_text(task)
    local words = {}
    for word in msg:gmatch("%w+") do
        words[#words + 1] = word
    end
    
    local count = 0
    local function if_member_cb(err, data)
        if err then
            rspamd_logger.errx(task, 'blacklist_word_cb error: %1', err)
            return
        end
        if data then
            count = count + 1
        end
    end
    
    for _, word in ipairs(words) do
        lua_redis.redis_make_request(task,
            redis_params,
            'tg:blacklist:words',
            false, -- is write
            if_member_cb,
            'SISMEMBER',
            {'tg:blacklist:words', word}
        )
    end
    
    if count > 0 then
        task:insert_result('BLACKLIST_WORD', count)
        rspamd_logger.infox(task, 'BLACKLIST_WORD triggered for user %1, count: %2', user_id, count)
    end
end

-- TG_EMOJI_SPAM: Detect excessive emoji usage
local function tg_emoji_spam_cb(task)
    local text = get_message_text(task)
    local count = 0
    
    -- Count emoji characters using Unicode ranges
    -- This covers most common emoji ranges
    for _ in text:gmatch('[\240-\244][\128-\191][\128-\191][\128-\191]') do
        count = count + 1
        if count > settings.emoji_limit then
            task:insert_result('TG_EMOJI_SPAM', 2.5)
            rspamd_logger.infox(task, 'TG_EMOJI_SPAM triggered, emoji count: %1', count)
            break
        end
    end
end

-- TG_INVITE_LINK: Detect Telegram invite links
local function tg_invite_link_cb(task)
    local text = get_message_text(task):lower()
    
    for _, pattern in ipairs(settings.invite_link_patterns) do
        if text:find(pattern, 1, true) then
            task:insert_result('TG_INVITE_LINK', 4.0)
            rspamd_logger.infox(task, 'TG_INVITE_LINK triggered, pattern: %1', pattern)
            break
        end
    end
end

-- TG_PHONE_SPAM: Detect phone number patterns (promo spam)
local function tg_phone_spam_cb(task)
    local text = get_message_text(task)
    
    if text:match(settings.phone_regex) then
        task:insert_result('TG_PHONE_SPAM', 3.0)
        rspamd_logger.infox(task, 'TG_PHONE_SPAM triggered')
    end
end

-- TG_SHORTENER: Detect URL shortener links
local function tg_shortener_cb(task)
    local text = get_message_text(task):lower()
    
    for _, shortener in ipairs(settings.shorteners) do
        if text:find(shortener) then
            task:insert_result('TG_SHORTENER', 2.0)
            rspamd_logger.infox(task, 'TG_SHORTENER triggered, shortener: %1', shortener)
            break
        end
    end
end

-- TG_GIBBERISH: Detect long sequences of random consonants
local function tg_gibberish_cb(task)
    local text = get_message_text(task)
    
    -- Pattern for 5+ consecutive consonants (indicating gibberish)
    if text:match('[bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ][bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ][bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ][bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ][bcdfghjklmnpqrstvwxzBCDFGHJKLMNPQRSTVWXZ]') then
        task:insert_result('TG_GIBBERISH')
        rspamd_logger.infox(task, 'TG_GIBBERISH triggered')
    end
end

-- Register symbols
rspamd_config.TG_FLOOD = {
    callback = tg_flood_cb,
    score = 1.2,
    description = 'User is flooding',
    group = 'telegram_core'
}

rspamd_config.TG_REPEAT = {
    callback = tg_repeat_cb,
    score = 2.0,
    description = 'User has sent a lot of equal messages',
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

rspamd_config.TG_SUSPICIOUS = {
    callback = tg_suspicious_cb,
    score = 5.0,
    description = 'Suspicious activity',
    group = 'telegram_core'
}

rspamd_config.TG_BAN = {
    callback = tg_ban_cb,
    score = 10.0,
    description = 'Banned for some time',
    group = 'telegram_core'
}

rspamd_config.TG_PERM_BAN = {
    callback = tg_perm_ban_cb,
    score = 15.0,
    description = 'Permanently banned',
    group = 'telegram_core'
}

-- Register whitelist/blacklist symbols
rspamd_config.WHITELIST_USER = {
    callback = whitelist_user_cb,
    description = 'User is in whitelist',
    group = 'telegram_lists'
}

rspamd_config.BLACKLIST_USER = {
    callback = blacklist_user_cb,
    description = 'User is in blacklist',
    group = 'telegram_lists'
}

rspamd_config.WHITELIST_WORD = {
    callback = whitelist_word_cb,
    description = 'Word is in whitelist',
    group = 'telegram_lists'
}

rspamd_config.BLACKLIST_WORD = {
    callback = blacklist_word_cb,
    description = 'Word is in blacklist',
    group = 'telegram_lists'
}

-- Register advanced detection symbols
rspamd_config.TG_EMOJI_SPAM = {
    callback = tg_emoji_spam_cb,
    score = 2.5,
    description = 'Excessive emoji usage',
    group = 'telegram_content'
}

rspamd_config.TG_INVITE_LINK = {
    callback = tg_invite_link_cb,
    score = 4.0,
    description = 'Telegram invite link detected',
    group = 'telegram_heuristics'
}

rspamd_config.TG_PHONE_SPAM = {
    callback = tg_phone_spam_cb,
    score = 3.0,
    description = 'Contains phone number spam',
    group = 'telegram_heuristics'
}

rspamd_config.TG_SHORTENER = {
    callback = tg_shortener_cb,
    score = 2.0,
    description = 'Contains URL shortener link',
    group = 'telegram_heuristics'
}

rspamd_config.TG_GIBBERISH = {
    callback = tg_gibberish_cb,
    score = 2.0,
    description = 'Gibberish consonant sequences',
    group = 'telegram_heuristics'
}

-- Log that symbols are registered
rspamd_logger.infox(rspamd_config, 'Telegram symbols registered: TG_FLOOD, TG_REPEAT, TG_LINK_SPAM, TG_MENTIONS, TG_CAPS, TG_SUSPICIOUS, TG_BAN, TG_PERM_BAN, TG_EMOJI_SPAM, TG_INVITE_LINK, TG_PHONE_SPAM, TG_SHORTENER, TG_GIBBERISH, WHITELIST_USER, BLACKLIST_USER, WHITELIST_WORD, BLACKLIST_WORD') 