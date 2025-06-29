--[[
  Timing-based Telegram Rules
  
  This module handles timing-based spam detection:
  - First message timing after join
  - Long silence detection
  - User activity patterns
]]

local utils = require "telegram.utils"
local settings = _G.telegram_settings

-- Initialize Redis connection
if not utils.init_redis('telegram') then
    return
end

local redis_params = utils.get_redis_params()

-- TG_TIMING: Handle all timing-related heuristics
local function tg_timing_cb(task)
    local user_id, _ = utils.get_user_chat_ids(task)
    if user_id == "" then return end
    
    local user_key = settings.user_prefix .. user_id
    local now = task:get_date({format = 'connect', gmt = true}) or os.time()

    local function timing_cb(err, data)
        if err then return end

        local joined = utils.safe_num(data[1]) -- may be nil
        local first_ts = utils.safe_num(data[2])
        local last_ts  = utils.safe_num(data[3])

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
            -- Set first_ts to mark first message sent
            lua_redis.redis_make_request(task, 
                redis_params, 
                user_key, 
                true, 
                function() end,
                'HSET', 
                {user_key, 'first_ts', tostring(now)}
            )
        end

        --------------------------------------------------
        -- 2. Long silence detection
        --------------------------------------------------
        if last_ts and (now - last_ts) >= settings.silence then
            task:insert_result('TG_SILENT', 1.5)
        end

        -- Always update last_ts to now
        lua_redis.redis_make_request(task, 
            redis_params, 
            user_key, 
            true, 
            function() end,
            'HSET', 
            {user_key, 'last_ts', tostring(now)}
        )
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

-- Register timing-based symbols (scores defined in groups.conf)
rspamd_config.TG_FIRST_FAST = {
    callback = tg_timing_cb,
    description = 'First message sent immediately after join',
    group = 'telegram_timing'
}

rspamd_config.TG_FIRST_SLOW = {
    callback = tg_timing_cb,
    description = 'First message sent long after join',
    group = 'telegram_timing'
}

rspamd_config.TG_SILENT = {
    callback = tg_timing_cb,
    description = 'User has been silent for a long time',
    group = 'telegram_timing'
} 