--[[
  Telegram Reply-Aware Filtering Module for Rspamd
  
  This module implements reply-aware filtering similar to Rspamd's Replies module
  but specifically designed for Telegram messages. It detects when a message is
  a reply to a trusted message (bot, admin, or verified user) and applies
  appropriate score reductions.
  
  Features:
  - Detects In-Reply-To headers in Telegram messages
  - Checks if the replied-to message is trusted
  - Applies different score reductions based on trust level
  - Tracks reply patterns for spam detection
]]

local rspamd_logger = require "rspamd_logger"
local lua_redis = require "lua_redis"
local lua_util = require "lua_util"

-- Module configuration
local config = {
    -- Redis configuration
    redis_prefix = "tg:trusted:",
    redis_replies_prefix = "tg:replies:",
    
    -- Score reductions for different types of trusted replies
    score_reductions = {
        bot = -3.0,      -- Highest trust for bot messages
        admin = -2.0,    -- Medium trust for admin messages
        verified = -1.0, -- Lower trust for verified users
        regular = 0.0,   -- No reduction for regular replies
    },
    
    -- TTL for trusted message tracking (24 hours)
    trusted_ttl = 86400,
    
    -- TTL for reply tracking (7 days)
    reply_ttl = 604800,
    
    -- Maximum score reduction to prevent abuse
    max_score_reduction = -5.0,
}

-- Initialize Redis connection
local redis_params = lua_redis.parse_redis_server('telegram')
if not redis_params then
    rspamd_logger.errx(rspamd_config, 'Failed to parse Redis server for telegram replies module')
    return
end

-- Helper function to extract message ID from In-Reply-To header
local function extract_message_id_from_header(header_value)
    if not header_value then
        return nil
    end
    
    -- Parse In-Reply-To header format: <type.message_id.chat_id@telegram.com>
    -- Examples: <bot.123.456@telegram.com>, <admin.789.101@telegram.com>
    local message_type, message_id, chat_id = header_value:match("<([^.]+)%.([^.]+)%.([^.]+)@telegram%.com>")
    if message_type and message_id and chat_id then
        return {
            type = message_type,
            message_id = message_id,
            chat_id = chat_id
        }
    end
    
    -- Fallback for other formats
    local simple_id = header_value:match("<([^>]+)>")
    if simple_id then
        return {
            type = "unknown",
            message_id = simple_id,
            chat_id = "unknown"
        }
    end
    
    return nil
end

-- Check if a message ID is trusted in Redis
local function is_message_trusted(task, message_id)
    if not message_id then
        return false
    end
    
    local key = config.redis_prefix .. message_id
    
    local function trusted_cb(err, data)
        if err then
            rspamd_logger.errx(task, 'Failed to check trusted message %1: %2', message_id, err)
            return false
        end
        
        if data and data > 0 then
            -- Get metadata to determine trust type
            local metadata_key = key .. ":metadata"
            
            local function metadata_cb(_err, _data)
                if _err then
                    rspamd_logger.errx(task, 'Failed to get metadata for message %1: %2', message_id, _err)
                    return "unknown"
                end
                
                if _data then
                    return _data
                else
                    return "unknown"
                end
            end
            
            lua_redis.redis_make_request(task,
                redis_params,
                metadata_key,
                false, -- is write
                metadata_cb,
                'HGET',
                {metadata_key, 'trusted_type'}
            )
            
            return true
        end
        
        return false
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        key,
        false, -- is write
        trusted_cb,
        'EXISTS',
        {key}
    )
    
    return false -- Default return value
end

