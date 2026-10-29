if status is-interactive
  # Commands to run in interactive sessions can go here
  alias npm="pnpm"
  fastfetch

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
set -gx PATH "@HOME@/.local/bin" $PATH

# bun
set --export BUN_INSTALL "$HOME/.bun"
set --export PATH $BUN_INSTALL/bin $PATH

# pnpm
set -gx PNPM_HOME '@HOME@/.local/share/pnpm'
if test "$PATH[1]" != "$PNPM_HOME/bin"
  set -gx PATH "$PNPM_HOME/bin" $PATH
end
# pnpm end
