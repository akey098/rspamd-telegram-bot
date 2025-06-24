--[[
  Whitelist/Blacklist Telegram Rules
  
  This module handles user and word-based whitelisting/blacklisting:
  - User whitelist/blacklist
  - Word whitelist/blacklist
]]

local utils = require "lua.local.d.telegram.utils"

-- Initialize Redis connection
if not utils.init_redis('whiteblacklist') then
    return
end

local redis_params = utils.get_redis_params()

-- List settings
local list_settings = {
    whitelist_users_key = 'tg:whitelist:users',
    blacklist_users_key = 'tg:blacklist:users',
    whitelist_words_key = 'tg:whitelist:words',
    blacklist_words_key = 'tg:blacklist:words',
    user_prefix = 'tg:users:'
}

-- WHITELIST_USER: Check if user is whitelisted
local function whitelist_user_cb(task)
    local user_id, chat_id = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = list_settings.user_prefix .. user_id
    
    utils.if_feature_enabled(task, chat_id, 'whitelist', function()
        local function whitelist_cb(err, data)
            if err then
                utils.log_error(task, 'whitelist_cb', err)
                return
            end
            if data then
                task:insert_result('WHITELIST_USER', 1.0)
            end
        end
        
        lua_redis.redis_make_request(task,
            redis_params,
            list_settings.whitelist_users_key,
            false, -- is write
            whitelist_cb,
            'SISMEMBER',
            {list_settings.whitelist_users_key, user_key}
        )
    end)
end

-- BLACKLIST_USER: Check if user is blacklisted
local function blacklist_user_cb(task)
    local user_id, chat_id = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = list_settings.user_prefix .. user_id
    
    utils.if_feature_enabled(task, chat_id, 'blacklist', function()
        local function blacklist_cb(err, data)
            if err then
                utils.log_error(task, 'blacklist_cb', err)
                return
            end
            if data then
                task:insert_result('BLACKLIST_USER', 1.0)
            end
        end
        
        lua_redis.redis_make_request(task,
            redis_params,
            list_settings.blacklist_users_key,
            false, -- is write
            blacklist_cb,
            'SISMEMBER',
            {list_settings.blacklist_users_key, user_key}
        )
    end)
end

-- WHITELIST_WORD: Check for whitelisted words
local function whitelist_word_cb(task)
    local user_id, chat_id = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    utils.if_feature_enabled(task, chat_id, 'whitelist', function()
        local msg = utils.get_message_text(task)
        local words = utils.break_to_words(msg)
        local count = 0
        
        local function if_member_cb(err, data)
            if err then
                utils.log_error(task, 'if_member_cb', err)
                return
            end
            if data then
                count = count + 1
            end
        end
        
        for _, word in ipairs(words) do
            lua_redis.redis_make_request(task,
                redis_params,
                list_settings.whitelist_words_key,
                false, -- is write
                if_member_cb,
                'SISMEMBER',
                {list_settings.whitelist_words_key, word}
            )
        end
        
        task:insert_result('WHITELIST_WORD', count)
    end)
end

-- BLACKLIST_WORD: Check for blacklisted words
local function blacklist_word_cb(task)
    local user_id, chat_id = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    utils.if_feature_enabled(task, chat_id, 'blacklist', function()
        local msg = utils.get_message_text(task)
        local words = utils.break_to_words(msg)
        local count = 0
        
        local function if_member_cb(err, data)
            if err then
                utils.log_error(task, 'if_member_cb', err)
                return
            end
            if data then
                count = count + 1
            end
        end
        
        for _, word in ipairs(words) do
            lua_redis.redis_make_request(task,
                redis_params,
                list_settings.blacklist_words_key,
                false, -- is write
                if_member_cb,
                'SISMEMBER',
                {list_settings.blacklist_words_key, word}
            )
        end
        
        task:insert_result('BLACKLIST_WORD', count)
    end)
end

-- Register list-based symbols (scores defined in groups.conf)
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