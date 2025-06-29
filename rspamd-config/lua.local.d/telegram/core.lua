--[[
  Core Telegram Rules - User Tracking and Reputation System
  
  This module handles the core user tracking functionality:
  - Flood detection
  - Repeated message detection  
  - Suspicious activity tracking
  - Ban system (temporary and permanent)
]]

local utils = require "telegram.utils"
local settings = _G.telegram_settings

-- Initialize Redis connection
if not utils.init_redis('telegram') then
    return
end

local redis_params = utils.get_redis_params()

-- TG_FLOOD: Detect message flooding
local function tg_flood_cb(task)
    local user_id, chat_id = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    utils.if_feature_enabled(task, chat_id, 'flood', function()
        local function flood_cb(err, data)
            if err then
                utils.log_error(task, 'flood_cb', err)
                return
            end
            
            local count = utils.safe_num(data)
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
                task:insert_result('TG_FLOOD', 1.2)
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
    end)
end

-- TG_REPEAT: Detect repeated messages
local function tg_repeat_cb(task)
    local user_id, chat_id = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    utils.if_feature_enabled(task, chat_id, 'repeat', function()
        local msg = utils.get_message_text(task)
        
        local function last_msg_cb(err, data)
            if err then
                utils.log_error(task, 'last_msg_cb', err)
                return
            end
            
            local function get_count_cb(_err, _data)
                if _err then return end
                local count = utils.safe_num(_data)
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
                end
            end
            
            if utils.safe_str(data) == msg then
                lua_redis.redis_make_request(task,
                    redis_params,
                    user_key,
                    true, -- is write
                    get_count_cb,
                    'HINCRBY',
                    {user_key, 'eq_msg_count', 1}
                )
            else
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
    end)
end

-- TG_SUSPICIOUS: Detect suspicious activity
local function tg_suspicious_cb(task)
    local user_id, chat_id = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    local function spam_cb(err, data)
        if err then
            utils.log_error(task, 'spam_cb', err)
            return
        end
        
        local total = utils.safe_num(data)
        if total > settings.suspicious then
            local chat_stats = settings.chat_prefix .. chat_id
            lua_redis.redis_make_request(task,
                redis_params,
                chat_stats,
                true, -- is write
                function() end,
                'HINCRBY',
                {chat_stats, 'spam_count', '1'}
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
    local user_id, chat_id = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    local function ban_cb(err, data)
        if err then
            utils.log_error(task, 'ban_cb', err)
            return
        end
        
        local total = utils.safe_num(data)
        if total > settings.ban then
            local chat_stats = settings.chat_prefix .. chat_id
            lua_redis.redis_make_request(task,
                redis_params,
                chat_stats,
                true, -- is write
                function() end,
                'HINCRBY',
                {chat_stats, 'banned', '1'}
            )
            
            local function banned_cb(_err, _data)
                if _err or not _data then return end
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
            lua_redis.redis_make_request(task,
                redis_params,
                user_key,
                true, -- is write
                function() end,
                'HINCRBY',
                {user_key, 'rep', '-5'}
            )
            lua_redis.redis_make_request(task,
                redis_params,
                user_key,
                true, -- is write
                function() end,
                'HINCRBY',
                {user_key, 'banned_q', '1'}
            )
            task:insert_result('TG_BAN', 10.0)
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
    local user_id, chat_id = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    
    local function perm_ban_cb(err, data)
        if err then
            utils.log_error(task, 'perm_ban_cb', err)
            return
        end
        
        local banned_q = utils.safe_num(data)
        if banned_q > settings.banned_q then
            local chat_stats = settings.chat_prefix .. chat_id
            lua_redis.redis_make_request(task,
                redis_params,
                chat_stats,
                true, -- is write
                function() end,
                'HINCRBY',
                {chat_stats, 'perm_banned', '1'}
            )
            task:insert_result('TG_PERM_BAN', 15.0)
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

-- Register core symbols (scores defined in groups.conf)
rspamd_config.TG_FLOOD = {
    callback = tg_flood_cb,
    description = 'User is flooding',
    group = 'telegram_core'
}

rspamd_config.TG_REPEAT = {
    callback = tg_repeat_cb,
    description = 'User has sent a lot of equal messages',
    group = 'telegram_core'
}

rspamd_config.TG_SUSPICIOUS = {
    callback = tg_suspicious_cb,
    description = 'Suspicious activity',
    group = 'telegram_core'
}

rspamd_config.TG_BAN = {
    callback = tg_ban_cb,
    description = 'Banned for some time',
    group = 'telegram_core'
}

rspamd_config.TG_PERM_BAN = {
    callback = tg_perm_ban_cb,
    description = 'Permanently banned',
    group = 'telegram_core'
} 