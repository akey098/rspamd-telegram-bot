local rspamd_redis = require "rspamd_redis"

rspamd_config:register_symbol(
  "TG_FLOOD",
  5.0,
  function(task)
    local user = task:get_header("X-Telegram-User") or task:get_user()
    if not user then return false end
    local key = "tg:" .. user .. ":count"
    local max_msgs = 30
    local function incr_cb(err, data)
      if err then
        return
      end
      local count = tonumber(data or "0") or 0
      if count > max_msgs then
        local key = "tg:" .. user .. ":rep"
        rspamd_redis.make_request(task, nil, nil, 'INCR', { key })
        rspamd_redis.make_request(task, nil, nil, 'EXPIRE', { key, '86400' })
        task:insert_result("TG_FLOOD", 1)
      end
      if count == 1 then
        -- set 60s expiration on the counter on first message
        rspamd_redis.make_request(task, nil, nil, 'EXPIRE', {key, '60'})
      end
    end
    rspamd_redis.make_request(task, nil, incr_cb, 'INCR', {key})
  end)

rspamd_config:register_symbol("TG_REPEAT", 3.0,
  function(task)
    local user = task:get_header("X-Telegram-User") or task:get_user()
    if not user then return false end
    local text = task:get_rawbody() and task:get_rawbody():get_text() or ""
    local key = "tg:" .. user .. ":lastmsg"
    local function get_cb(err, data)
      if not err and data and data == text then
        task:insert_result("TG_REPEAT", 1)  -- same as last message
      end
      -- Update last message (store current text with TTL 300s)
      rspamd_redis.make_request(task, nil, nil, 'SETEX', {key, '300', text})
    end
    rspamd_redis.make_request(task, nil, get_cb, 'GET', {key})
  end)

rspamd_config:register_symbol( "TG_SUSPICIOUS", 4.0,
  function(task)
    local user = task:get_user() or task:get_header("X-Telegram-User")
    if not user then return false end
    local rep_key = "tg:"..user..":rep"
    local function redis_cb(err, data)
      if err or not data then return end
      local score = tonumber(data) or 0
      if score > 9 then
        -- user has bad reputation (<= -10), add a symbol
        task:insert_result("TG_SUSPICIOUS", 1)
      end
    end
    rspamd_redis.make_request(task,  -- asynchronous Redis request
      nil, redis_cb, 'GET', { rep_key })
  end)

rspamd_config:add_on_load(function(cfg, ev_base, worker)
  -- Only run in the normal filtering workers
  if worker:get_name() ~= 'normal' then
    return
  end

  -- Schedule a task every hour (3600 seconds)
  rspamd_config:add_periodic(ev_base, 3600.0, function()
    -- Callback to process all `tg:*:rep` keys
    local function redis_cb(err, replies)
      if err then
        rspamd_logger.errx(rspamd_config, 'Redis error while scanning reps: %1', err)
        return
      end
      for _, k in ipairs(replies) do
        -- Decrement each key by 1
        rspamd_redis.make_request(nil, nil, nil,
          'DECRBY', { k, '1' })
      end
    end

    -- Find all reputation keys
    rspamd_redis.make_request(nil, redis_cb, nil,
      'KEYS', { 'tg:*:rep' })

    -- Return true so that this periodic callback stays active
    return true
  end)
end)