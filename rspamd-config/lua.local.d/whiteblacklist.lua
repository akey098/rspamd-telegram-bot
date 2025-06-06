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
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                                   cmd='SISMEMBER', args={settings.whitelist_words_key, word}, callback=if_member_cb})
    end
    task:insert_result('WHITELIST_WORD', count)
end)

rspamd_config:register_symbol('BLACKLIST_WORD', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end
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
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                                   cmd='SISMEMBER', args={settings.blacklist_words_key, word}, callback=if_member_cb})
    end
    task:insert_result('BLACKLIST_WORD', count)
end)

function break_to_words_util(str)
    local t = {}
    for w in str:gmatch("%w+") do
        t[#t+1] = w
    end
    return t
end

rspamd_config:set_metric_symbol('WHITELIST_USER', -20.0, 'User is in whitelist')
rspamd_config:set_metric_symbol('BLACKLIST_USER', 20.0, 'User is in blacklist')
rspamd_config:set_metric_symbol('WHITELIST_WORD', -1.0, 'Word is in whitelist')
rspamd_config:set_metric_symbol('BLACKLIST_WORD', 1.0, 'Word is in blacklist')