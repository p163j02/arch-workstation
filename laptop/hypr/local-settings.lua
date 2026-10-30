-- Bridge for existing monitor settings, local overrides and Wallpaper Studio borders.
-- New Lua customizations belong in local.lua, loaded after these settings.
local settings = {}

local function trim(text)
    return text:match("^%s*(.-)%s*$")
end

local function scalar(text)
    if text == "true" or text == "yes" then return true end
    if text == "false" or text == "no" then return false end
    if text:match("^rgba%(%x+%)%s+rgba%(") then
        local colors = {}
        for color in text:gmatch("rgba%(%x+%)") do colors[#colors + 1] = color end
        return { colors = colors, angle = tonumber(text:match("(%d+)deg$")) or 0 }
    end
    return tonumber(text) or text
end

function settings.monitor(text)
    local parts = {}
    for part in (text .. ","):gmatch("(.-),") do parts[#parts + 1] = trim(part) end
    local spec = { output = parts[1] }
    if parts[2] == "disabled" or parts[2] == "disable" then
        spec.disabled = true
    else
        spec.mode, spec.position, spec.scale = parts[2], parts[3], scalar(parts[4] or "1")
        if parts[5] == "mirror" then spec.mirror = parts[6] end
    end
    hl.monitor(spec)
end

function settings.read(path)
    local file = io.open(path)
    if not file then return end
    local config, sections = {}, {}
    for raw in file:lines() do
        local line = trim(raw:gsub("%s*#.*$", ""))
        if line ~= "" then
            local section = line:match("^(%w+)%s*{$")
            if section then
                sections[#sections + 1] = section
            elseif line == "}" then
                assert(#sections > 0, "Unexpected closing brace in " .. path)
                sections[#sections] = nil
            else
                local key, text = line:match("^(.-)%s*=%s*(.*)$")
                assert(key, "Unsupported local setting in " .. path .. ": " .. line)
                key = trim(key)
                if key == "monitor" and #sections == 0 then
                    settings.monitor(text)
                else
                    local keys = {}
                    for _, name in ipairs(sections) do keys[#keys + 1] = name end
                    for name in key:gmatch("[^:.]+") do keys[#keys + 1] = name end
                    local target = config
                    for index = 1, #keys - 1 do
                        target[keys[index]] = target[keys[index]] or {}
                        target = target[keys[index]]
                    end
                    target[keys[#keys]] = scalar(trim(text))
                end
            end
        end
    end
    file:close()
    assert(#sections == 0, "Unclosed section in " .. path)
    if next(config) then hl.config(config) end
end

return settings
