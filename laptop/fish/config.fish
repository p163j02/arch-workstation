if status is-interactive
  # Commands to run in interactive sessions can go here
  # El logo y el marco necesitan espacio; usar una vista compacta en ventanas divididas.
  if test "$COLUMNS" -ge 100
    fastfetch
  else
    fastfetch --logo none --structure title:os:uptime:shell:terminal:memory
  end

  # Reemplazar ls por eza con iconos y soporte Git
  alias ls="eza --icons --git"
  alias ll="eza -lh --icons --git"
  alias la="eza -la --icons --git"
  alias tree="eza --tree --icons --ignore-glob='node_modules|.git'"

  # Alias corto para lazygit
  alias lg="lazygit"

  function qgis
    QT_QPA_PLATFORM=xcb command qgis $argv
  end

  function y
    set tmp (mktemp -t "yazi-cwd.XXXXXX")
    yazi $argv --cwd-file="$tmp"
    if set cwd (command cat -- "$tmp"); and [ -n "$cwd" ]; and [ "$cwd" != "$PWD" ]
      builtin cd -- "$cwd"
    end
    rm -f -- "$tmp"
  end

end
#source /usr/share/nvm/init-nvm.sh


# Added by Antigravity CLI installer
fish_add_path "$HOME/.local/bin"

# bun
set --export BUN_INSTALL "$HOME/.bun"
set --export PATH $BUN_INSTALL/bin $PATH

# pnpm
set -gx PNPM_HOME "$HOME/.local/share/pnpm"
fish_add_path "$PNPM_HOME"
# pnpm end
