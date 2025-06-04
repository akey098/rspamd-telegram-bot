local rspamd_redis = require "rspamd_redis"

local settings = {
    whitelist_users_key = 'tg:whitelist:users',
    blacklist_users_key = 'tg:blacklist:users',
    whitelist_words_key = 'tg:whitelist:words',
    blacklist_words_key = 'tg:blacklist:words'
}

rspamd_config:register_symbol('WHITELIST_USER', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id
    local function whitelist_cb (err, data)
        if err or not data then return end
        if data then
            task:insert_result('WHITELIST_USER', 1.0)
        end
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                               cmd='SISMEMBER', args={settings.whitelist_users_key, user_key}, callback=whitelist_cb})
end)

rspamd_config:register_symbol('BLACKLIST_USER', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id
    local function whitelist_cb (err, data)
        if err or not data then return end
        if data then
            task:insert_result('BLACKLIST_USER', 1.0)
        end
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                               cmd='SISMEMBER', args={settings.blacklist_users_key, user_key}, callback=whitelist_cb})
end)

rspamd_config:register_symbol('WHITELIST_WORD', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end

end)

rspamd_config:register_symbol('BLACKLIST_WORD', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end

end)

rspamd_config:set_metric_symbol('WHITELIST_USER', -20.0, 'User is in whitelist')
rspamd_config:set_metric_symbol('BLACKLIST_USER', 20.0, 'User is in blacklist')
rspamd_config:set_metric_symbol('WHITELIST_WORD', -2.0, 'Word is in whitelist')
rspamd_config:set_metric_symbol('BLACKLIST_WORD', 2.0, 'Word is in blacklist')