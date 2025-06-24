local rspamd_redis = require "rspamd_redis"
local lua_redis = require "lua_redis"
local rspamd_logger = require "rspamd_logger"

local settings = {
    flood = 30,
    repeated = 6,
    suspicious = 10,
    ban = 20,
    user_prefix = 'tg:users:',
    chat_prefix = 'tg:chats:',
    exp_flood = '60',
    exp_ban = '3600',
    banned_q = 3,
    features_key = 'tg:enabled_features'
}

local redis_params

local function if_feature_enabled(task, chat_id, feature, cb)
  local chat_key = settings.chat_prefix .. chat_id
  local field = 'feat:' .. feature
  lua_redis.redis_make_request(task,
    redis_params,
    chat_key,
    false, -- is write
    function(err, data)
      if err then return end
      if data == '1' then
        cb()
      elseif data == '0' then
        return
      else
        lua_redis.redis_make_request(task,
          redis_params,
          settings.features_key,
          false, -- is write
          function(e, d)
            if e then return end
            if d == 1 or d == true then cb() end
          end,
          'SISMEMBER',
          {settings.features_key, feature}
        )
      end
    end,
    'HGET',
    {chat_key, field}
  )
end

-- TG_FLOOD symbol
local function tg_flood_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if user_id == "" then return end
  local user_key = settings.user_prefix .. user_id
  
  if_feature_enabled(task, chat_id, 'flood', function()
    local function flood_cb(err, data)
      if err then
        rspamd_logger.errx(task, 'flood_cb received error: %1', err)
        return
      end
      local count = tonumber(data) or 0
      lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        true, -- is write
        function() end,
        'HEXPIRE',
        {user_key, settings.exp_flood, 'NX', 'FIELDS', 1, 'flood'}
      )
      if count > settings.flood then
        local chat_key = settings.chat_prefix .. chat_id
        lua_redis.redis_make_request(task,
          redis_params,
          chat_key,
          true, -- is write
          function() end,
          'HINCRBY',
          {chat_key, 'spam_count', '1'}
        )
        lua_redis.redis_make_request(task,
          redis_params,
          user_key,
          true, -- is write
          function() end,
          'HINCRBY',
          {user_key, 'rep', '1'}
        )
        task:insert_result('TG_FLOOD', 1.0)
      end
    end
    lua_redis.redis_make_request(task,
      redis_params,
      user_key,
      true, -- is write
      flood_cb,
      'HINCRBY',
      {user_key, 'flood', 1}
    )
  end)
end

-- TG_REPEAT symbol
local function tg_repeat_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if user_id == "" then return end
  local user_key = settings.user_prefix .. user_id
  
  if_feature_enabled(task, chat_id, 'repeat', function()
    local msg = tostring(task:get_rawbody()) or ""
    local function last_msg_cb(err, data)
      if err then
        rspamd_logger.errx(task, 'last_msg_cb received error: %1', err)
        return
      end
      local function get_count_cb(_err, _data)
        if _err then return end
        local count = tonumber(_data) or 0
        if count > settings.repeated then
          local chat_key = settings.chat_prefix .. chat_id
          lua_redis.redis_make_request(task,
            redis_params,
            chat_key,
            true, -- is write
            function() end,
            'HINCRBY',
            {chat_key, 'spam_count', '1'}
          )
          lua_redis.redis_make_request(task,
            redis_params,
            user_key,
            true, -- is write
            function() end,
            'HINCRBY',
            {user_key, 'rep', '1'}
          )
          task:insert_result('TG_REPEAT', 1.0)
        end
      end
      if tostring(data) == msg then
        lua_redis.redis_make_request(task,
          redis_params,
          user_key,
          true, -- is write
          get_count_cb,
          'HINCRBY',
          {user_key, 'eq_msg_count', 1}
        )
      else
        lua_redis.redis_make_request(task,
          redis_params,
          user_key,
          true, -- is write
          function() end,
          'HSET',
          {user_key, 'eq_msg_count', 0}
        )
      end
      lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        true, -- is write
        function() end,
        'HSET',
        {user_key, 'last_msg', msg}
      )
    end
    lua_redis.redis_make_request(task,
      redis_params,
      user_key,
      false, -- is write
      last_msg_cb,
      'HGET',
      {user_key, 'last_msg'}
    )
  end)
end

