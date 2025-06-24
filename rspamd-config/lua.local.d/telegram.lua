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
    link_spam = 3,
    mentions = 5,
    caps_ratio = 0.7,
    -- Timing heuristics (seconds)
    join_fast  = 10,          -- first message within 10 s of join → spammy
    join_slow  = 86400,       -- first message after 24 h of join → suspicious bot
    silence    = 2592000,     -- 30 days without message → dormant bot
    -- Additional heuristic thresholds
    invite_link_patterns = {'t.me/joinchat', 't.me/+', 'telegram.me/joinchat'},
    emoji_limit = 10,        -- more than 10 emoji considered spam
    phone_regex = '%+?%d[%d%-%s%(%)]%d%d%d%d', -- simplistic phone pattern
    spam_chat_regex = 't.me/joinchat',
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
        task:insert_result('TG_FLOOD', 1.2)
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
          task:insert_result('TG_REPEAT', 2.0)
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
      task:insert_result('TG_SUSPICIOUS', 5.0)
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
      task:insert_result('TG_BAN', 10.0)
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
      task:insert_result('TG_PERM_BAN', 15.0)
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

-- TG_LINK_SPAM: too many URLs in a single message
local function tg_link_spam_cb(task)
  local chat_id = tostring(task:get_header('X-Telegram-Chat', true) or "")
  if chat_id == "" then return end

  -- We don't need per-user tracking for simple link counting
  local urls = task:get_urls() or {}
  if #urls >= settings.link_spam then
    task:insert_result('TG_LINK_SPAM', 2.5)
  end
end

-- TG_MENTIONS: message mentions too many users (potentially mass ping)
local function tg_mentions_cb(task)
  local raw = tostring(task:get_rawbody()) or ""
  -- count occurrences of @username – Telegram usernames are 5–32 chars of
  -- letters, digits and underscores. We use a simple pattern here.
  local n = 0
  for _ in raw:gmatch("@[%w_]+") do n = n + 1 end
  if n >= settings.mentions then
    task:insert_result('TG_MENTIONS', 2.5)
  end
end

-- TG_CAPS: excessive capital letters (shouting)
local function tg_caps_cb(task)
  local text = tostring(task:get_rawbody()) or ""
  if #text < 20 then return end -- ignore very short messages

  local letters, caps = 0, 0
  for ch in text:gmatch("%a") do
    letters = letters + 1
    if ch:match("%u") then caps = caps + 1 end
  end
  if letters > 0 and (caps / letters) >= settings.caps_ratio then
    task:insert_result('TG_CAPS', 1.5)
  end
end

-- TG_TIMING related rules
local function tg_timing_cb(task)
  local user_id = tostring(task:get_header('X-Telegram-User', true) or "")
  if user_id == "" then return end
  local user_key = settings.user_prefix .. user_id

  local now = task:get_date({format = 'connect', gmt = true}) or os.time()

  local function timing_cb(err, data)
    if err then return end

    local joined = tonumber(data[1]) -- may be nil
    local first_ts = tonumber(data[2])
    local last_ts  = tonumber(data[3])

    --------------------------------------------------
    -- 1. First-message timing after join
    --------------------------------------------------
    if not first_ts and joined then
      local delta = now - joined
      if delta <= settings.join_fast then
        task:insert_result('TG_FIRST_FAST', 3.0)
      elseif delta >= settings.join_slow then
        task:insert_result('TG_FIRST_SLOW', 2.0)
      end
      -- set first_ts
      lua_redis.redis_make_request(task, redis_params, user_key, true, function() end,
        'HSET', {user_key, 'first_ts', tostring(now)})
    end

    --------------------------------------------------
    -- 2. Long silence detection
    --------------------------------------------------
    if last_ts and (now - last_ts) >= settings.silence then
      task:insert_result('TG_SILENT', 1.5)
    end

    -- Always update last_ts to now
    lua_redis.redis_make_request(task, redis_params, user_key, true, function() end,
      'HSET', {user_key, 'last_ts', tostring(now)})
  end

  -- Fetch joined, first_ts, last_ts in one HMGET
  lua_redis.redis_make_request(task,
    redis_params,
    user_key,
    false,
    timing_cb,
    'HMGET',
    {user_key, 'joined', 'first_ts', 'last_ts'}
  )
end

