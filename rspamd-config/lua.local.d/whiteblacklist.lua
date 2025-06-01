local rspamd_redis = require "rspamd_redis"

local settings = {

}

rspamd_config:register_symbol('WHITELIST', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id


end)

rspamd_config:register_symbol('BLACKLIST', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id


end)

rspamd_config:set_metric_symbol('WHITELIST', -20.0, 'User is in whitelist')
rspamd_config:set_metric_symbol('WHITELIST', 20.0, 'User is in blacklist')