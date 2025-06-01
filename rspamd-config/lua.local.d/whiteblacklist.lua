local rspamd_redis = require "rspamd_redis"

local settings = {
    whitelist_key = 'tg:whitelist:',
    blacklist_key = 'tg:blacklist:'
}

rspamd_config:register_symbol('WHITELIST', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id
    local function whitelist_cb (err, data)
        if err or not data then return end
        if data then
            task:insert_result('WHITELIST', 1.0)
        end
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                               cmd='SISMEMBER', args={settings.whitelist_key, user_key}, callback=whitelist_cb})
end)

rspamd_config:register_symbol('BLACKLIST', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id
    local function whitelist_cb (err, data)
        if err or not data then return end
        if data then
            task:insert_result('WHITELIST', 1.0)
        end
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                               cmd='SISMEMBER', args={settings.blacklist_key, user_key}, callback=whitelist_cb})
end)

rspamd_config:set_metric_symbol('WHITELIST', -20.0, 'User is in whitelist')
rspamd_config:set_metric_symbol('WHITELIST', 20.0, 'User is in blacklist')