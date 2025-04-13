local rspamd_redis = require "rspamd_redis"

rspamd_config.TG_FLOOD = {
    callback = function(task)
        local user = task:get_header("X-Telegram-User")
        if not user then return false end
        local key = "tg:" .. user .. ":count"
        local max_msgs = 5
        local function incr_cb(err, data)
            if err then
                task:insert_result("TG_FLOOD", 0.0)
                return
            end
            local count = tonumber(data or "0") or 0
            if count > max_msgs then
                task:insert_result("TG_FLOOD", 1.0)
                -- Optionally, update reputation:
                rspamd_redis.make_request(task, nil, nil, 'INCR', {"tg:" .. user .. ":rep"})
            end
            if count == 1 then
                rspamd_redis.make_request(task, nil, nil, 'EXPIRE', {key, '60'})
            end
        end
        rspamd_redis.make_request(task, nil, incr_cb, 'INCR', {key})
        return false
    end,
    score = 5.0,
    description = "Telegram user sent too many messages in a short time",
    group = "telegram"
}

rspamd_config.TG_REPEAT = {
    callback = function(task)
        local user = task:get_header("X-Telegram-User")
        if not user then return false end
        -- Get the raw text of the message body
        local text = tostring(task:get_rawbody() or "")  -- raw body as string
        local key = "tg:" .. user .. ":lastmsg"
        local function get_cb(err, data)
            if not err and data then
                if data == text then
                    -- Same content as last message
                    task:insert_result("TG_REPEAT", 1.0)
                end
            end
            -- Store this message as last message with TTL 300s
            rspamd_redis.make_request(task, nil, nil, 'SETEX', {key, '300', text})
        end
        -- Asynchronously get the last message content
        rspamd_redis.make_request(task, nil, get_cb, 'GET', {key})
        return false
    end,
    score = 3.0,
    description = "User repeated the same message content",
    group = "telegram"
}

rspamd_config.TG_SUSPICIOUS = {
    callback = function(task)
        local user = task:get_header("X-Telegram-User")
        if not user then return false end
        local rep_key = "tg:" .. user .. ":rep"
        local function rep_cb(err, data)
            if not err and data then
                local rep = tonumber(data)
                if rep and rep >= 5 then
                    -- User has 5 or more spam incidents
                    task:insert_result("TG_SUSPICIOUS", 1.0)
                end
            end
        end
        rspamd_redis.make_request(task, nil, rep_cb, 'GET', {rep_key})
        return false
    end,
    score = 4.0,
    description = "User has a bad spam reputation",
    group = "telegram"
}