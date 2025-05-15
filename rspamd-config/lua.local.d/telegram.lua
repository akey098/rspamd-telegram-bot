local rspamd_redis = require "rspamd_redis"

rspamd_config:register_symbol('TG_FLOOD', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    if user_id == "" then return false end
    local redis_key = 'tg:users:' .. user_id
    -- Define callback to be called when Redis returns
    local function flood_cb(err, data)
      if err or not data then return end
      local count = tonumber(data) or 0
      --rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      --  cmd='HEXPIRE', args={redis_key, '60', 'NX', 'FIELDS', 1, 'flood'}, callback=function() end})
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
      cmd='HINCRBY', args={redis_key, 'flood', 1}, callback=flood_cb})
    
    
  end)

rspamd_config:register_symbol('TG_REPEAT', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local msg = tostring(task:get_rawbody()) or ""
    if user_id == "" then return end
    local hash_key = 'tg:users:' .. user_id
    -- Use HINCRBY on a hash field for the message text
    local function last_msg_cb(err, data)
      local function get_count_cb(err, data)
        if err then return end
        local count = tonumber(data) or 0
        if count > 5 then
          local stats_key = 'tg:users:' .. user_id
          local overall_stats = 'tg:stats'
          rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
            cmd='HINCRBY', args={overall_stats, 'spam_count', '1'}, callback=function() end})
          rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
            cmd='HINCRBY', args={stats_key, 'rep', '1'}, callback=function() end})
          task:insert_result('TG_REPEAT', 1.0)
        end
      end
      if tostring(data) == msg then
        rspamd_redis.make_request({task = task, host="127.0.0.1:6379", cmd='HINCRBY', args={hash_key, 'eq_msg_count', 1}, callback=get_count_cb})
      else
        rspamd_redis.make_request({task = task, host="127.0.0.1:6379", cmd='HSET', args={hash_key, 'eq_msg_count', 0}, callback=function() end})
      end
      rspamd_redis.make_request({task = task, host="127.0.0.1:6379", cmd='HSET', args={hash_key, 'last_msg', msg}, callback=function() end})
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HGET', args={hash_key, 'last_msg'}, callback=last_msg_cb})
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

local redis_params
local lua_redis = require "lua_redis"
redis_params = lua_redis.parse_redis_server('replies')

rspamd_config:set_metric_symbol('TG_FLOOD', 1.2, 'tg flood')
rspamd_config:set_metric_symbol('TG_REPEAT', 2.0, 'tg repeat')
rspamd_config:set_metric_symbol('TG_SUSPICIOUS', 10.0, 'tg suspicious')