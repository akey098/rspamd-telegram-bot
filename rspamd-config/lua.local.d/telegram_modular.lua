--[[
  Telegram Bot Rspamd Rules - Modular Entry Point
  
  This file loads the modular telegram rules structure.
  It replaces the monolithic telegram.lua with a cleaner, 
  more maintainable modular approach.
]]

-- Load the modular telegram rules
require "lua.local.d.telegram.init" 