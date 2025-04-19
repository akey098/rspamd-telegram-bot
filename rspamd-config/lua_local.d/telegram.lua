local rspamd_redis = require "rspamd_redis"

rspamd_config.TG_FLOOD = {
  callback = function(task)
    local user = task:get_header("X-Telegram-User") or task:get_user()
    if not user then return false end
    local key = "tg:" .. user .. ":count"
    local max_msgs = 5
    local function incr_cb(err, data)
      if err then
        task:insert_result("TG_FLOOD", 0.0); return
      end
      local count = tonumber(data or "0") or 0
      if count > max_msgs then
        task:insert_result("TG_FLOOD", 1.0)  -- user sent >5 messages in interval
        -- Optionally increase user's spam incident count:
        rspamd_redis.make_request(task, nil, nil, 'INCR', {"tg:"..user..":rep"})
      end
      if count == 1 then
        -- set 60s expiration on the counter on first message
        rspamd_redis.make_request(task, nil, nil, 'EXPIRE', {key, '60'})
      end
    end
    rspamd_redis.make_request(task, nil, incr_cb, 'INCR', {key})
    return true
  end,
  score = 5.0,
  description = "Telegram user sent too many messages in a short time",
  group = "telegram"
}


rspamd_config.TG_REPEAT = {
  callback = function(task)
    local user = task:get_header("X-Telegram-User") or task:get_user()
    if not user then return false end
    local text = task:get_rawbody() and task:get_rawbody():get_text() or ""
    local key = "tg:" .. user .. ":lastmsg"
    local function get_cb(err, data)
      if not err and data and data == text then
        task:insert_result("TG_REPEAT", 1.0)  -- same as last message
      end
      -- Update last message (store current text with TTL 300s)
      rspamd_redis.make_request(task, nil, nil, 'SETEX', {key, '300', text})
    end
    rspamd_redis.make_request(task, nil, get_cb, 'GET', {key})
    return true
  end,
  score = 3.0,
  description = "User repeated the same message content",
  group = "telegram"
}

rspamd_config.TG_SUSPICIOUS = {
  callback = function(task)
    local user = task:get_user() or task:get_header("X-Telegram-User")
    if not user then return false end
    local rep_key = "user:reputation_scores"
    local function redis_cb(err, data)
      if err or not data then return end
      local score = tonumber(data) or 0
      if score <= -10 then
        -- user has bad reputation (<= -10), add a symbol
        task:insert_result("USER_REP_BAD", 1.0, tostring(score))
      end
    end
    rspamd_redis.make_request(task,  -- asynchronous Redis request
      nil, redis_cb, 'HGET', {rep_key, user})
    return true  -- task will be resumed after Redis callback
  end,
  score = 4.0,
  description = "User has a bad spam reputation",
  group = "telegram"
}
