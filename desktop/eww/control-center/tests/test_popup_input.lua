-- Run real callbacks against a fake compositor, with no graphical session.
local install = dofile(arg[1])
local binds, commands = {}, {}
local cursor = { x = 500, y = 500 }
local panels = {}
hl = {
    bind = function(key, callback, options)
        local binding = { key = key, callback = callback, options = options, removed = false }
        function binding:remove() self.removed = true end
        binds[#binds + 1] = binding
        return binding
    end,
    exec_cmd = function(command) commands[#commands + 1] = command end,
    get_cursor_pos = function() return cursor end,
    get_layers = function(filter)
        assert(filter.namespace == 'waybar-control-center')
        return panels
    end,
}
local function click(key)
    for _, binding in ipairs(binds) do
        if not binding.removed and binding.key == key then binding.callback() end
    end
end
install('dismiss first')
assert(#binds == 4)
panels = {{ mapped = true, x = 100, y = 50, w = 380, h = 560 }}
cursor = { x = 150, y = 100 }
click('mouse:272')
assert(#commands == 0, 'Click inside must preserve the popup')
-- On Waybar, on the desktop, in an application or on another monitor.
for _, position in ipairs({{ x = 150, y = 25 }, { x = 500, y = 800 }, { x = -500, y = 25 }}) do
    cursor = position
    for _, key in ipairs({'mouse:272', 'mouse:273', 'mouse:274'}) do click(key) end
end
assert(#commands == 9)
for _, binding in ipairs(binds) do
    if binding.key ~= 'Escape' then
        assert(binding.options.non_consuming, 'The original click must reach the app/bar')
        assert(binding.options.ignore_mods)
    end
end
click('Escape')
assert(#commands == 10 and commands[10] == 'dismiss first')
local original = binds
install('dismiss second')
for i = 1, 4 do assert(original[i].removed) end
click('Escape')
assert(#commands == 11 and commands[11] == 'dismiss second')
install('')
click('Escape')
click('mouse:272')
assert(#commands == 11, 'No binds should remain after closing')
-- Reloading/cleanup must preserve unrelated user binds.
assert(#binds == 8)
print('Popup input callbacks: OK')
