local rspamd_redis = require "rspamd_redis"

local settings = {
    flood = 30,
    repeated = 6,
    suspicious = 10,
    ban = 20,
    user_prefix = 'tg:users:',
    chat_prefix = 'tg:chats:',
    exp_flood = '60',
    exp_ban = '3600',
    banned_q = 3
    ,features_key = 'tg:enabled_features'
}

local function if_feature_enabled(task, chat_id, feature, cb)
  local chat_key = settings.chat_prefix .. chat_id
  local field = 'feat:' .. feature
  rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
    cmd='HGET', args={chat_key, field}, callback=function(err, data)
      if err then return end
      if data == '1' then
        cb()
      elseif data == '0' then
        return
      else
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='SISMEMBER', args={settings.features_key, feature}, callback=function(e,d)
            if e then return end
            if d == 1 or d == true then cb() end
          end})
      end
    end})
end

rspamd_config:register_symbol('TG_FLOOD', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id
    if_feature_enabled(task, chat_id, 'flood', function()
      local function flood_cb(err, data)
        if err or not data then return end
        local count = tonumber(data) or 0
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HEXPIRE', args={user_key, settings.exp_flood, 'NX', 'FIELDS', 1, 'flood'}, callback=function() end})
        if count > settings.flood then
          local chat_key = settings.chat_prefix .. chat_id
          rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
            cmd='HINCRBY', args={chat_key, 'spam_count', '1'}, callback=function() end})
          rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
            cmd='HINCRBY', args={user_key, 'rep', '1'}, callback=function() end})
          task:insert_result('TG_FLOOD', 1.0)
        end
      end
      rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
        cmd='HINCRBY', args={user_key, 'flood', 1}, callback=flood_cb})
    end)
    
  end)

rspamd_config:register_symbol('TG_REPEAT', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
    local msg = tostring(task:get_rawbody()) or ""
    if user_id == "" then return end
    local user_key = settings.user_prefix .. user_id
    -- Use HINCRBY on a hash field for the message text
    if_feature_enabled(task, chat_id, 'repeat', function()
    local function last_msg_cb(_, data)
      local function get_count_cb(_err, _data)
        if _err then return end
        local count = tonumber(_data) or 0
        if count > settings.repeated then
          local chat_key = settings.chat_prefix .. chat_id
          rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HINCRBY', args={chat_key, 'spam_count', '1'}, callback=function() end})
          rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
            cmd='HINCRBY', args={user_key, 'rep', '1'}, callback=function() end})
          task:insert_result('TG_REPEAT', 1.0)
        end
      end
      if tostring(data) == msg then
        rspamd_redis.make_request({task = task, host="127.0.0.1:6379", cmd='HINCRBY', args={user_key, 'eq_msg_count', 1}, callback=get_count_cb})
      else
        rspamd_redis.make_request({task = task, host="127.0.0.1:6379", cmd='HSET', args={user_key, 'eq_msg_count', 0}, callback=function() end})
      end
      rspamd_redis.make_request({task = task, host="127.0.0.1:6379", cmd='HSET', args={user_key, 'last_msg', msg}, callback=function() end})
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HGET', args={user_key, 'last_msg'}, callback=last_msg_cb})
    end)
  end)

rspamd_config:register_symbol('TG_SUSPICIOUS', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id
    local function spam_cb(err, data)
      if err or not data then return end
      local total = tonumber(data) or 0
      if total > settings.suspicious then
        local chat_stats = settings.chat_prefix .. chat_id
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HINCRBY', args={chat_stats, 'spam_count', '1'}, callback=function() end})
        rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
          cmd='HINCRBY', args={user_key, 'rep', '1'}, callback=function() end})
        task:insert_result('TG_SUSPICIOUS', 1.0)
      end
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
      cmd='HGET', args={user_key, 'rep'}, callback=spam_cb})
  end)

rspamd_config:register_symbol('TG_BAN', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id
    local function ban_cb(err, data)
        if err or not data then return end
        local total = tonumber(data) or 0
        if total > settings.ban then
            local chat_stats = settings.chat_prefix .. chat_id
            rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                                       cmd='HINCRBY', args={chat_stats, 'banned', '1'}, callback=function() end})
            local function banned_cb(_err, _data)
                if _err or not _data then return end
                rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                                           cmd='HEXPIRE', args={user_key, settings.exp_ban, 'FIELDS', 1, 'banned'}, callback=function() end})
            end
            rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                                       cmd='HSET', args={user_key, 'banned', '1'}, callback=banned_cb})
            rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                                       cmd='HINCRBY', args={user_key, 'rep', '-5'}, callback=function() end})
            rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                                       cmd='HINCRBY', args={user_key, 'banned_q', '1'}, callback=function() end})
            task:insert_result('TG_BAN', 1.0)
        end
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                               cmd='HGET', args={user_key, 'rep'}, callback=ban_cb})
end)

rspamd_config:register_symbol('TG_PERM_BAN', 1.0, function(task)
    local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
    local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
    if user_id == "" then return false end
    local user_key = settings.user_prefix .. user_id
    local function perm_ban_cb (err, data)
        if err or not data then return end
        local banned_q = tonumber(data) or 0
        if banned_q > settings.banned_q then
            local chat_stats = settings.chat_prefix .. chat_id
            rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                                       cmd='HINCRBY', args={chat_stats, 'perm_banned', '1'}, callback=function() end})
            task:insert_result('TG_PERM_BAN', 1.0)
        end
    end
    rspamd_redis.make_request({task=task, host="127.0.0.1:6379",
                               cmd='HGET', args={user_key, 'banned_q'}, callback=perm_ban_cb})
end)

rspamd_config:set_metric_symbol('TG_FLOOD', 1.2, 'User is flooding')
rspamd_config:set_metric_symbol('TG_REPEAT', 2.0, 'User have send a lot of equal messages')
rspamd_config:set_metric_symbol('TG_SUSPICIOUS', 5.0, 'Suspicious activity')
rspamd_config:set_metric_symbol('TG_BAN', 10.0, 'Banned for some time')
rspamd_config:set_metric_symbol('TG_PERM_BAN', 15.0, 'Permanently banned')