-- Main callback function for reply detection
local function telegram_reply_callback(task)
    local headers = task:get_headers()
    local in_reply_to = headers:get("In-Reply-To")
    
    if not in_reply_to then
        return -- Not a reply
    end
    
    -- Extract message information from In-Reply-To header
    local reply_info = extract_message_id_from_header(in_reply_to)
    if not reply_info then
        rspamd_logger.debugx(task, "Invalid In-Reply-To header format: %s", in_reply_to)
        return
    end
    
    rspamd_logger.debugx(task, "Processing reply to message %s (type: %s)", 
                        reply_info.message_id, reply_info.type)
    
    -- Check if this is a reply to a trusted message
    local key = config.redis_prefix .. reply_info.message_id
    
    local function trusted_cb(err, data)
        if err then
            rspamd_logger.errx(task, 'Failed to check trusted message %1: %2', reply_info.message_id, err)
            return
        end
        
        if data and data > 0 then
            -- This is a reply to a trusted message
            local score_reduction = config.score_reductions[reply_info.type] or 0.0
            
            -- Apply score reduction based on trust type
            if score_reduction < 0 then
                task:insert_result('TG_REPLY', score_reduction)
                
                -- Add specific symbol based on trust type
                if reply_info.type == "bot" then
                    task:insert_result('TG_REPLY_BOT', score_reduction)
                elseif reply_info.type == "admin" then
                    task:insert_result('TG_REPLY_ADMIN', score_reduction)
                elseif reply_info.type == "verified" then
                    task:insert_result('TG_REPLY_VERIFIED', score_reduction)
                end
                
                rspamd_logger.debugx(task, "Applied reply score reduction: %s for type: %s", 
                                    score_reduction, reply_info.type)
            end
            
            -- Track this reply in Redis for future reference
            local reply_key = string.format("%s%s:%s:%s", 
                                          config.redis_replies_prefix,
                                          reply_info.chat_id,
                                          reply_info.message_id,
                                          task:get_message_id())
            
            lua_redis.redis_make_request(task,
                redis_params,
                reply_key,
                true, -- is write
                function() end,
                'SETEX',
                {reply_key, config.reply_ttl, "1"}
            )
            
        else
            -- Regular reply (not to a trusted message)
            rspamd_logger.debugx(task, "Regular reply detected (not to trusted message)")
            
            -- Optional: Add a small penalty for replies to non-trusted messages
            -- This can help detect spam attempts that reply to random messages
            local regular_reply_score = config.score_reductions.regular or 0.0
            if regular_reply_score > 0 then
                task:insert_result('TG_REPLY', regular_reply_score)
            end
        end
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        key,
        false, -- is write
        trusted_cb,
        'EXISTS',
        {key}
    )
end

-- Callback for checking if a message is a reply to a trusted message
local function telegram_reply_check_callback(task)
    local headers = task:get_headers()
    local in_reply_to = headers:get("In-Reply-To")
    
    if not in_reply_to then
        return -- Not a reply
    end
    
    local reply_info = extract_message_id_from_header(in_reply_to)
    if not reply_info then
        return
    end
    
    -- Check if this message is tracked as a reply to a trusted message
    local reply_key = string.format("%s%s:*:%s", 
                                  config.redis_replies_prefix,
                                  reply_info.chat_id,
                                  task:get_message_id())
    
    local function reply_check_cb(err, data)
        if err then
            rspamd_logger.errx(task, 'Failed to check reply tracking: %1', err)
            return
        end
        
        if data and #data > 0 then
            -- This message is tracked as a reply to a trusted message
            task:insert_result('TG_REPLY_TRACKED', -1.0)
            rspamd_logger.debugx(task, "Message is tracked reply to trusted message")
        end
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        reply_key,
        false, -- is write
        reply_check_cb,
        'KEYS',
        {reply_key}
    )
end