-- TG_SUSPICIOUS symbol
local function tg_suspicious_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if user_id == "" then return end
  local user_key = settings.user_prefix .. user_id
  
  local function spam_cb(err, data)
    if err then
      rspamd_logger.errx(task, 'spam_cb received error: %1', err)
      return
    end
    local total = tonumber(data) or 0
    if total > settings.suspicious then
      local chat_stats = settings.chat_prefix .. chat_id
      lua_redis.redis_make_request(task,
        redis_params,
        chat_stats,
        true, -- is write
        function() end,
        'HINCRBY',
        {chat_stats, 'spam_count', '1'}
      )
      lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        true, -- is write
        function() end,
        'HINCRBY',
        {user_key, 'rep', '1'}
      )
      task:insert_result('TG_SUSPICIOUS', 1.0)
    end
  end
  lua_redis.redis_make_request(task,
    redis_params,
    user_key,
    false, -- is write
    spam_cb,
    'HGET',
    {user_key, 'rep'}
  )
end

-- TG_BAN symbol
local function tg_ban_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if user_id == "" then return end
  local user_key = settings.user_prefix .. user_id
  
  local function ban_cb(err, data)
    if err then
      rspamd_logger.errx(task, 'ban_cb received error: %1', err)
      return
    end
    local total = tonumber(data) or 0
    if total > settings.ban then
      local chat_stats = settings.chat_prefix .. chat_id
      lua_redis.redis_make_request(task,
        redis_params,
        chat_stats,
        true, -- is write
        function() end,
        'HINCRBY',
        {chat_stats, 'banned', '1'}
      )
      local function banned_cb(_err, _data)
        if _err or not _data then return end
        lua_redis.redis_make_request(task,
          redis_params,
          user_key,
          true, -- is write
          function() end,
          'HEXPIRE',
          {user_key, settings.exp_ban, 'FIELDS', 1, 'banned'}
        )
      end
      lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        true, -- is write
        banned_cb,
        'HSET',
        {user_key, 'banned', '1'}
      )
      lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        true, -- is write
        function() end,
        'HINCRBY',
        {user_key, 'rep', '-5'}
      )
      lua_redis.redis_make_request(task,
        redis_params,
        user_key,
        true, -- is write
        function() end,
        'HINCRBY',
        {user_key, 'banned_q', '1'}
      )
      task:insert_result('TG_BAN', 1.0)
    end
  end
  lua_redis.redis_make_request(task,
    redis_params,
    user_key,
    false, -- is write
    ban_cb,
    'HGET',
    {user_key, 'rep'}
  )
end

-- TG_PERM_BAN symbol
local function tg_perm_ban_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if user_id == "" then return end
  local user_key = settings.user_prefix .. user_id
  
  local function perm_ban_cb(err, data)
    if err then
      rspamd_logger.errx(task, 'perm_ban_cb received error: %1', err)
      return
    end
    local banned_q = tonumber(data) or 0
    if banned_q > settings.banned_q then
      local chat_stats = settings.chat_prefix .. chat_id
      lua_redis.redis_make_request(task,
        redis_params,
        chat_stats,
        true, -- is write
        function() end,
        'HINCRBY',
        {chat_stats, 'perm_banned', '1'}
      )
      task:insert_result('TG_PERM_BAN', 1.0)
    end
  end
  lua_redis.redis_make_request(task,
    redis_params,
    user_key,
    false, -- is write
    perm_ban_cb,
    'HGET',
    {user_key, 'banned_q'}
  )
end

-- Load redis server for module named 'telegram'
redis_params = lua_redis.parse_redis_server('telegram')
if redis_params then
  -- Register symbols using modern syntax
  rspamd_config.TG_FLOOD = {
    callback = tg_flood_cb,
    description = 'User is flooding'
  }
  rspamd_config.TG_REPEAT = {
    callback = tg_repeat_cb,
    description = 'User have send a lot of equal messages'
  }
  rspamd_config.TG_SUSPICIOUS = {
    callback = tg_suspicious_cb,
    description = 'Suspicious activity'
  }
  rspamd_config.TG_BAN = {
    callback = tg_ban_cb,
    description = 'Banned for some time'
  }
  rspamd_config.TG_PERM_BAN = {
    callback = tg_perm_ban_cb,
    description = 'Permanently banned'
  }
end 