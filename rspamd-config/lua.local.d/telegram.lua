local rspamd_redis = require "rspamd_redis"

rspamd_config.TG_FLOOD = {
  callback = function(task)
    local user_id = tostring(task:get_user_id() or "")
    if user_id == "" then return false end
    local redis_key = 'tg:user:' .. user_id .. ':flood'
    -- Define callback to be called when Redis returns
    local function flood_cb(task, err, data)
      if err or not data then return end
      local count = tonumber(data) or 0
      if count > 5 then
        task:insert_result('TG_FLOOD', 1.0)
      end
    end
    -- Increment flood counter and set a 60s expiry
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='INCR', args={redis_key}, callback=flood_cb})
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='EXPIRE', args={redis_key, '60'}, callback=function() end})
    local stats_key = 'tg:users:' .. user_id
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HINCRBY', args={stats_key, 'spam_count', '1'}, callback=function() end})
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HINCRBY', args={stats_key, 'rep', '1'}, callback=function() end})
    return false  -- return false so symbol is only added in callback
  end,
  score = 1.0,
  description = 'Telegram message flood'
}


rspamd_config.TG_REPEAT = {
  callback = function(task)
    local user_id = tostring(task:get_user_id() or "")
    local msg = task:get_content() or ""
    local short = msg:sub(1, 100)  -- limit length
    if user_id == "" or short == "" then return false end
    local hash_key = 'tg:user:' .. user_id .. ':repeat'
    -- Use HINCRBY on a hash field for the message text
    local function repeat_cb(task, err, data)
      if err or not data then return end
      local repeats = tonumber(data) or 0
      if repeats > 1 then
        task:insert_result('TG_REPEAT', 1.0)
      end
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HINCRBY', args={hash_key, short, '1'}, callback=repeat_cb})
    local stats_key = 'tg:users:' .. user_id
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HINCRBY', args={stats_key, 'spam_count', '1'}, callback=function() end})
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HINCRBY', args={stats_key, 'rep', '1'}, callback=function() end})
    return false
  end,
  score = 1.0,
  description = 'Telegram repeated message'
}

rspamd_config.TG_SUSPICIOUS = {
  callback = function(task)
    local user_id = tostring(task:get_user_id() or "")
    if user_id == "" then return false end
    local spam_key = 'tg:user:' .. user_id .. ':spam_count'
    -- Increment total spam count for this user
    local function spam_cb(task, err, data)
      if err or not data then return end
      local total = tonumber(data) or 0
      if total > 10 then
        task:insert_result('TG_SUSPICIOUS', 1.0)
      end
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='INCR', args={spam_key}, callback=spam_cb})
    local stats_key = 'tg:users:' .. user_id
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HINCRBY', args={stats_key, 'spam_count', '1'}, callback=function() end})
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HINCRBY', args={stats_key, 'rep', '1'}, callback=function() end})
    return false
  end,
  score = 1.0,
  description = 'Telegram suspicious user'
}

rspamd_config.TG_STATS = {
  callback = function(task)
    local user = task:get_user_id() or ""
    local stats_key = 'tg:users:' .. user
    local function stats_cb(task, err, data)
      if err or not data then return end
      local spam = data[1] or 0
      local rep = data[2] or 0
      local deleted = data[3] or 0
      -- Format and send response via Telegram API...
      -- (This part would involve the Rust bot, not Rspamd directly.)
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HMGET', args={stats_key, 'spam_count', 'rep', 'deleted'}, callback=stats_cb})
    return false
  end,
  score = 0.0,
  description = 'Telegram stats query'
}

local rspamd_logger = require "rspamd_logger"
local lua_redis = require "lua_redis"

-- Make sure redis_params is parsed at the top of your module
local redis_params = lua_redis.parse_redis_server('multimaps') -- or whatever upstream you use

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