-- TG_INVITE_LINK: message contains Telegram invite link
local function tg_invite_link_cb(task)
  local text = tostring(task:get_rawbody()) or ""
  for _,pat in ipairs(settings.invite_link_patterns) do
    if text:lower():find(pat, 1, true) then
      task:insert_result('TG_INVITE_LINK', 4.0)
      break
    end
  end
end

-- TG_EMOJI_SPAM: too many emoji characters
local function tg_emoji_spam_cb(task)
  local text = tostring(task:get_rawbody()) or ""
  local count = 0
  for _ in text:gmatch('[600-64f300-5ff680-6ff1e0-1ff]') do
    count = count + 1
    if count > settings.emoji_limit then
      task:insert_result('TG_EMOJI_SPAM', 2.5)
      break
    end
  end
end

-- TG_PHONE_SPAM: message contains phone number pattern (promo spam)
local function tg_phone_spam_cb(task)
  local text = tostring(task:get_rawbody()) or ""
  if text:match(settings.phone_regex) then
    task:insert_result('TG_PHONE_SPAM', 3.0)
  end
end

-- TG_SPAM_CHAT: message contains spam chat
local function tg_spam_chat_cb(task)
  local text = tostring(task:get_rawbody()) or ""
  if text:match(settings.spam_chat_regex) then
    task:insert_result('TG_SPAM_CHAT', 3.0)
  end
end

-- Load redis server for module named 'telegram'
redis_params = lua_redis.parse_redis_server('telegram')
if redis_params then
  -- Register symbols using modern syntax
  rspamd_config.TG_FLOOD = {
    callback = tg_flood_cb,
    description = 'User is flooding',
    score = 1.2,
    group = 'telegram'
  }
  rspamd_config.TG_REPEAT = {
    callback = tg_repeat_cb,
    description = 'User have send a lot of equal messages',
    score = 2.0,
    group = 'telegram'
  }
  rspamd_config.TG_SUSPICIOUS = {
    callback = tg_suspicious_cb,
    description = 'Suspicious activity',
    score = 5.0,
    group = 'telegram'
  }
  rspamd_config.TG_BAN = {
    callback = tg_ban_cb,
    description = 'Banned for some time',
    score = 10.0,
    group = 'telegram'
  }
  rspamd_config.TG_PERM_BAN = {
    callback = tg_perm_ban_cb,
    description = 'Permanently banned',
    score = 15.0,
    group = 'telegram'
  }
  -- Register additional heuristics
  rspamd_config.TG_LINK_SPAM = {
    callback = tg_link_spam_cb,
    description = 'Message contains excessive number of links',
    score = 2.5,
    group = 'telegram'
  }
  rspamd_config.TG_MENTIONS = {
    callback = tg_mentions_cb,
    description = 'Message mentions too many users',
    score = 2.5,
    group = 'telegram'
  }
  rspamd_config.TG_CAPS = {
    callback = tg_caps_cb,
    description = 'Message is written almost entirely in capital letters',
    score = 1.5,
    group = 'telegram'
  }
  -- Timing heuristics
  rspamd_config.TG_FIRST_FAST = {
    callback = tg_timing_cb,
    description = 'First message sent immediately after join',
    score = 3.0,
    group = 'telegram'
  }
  rspamd_config.TG_FIRST_SLOW = {
    callback = tg_timing_cb,
    description = 'First message sent long after join',
    score = 2.0,
    group = 'telegram'
  }
  rspamd_config.TG_SILENT = {
    callback = tg_timing_cb,
    description = 'User has been silent for a long time',
    score = 1.5,
    group = 'telegram'
  }
  -- ClubDoorman-inspired heuristics
  rspamd_config.TG_INVITE_LINK = {
    callback = tg_invite_link_cb,
    description = 'Telegram invite link detected',
    score = 4.0,
    group = 'telegram'
  }
  rspamd_config.TG_EMOJI_SPAM = {
    callback = tg_emoji_spam_cb,
    description = 'Excessive emoji usage',
    score = 2.5,
    group = 'telegram'
  }
  rspamd_config.TG_PHONE_SPAM = {
    callback = tg_phone_spam_cb,
    description = 'Contains phone number spam',
    score = 3.0,
    group = 'telegram'
  }
  rspamd_config.TG_SPAM_CHAT = {
    callback = tg_spam_chat_cb,
    description = 'Contains spam chat',
    score = 3.0,
    group = 'telegram'
  }
end 