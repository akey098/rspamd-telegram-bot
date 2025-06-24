local rspamd_redis = require "rspamd_redis"

-- Plugin configuration
local config = {
    whitelist_users_key = 'tg:whitelist:users',
    blacklist_users_key = 'tg:blacklist:users',
    whitelist_words_key = 'tg:whitelist:words',
    blacklist_words_key = 'tg:blacklist:words',
    features_key = 'tg:enabled_features',
    user_prefix = 'tg:users:'
}

-- Helper function to check if a feature is enabled for a chat
local function if_feature_enabled(task, chat_id, feature, cb)
    local chat_key = 'tg:chats:' .. chat_id
    local field = 'feat:' .. feature
    rspamd_redis.make_request({
        task = task,
        host = "127.0.0.1:6379",
        cmd = 'HGET',
        args = {chat_key, field},
        callback = function(err, data)
            if err then return end
            if data == '1' then
                cb()
            elseif data == '0' then
                return
            else
                rspamd_redis.make_request({
                    task = task,
                    host = "127.0.0.1:6379",
                    cmd = 'SISMEMBER',
                    args = {config.features_key, feature},
                    callback = function(e, d)
                        if e then return end
                        if d == 1 or d == true then cb() end
                    end
                })
            end
        end
    })
end

-- Helper function to break text into words
local function break_to_words_util(str)
    local t = {}
    for w in str:gmatch("%w+") do
        t[#t+1] = w
    end
    return t
end

-- Create the plugin
local plugin = {}

-- Plugin initialization
function plugin.init()
    -- Register symbols with their scores and descriptions
    rspamd_config:set_metric_symbol('WHITELIST_USER', -20.0, 'User is in whitelist')
    rspamd_config:set_metric_symbol('BLACKLIST_USER', 20.0, 'User is in blacklist')
    rspamd_config:set_metric_symbol('WHITELIST_WORD', -1.0, 'Word is in whitelist')
    rspamd_config:set_metric_symbol('BLACKLIST_WORD', 1.0, 'Word is in blacklist')
end

-- Main plugin callback function
function plugin.callback(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
    
    if user_id == "" then return false end
    
    local user_key = config.user_prefix .. user_id
    
    -- Check whitelist users
    if_feature_enabled(task, chat_id, 'whitelist', function()
        local function whitelist_cb(err, data)
            if err or not data then return end
            if data then
                task:insert_result('WHITELIST_USER', 1.0)
            end
        end
        rspamd_redis.make_request({
            task = task,
            host = "127.0.0.1:6379",
            cmd = 'SISMEMBER',
            args = {config.whitelist_users_key, user_key},
            callback = whitelist_cb
        })
    end)
    
    -- Check blacklist users
    if_feature_enabled(task, chat_id, 'blacklist', function()
        local function blacklist_cb(err, data)
            if err or not data then return end
            if data then
                task:insert_result('BLACKLIST_USER', 1.0)
            end
        end
        rspamd_redis.make_request({
            task = task,
            host = "127.0.0.1:6379",
            cmd = 'SISMEMBER',
            args = {config.blacklist_users_key, user_key},
            callback = blacklist_cb
        })
    end)
    
    -- Check whitelist words
    if_feature_enabled(task, chat_id, 'whitelist', function()
        local msg = tostring(task:get_rawbody()) or ""
        local words = break_to_words_util(msg)
        local count = 0
        local function if_member_cb(err, data)
            if err or not data then return end
            if data then
                count = count + 1
            end
        end
        for word in words do
            rspamd_redis.make_request({
                task = task,
                host = "127.0.0.1:6379",
                cmd = 'SISMEMBER',
                args = {config.whitelist_words_key, word},
                callback = if_member_cb
            })
        end
        task:insert_result('WHITELIST_WORD', count)
    end)
    
    -- Check blacklist words
    if_feature_enabled(task, chat_id, 'blacklist', function()
        local msg = tostring(task:get_rawbody()) or ""
        local words = break_to_words_util(msg)
        local count = 0
        local function if_member_cb(err, data)
            if err or not data then return end
            if data then
                count = count + 1
            end
        end
        for word in words do
            rspamd_redis.make_request({
                task = task,
                host = "127.0.0.1:6379",
                cmd = 'SISMEMBER',
                args = {config.blacklist_words_key, word},
                callback = if_member_cb
            })
        end
        task:insert_result('BLACKLIST_WORD', count)
    end)
    
    return true
end

-- Register the plugin
rspamd_config:register_plugin('whiteblacklist', plugin)