--[[
  Telegram Bot Rspamd Rules - Modular Entry Point
  
  This file loads the simplified telegram rules structure.
  The complex modular structure has been replaced with a simpler
  approach that avoids module loading issues.
]]

-- Load the simple telegram rules
dofile(rspamd_paths["CONFDIR"] .. "/lua.local.d/telegram_simple.lua") 