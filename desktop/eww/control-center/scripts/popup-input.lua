-- Owned handles let us remove only our temporary binds, even after a restart.
return function(command)
    for _, binding in ipairs(_G.waybar_cc_bindings or {}) do
        binding:remove()
    end
    _G.waybar_cc_bindings = {}
    if command == "" then return end

    local function dismiss()
        hl.exec_cmd(command)
    end
    local function outside()
        -- Read position at button press, before the pointer or popup can move.
        local cursor = hl.get_cursor_pos()
        if not cursor then return end
        for _, panel in ipairs(hl.get_layers({ namespace = "waybar-control-center" })) do
            if panel.mapped and cursor.x >= panel.x and cursor.x < panel.x + panel.w
                and cursor.y >= panel.y and cursor.y < panel.y + panel.h then
                return
            end
        end
        dismiss()
    end

    local bindings = _G.waybar_cc_bindings
    bindings[#bindings + 1] = hl.bind("Escape", dismiss, {
        submap_universal = true,
        description = "Cerrar widget de Waybar",
    })
    for _, key in ipairs({ "mouse:272", "mouse:273", "mouse:274" }) do
        bindings[#bindings + 1] = hl.bind(key, outside, {
            non_consuming = true,
            ignore_mods = true,
            submap_universal = true,
            description = "Cerrar widget de Waybar al hacer clic afuera",
        })
    end
end
