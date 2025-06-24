local rspamd_redis = require "rspamd_redis"
local lua_redis = require "lua_redis"
local rspamd_logger = require "rspamd_logger"

local settings = {
    whitelist_users_key = 'tg:whitelist:users',
    blacklist_users_key = 'tg:blacklist:users',
    whitelist_words_key = 'tg:whitelist:words',
    blacklist_words_key = 'tg:blacklist:words',
    features_key = 'tg:enabled_features',
    user_prefix = 'tg:users:'
}

local redis_params

local function if_feature_enabled(task, chat_id, feature, cb)
  local chat_key = 'tg:chats:' .. chat_id
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

local function break_to_words_util(str)
  local t = {}
  for w in str:gmatch("%w+") do
    t[#t+1] = w
  end
  return t
end

-- WHITELIST_USER symbol
local function whitelist_user_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if user_id == "" then return end
  local user_key = settings.user_prefix .. user_id
  
  if_feature_enabled(task, chat_id, 'whitelist', function()
    local function whitelist_cb(err, data)
      if err then
        rspamd_logger.errx(task, 'whitelist_cb received error: %1', err)
        return
      end
      if data then
        task:insert_result('WHITELIST_USER', 1.0)
      end
    end
    lua_redis.redis_make_request(task,
      redis_params,
      settings.whitelist_users_key,
      false, -- is write
      whitelist_cb,
      'SISMEMBER',
      {settings.whitelist_users_key, user_key}
    )
  end)
end

-- BLACKLIST_USER symbol
local function blacklist_user_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if user_id == "" then return end
  local user_key = settings.user_prefix .. user_id
  
  if_feature_enabled(task, chat_id, 'blacklist', function()
    local function blacklist_cb(err, data)
      if err then
        rspamd_logger.errx(task, 'blacklist_cb received error: %1', err)
        return
      end
      if data then
        task:insert_result('BLACKLIST_USER', 1.0)
      end
    end
    lua_redis.redis_make_request(task,
      redis_params,
      settings.blacklist_users_key,
      false, -- is write
      blacklist_cb,
      'SISMEMBER',
      {settings.blacklist_users_key, user_key}
    )
  end)
end

-- WHITELIST_WORD symbol
local function whitelist_word_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if user_id == "" then return end
  
  if_feature_enabled(task, chat_id, 'whitelist', function()
    local msg = tostring(task:get_rawbody()) or ""
    local words = break_to_words_util(msg)
    local count = 0
    local function if_member_cb(err, data)
      if err then
        rspamd_logger.errx(task, 'if_member_cb received error: %1', err)
        return
      end
      if data then
        count = count + 1
      end
    end
    for word in words do
      lua_redis.redis_make_request(task,
        redis_params,
        settings.whitelist_words_key,
        false, -- is write
        if_member_cb,
        'SISMEMBER',
        {settings.whitelist_words_key, word}
      )
    end
    task:insert_result('WHITELIST_WORD', count)
  end)
end

-- BLACKLIST_WORD symbol
local function blacklist_word_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if user_id == "" then return end
  
  if_feature_enabled(task, chat_id, 'blacklist', function()
    local msg = tostring(task:get_rawbody()) or ""
    local words = break_to_words_util(msg)
    local count = 0
    local function if_member_cb(err, data)
      if err then
        rspamd_logger.errx(task, 'if_member_cb received error: %1', err)
        return
      end
      if data then
        count = count + 1
      end
    end
    for word in words do
      lua_redis.redis_make_request(task,
        redis_params,
        settings.blacklist_words_key,
        false, -- is write
        if_member_cb,
        'SISMEMBER',
        {settings.blacklist_words_key, word}
      )
    end
    task:insert_result('BLACKLIST_WORD', count)
  end)
end

-- Load redis server for module named 'whiteblacklist'
redis_params = lua_redis.parse_redis_server('whiteblacklist')
if redis_params then
  -- Register symbols using modern syntax
  rspamd_config.WHITELIST_USER = {
    callback = whitelist_user_cb,
    description = 'User is in whitelist',
    score = -20.0,
    group = 'telegram'
  }
  rspamd_config.BLACKLIST_USER = {
    callback = blacklist_user_cb,
    description = 'User is in blacklist',
    score = 20.0,
    group = 'telegram'
  }
  rspamd_config.WHITELIST_WORD = {
    callback = whitelist_word_cb,
    description = 'Word is in whitelist',
    score = -1.0,
    group = 'telegram'
  }
  rspamd_config.BLACKLIST_WORD = {
    callback = blacklist_word_cb,
    description = 'Word is in blacklist',
    score = 1.0,
    group = 'telegram'
  }
end 