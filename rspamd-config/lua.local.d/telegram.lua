local rspamd_redis = require "rspamd_redis"

rspamd_config:register_symbol('TG_FLOOD', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end
    local redis_key = 'tg:user:' .. user_id .. ':flood'
    -- Define callback to be called when Redis returns
    local function flood_cb(err, data)
      if err or not data then return end
      local count = tonumber(data) or 0
      if count == 1 then
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='EXPIRE', args={redis_key, '60'}, callback=function() end})
      end
      if count > 30 then
        local stats_key = 'tg:users:' .. user_id
        local overall_stats = 'tg:stats'
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HINCRBY', args={overall_stats, 'spam_count', '1'}, callback=function() end})
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HINCRBY', args={stats_key, 'rep', '1'}, callback=function() end})
        task:insert_result('TG_FLOOD', 1.0)
      end
    end
    -- Increment flood counter and set a 60s expiry
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='INCR', args={redis_key}, callback=flood_cb})
    
    
  end)




rspamd_config:register_symbol('TG_REPEAT', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local msg = tostring(task:get_rawbody()) or ""
    if user_id == "" then return end
    local hash_key = 'tg:user:' .. user_id .. ':lastmsg'
    -- Use HINCRBY on a hash field for the message text
    local function last_msg_cb(err, data)
      if tostring(data) == msg then
        local stats_key = 'tg:users:' .. user_id
        local overall_stats = 'tg:stats'
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HINCRBY', args={overall_stats, 'spam_count', '1'}, callback=function() end})
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HINCRBY', args={stats_key, 'rep', '1'}, callback=function() end})
        task:insert_result('TG_REPEAT', 1.0)
      end
      rspamd_redis.make_request({task = task, host="127.0.0.1:6379", cmd='SET', args={hash_key, msg}, callback=function() end})
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='GET', args={hash_key}, callback=last_msg_cb})
  end)



rspamd_config:register_symbol('TG_SUSPICIOUS', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end
    -- Increment total spam count for this user
    local function spam_cb(err, data)
      if err or not data then return end
      local total = tonumber(data) or 0
      if total > 10 then
        local stats_key = 'tg:users:' .. user_id
        local overall_stats = 'tg:stats'
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HINCRBY', args={overall_stats, 'spam_count', '1'}, callback=function() end})
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HINCRBY', args={stats_key, 'rep', '1'}, callback=function() end})
        task:insert_result('TG_SUSPICIOUS', 1.0)
      end
    end
    local stats_key = 'tg:users:' .. user_id
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HGET', args={stats_key, 'rep'}, callback=spam_cb})
  end)


--[[
rspamd_config:add_on_load(function(cfg, ev_base, worker)
  if worker:get_name() ~= 'normal' then
    return
  end

  rspamd_logger.infox(rspamd_config, "Setting up periodic decrement for user reputations")

  rspamd_config:add_periodic(ev_base, 3600.0, function()
    local function redis_cb(err, keys)
      if err then
        rspamd_logger.errx("Redis KEYS error: %1", err)
        return true -- keep periodic running
      end

      if not keys or type(keys) ~= "table" then
        rspamd_logger.infox("No reputation keys found")
        return true
      end

      for _, key in ipairs(keys) do
        rspamd_logger.debugm("telegram", rspamd_config, "Decrementing reputation key: %s", key)
        lua_redis.redis_make_request({
          config = rspamd_config,
          ev_base = ev_base,
          host = redis_params,
          cmd = "DECRBY",
          args = { key, "1" },
          callback = function() end, -- silent
          is_write = true
        })
      end

      return true
    end

    -- Perform scan for keys matching tg:*:rep
    lua_redis.redis_make_request({
      config = rspamd_config,
      ev_base = ev_base,
      host = redis_params,
      cmd = "KEYS",
      args = { "tg:*:rep" },
      callback = redis_cb,
      is_write = false
    })

    return true -- keep periodic alive
  end)
end)
]]--
rspamd_config:set_metric_symbol('TG_FLOOD', 1.2, 'tg flood')
rspamd_config:set_metric_symbol('TG_REPEAT', 2.0, 'tg repeat')
rspamd_config:set_metric_symbol('TG_SUSPICIOUS', 10.0, 'tg suspicious')