-- Anti-evasion callback: detect spam patterns in replies
local function telegram_reply_spam_check_callback(task)
    local headers = task:get_headers()
    local in_reply_to = headers:get("In-Reply-To")
    
    if not in_reply_to then
        return
    end
    
    local reply_info = extract_message_id_from_header(in_reply_to)
    if not reply_info then
        return
    end
    
    -- Check if this is a reply to a trusted message
    local key = config.redis_prefix .. reply_info.message_id
    
    local function spam_check_cb(err, data)
        if err then
            rspamd_logger.errx(task, 'Failed to check trusted message for spam: %1', err)
            return
        end
        
        if data and data > 0 then
            -- Check for spam patterns in the reply content
            local text = task:get_text()
            if text then
                local spam_indicators = {
                    -- Excessive links
                    {"https?://[%w%.%-]+", 3, "TG_REPLY_LINK_SPAM"},
                    -- Phone numbers
                    {"%+?%d[%d%-%s%(%)]%d%d%d%d", 2, "TG_REPLY_PHONE_SPAM"},
                    -- Invite links
                    {"t%.me/joinchat", 4, "TG_REPLY_INVITE_SPAM"},
                    -- Excessive caps
                    {"[A-Z]", 0.5, "TG_REPLY_CAPS_SPAM"},
                }
                
                for _, pattern in ipairs(spam_indicators) do
                    local regex, threshold, symbol = table.unpack(pattern)
                    local count = 0
                    
                    for _ in text:gmatch(regex) do
                        count = count + 1
                    end
                    
                    if count >= threshold then
                        local score = math.min(count * 0.5, 5.0) -- Cap at 5.0
                        task:insert_result(symbol, score)
                        rspamd_logger.debugx(task, "Spam pattern detected in reply: %s (count: %d)", 
                                            symbol, count)
                    end
                end
            end
        end
    end
    
    lua_redis.redis_make_request(task,
        redis_params,
        key,
        false, -- is write
        spam_check_cb,
        'EXISTS',
        {key}
    )
end

-- Register the symbols
rspamd_config.TG_REPLY = {
    callback = telegram_reply_callback,
    description = 'Reply to trusted message',
    score = -2.0,
    group = 'telegram_replies'
}

rspamd_config.TG_REPLY_BOT = {
    callback = telegram_reply_callback,
    description = 'Reply to bot message',
    score = -3.0,
    group = 'telegram_replies'
}

rspamd_config.TG_REPLY_ADMIN = {
    callback = telegram_reply_callback,
    description = 'Reply to admin message',
    score = -2.0,
    group = 'telegram_replies'
}

rspamd_config.TG_REPLY_VERIFIED = {
    callback = telegram_reply_callback,
    description = 'Reply to verified user message',
    score = -1.0,
    group = 'telegram_replies'
}

rspamd_config.TG_REPLY_TRACKED = {
    callback = telegram_reply_check_callback,
    description = 'Message is tracked reply to trusted message',
    score = -1.0,
    group = 'telegram_replies'
}

rspamd_config.TG_REPLY_LINK_SPAM = {
    callback = telegram_reply_spam_check_callback,
    description = 'Excessive links in reply to trusted message',
    score = 2.0,
    group = 'telegram_replies'
}

rspamd_config.TG_REPLY_PHONE_SPAM = {
    callback = telegram_reply_spam_check_callback,
    description = 'Phone number spam in reply to trusted message',
    score = 1.5,
    group = 'telegram_replies'
}

rspamd_config.TG_REPLY_INVITE_SPAM = {
    callback = telegram_reply_spam_check_callback,
    description = 'Invite link spam in reply to trusted message',
    score = 3.0,
    group = 'telegram_replies'
}

rspamd_config.TG_REPLY_CAPS_SPAM = {
    callback = telegram_reply_spam_check_callback,
    description = 'Excessive caps in reply to trusted message',
    score = 1.0,
    group = 'telegram_replies'
}

-- Log that symbols are registered
rspamd_logger.infox(rspamd_config, 'Telegram replies symbols registered: TG_REPLY, TG_REPLY_BOT, TG_REPLY_ADMIN, TG_REPLY_VERIFIED, TG_REPLY_TRACKED, TG_REPLY_LINK_SPAM, TG_REPLY_PHONE_SPAM, TG_REPLY_INVITE_SPAM, TG_REPLY_CAPS_SPAM') 