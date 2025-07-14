--[[
  Shared utilities for Telegram Rspamd rules
]]

local rspamd_redis = require "rspamd_redis"
local lua_redis = require "lua_redis"
local rspamd_logger = require "rspamd_logger"

local M = {}

-- Redis connection parameters
local redis_params

-- Initialize Redis connection
function M.init_redis(module_name)
    if not lua_redis then
        rspamd_logger.errx(nil, 'lua_redis module not available')
        return false
    end
    redis_params = lua_redis.parse_redis_server(module_name)
    return redis_params ~= nil
end

-- Get Redis parameters
function M.get_redis_params()
    return redis_params
end

-- Feature flag checker
function M.if_feature_enabled(task, chat_id, feature, cb)
    if not lua_redis or not redis_params then
        return
    end
    
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
                -- Check global features
                lua_redis.redis_make_request(task,
                    redis_params,
                    'tg:enabled_features',
                    false, -- is write
                    function(e, d)
                        if e then return end
                        if d == 1 or d == true then cb() end
                    end,
                    'SISMEMBER',
                    {'tg:enabled_features', feature}
                )
            end
        end,
        'HGET',
        {chat_key, field}
    )
end

-- Utility to break text into words
function M.break_to_words(str)
    local t = {}
    for w in str:gmatch("%w+") do
        t[#t+1] = w
    end
    return t
end

-- Safe string conversion
function M.safe_str(val)
    return tostring(val or "")
end

-- Safe number conversion
function M.safe_num(val, default)
    return tonumber(val) or (default or 0)
end

-- Get user and chat IDs from task
function M.get_user_chat_ids(task)
    local user_id = M.safe_str(task:get_header('X-Telegram-User', true))
    local chat_id = M.safe_str(task:get_header('X-Telegram-Chat', true))
    return user_id, chat_id
end

-- Get message text from task
function M.get_message_text(task)
    return M.safe_str(task:get_rawbody())
end

-- Log error with context
function M.log_error(task, context, err)
    rspamd_logger.errx(task, '%1 received error: %2', context, err)
end

return M 