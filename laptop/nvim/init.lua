-- =============================================
--   Neovim Config - TokyoNight + JetBrains Mono
-- =============================================

-- Bootstrap lazy.nvim (plugin manager)
local lazypath = vim.fn.stdpath("data") .. "/lazy/lazy.nvim"
if not vim.loop.fs_stat(lazypath) then
  vim.fn.system({
    "git", "clone", "--filter=blob:none",
    "https://github.com/folke/lazy.nvim.git",
    "--branch=stable", lazypath,
  })
end
vim.opt.rtp:prepend(lazypath)

-- =============================================
--   Opciones generales
-- =============================================
vim.opt.number         = true          -- Números de línea
vim.opt.relativenumber = true          -- Números relativos (útil para saltar líneas)
vim.opt.tabstop        = 2             -- Tab = 2 espacios
vim.opt.shiftwidth     = 2
vim.opt.expandtab      = true          -- Usar espacios en vez de tabs
vim.opt.smartindent    = true          -- Indentación inteligente
vim.opt.wrap           = false         -- Sin wrap de líneas
vim.opt.termguicolors  = true          -- Colores 24-bit
vim.opt.signcolumn     = "yes"         -- Columna para iconos de git/errores
vim.opt.cursorline     = true          -- Resaltar línea actual
vim.opt.scrolloff      = 8             -- Margen de 8 líneas al scrollear
vim.opt.clipboard      = "unnamedplus" -- Compartir clipboard con el sistema
vim.opt.ignorecase     = true          -- Búsqueda sin distinción de mayúsculas
vim.opt.smartcase      = true          -- ...salvo que escribas mayúsculas
vim.opt.splitright     = true          -- Splits verticales a la derecha
vim.opt.splitbelow     = true          -- Splits horizontales abajo

-- Fuente (para GUIs, en kitty se configura en kitty.conf)
vim.opt.guifont = "JetBrainsMono Nerd Font:h13"

-- Leader key = Espacio
vim.g.mapleader = " "
vim.g.maplocalleader = " "

-- =============================================
--   Plugins
-- =============================================
require("lazy").setup({

  -- 🎨 Tema TokyoNight
  {
    "folke/tokyonight.nvim",
    priority = 1000,
    config = function()
      require("tokyonight").setup({
        style = "night",          -- night | storm | moon | day
        transparent = false,
        terminal_colors = true,
        styles = {
          comments = { italic = true },
          keywords = { italic = true },
          functions = {},
          variables = {},
        },
      })
      vim.cmd.colorscheme("tokyonight-night")
    end,
  },

  -- 📊 Barra de estado (statusline)
  {
    "nvim-lualine/lualine.nvim",
    dependencies = { "nvim-tree/nvim-web-devicons" },
    config = function()
      require("lualine").setup({
        options = {
          theme = "tokyonight",
          component_separators = "|",
          section_separators = { left = "", right = "" },
        },
      })
    end,
  },

  -- 🌲 Explorador de archivos
  {
    "nvim-tree/nvim-tree.lua",
    dependencies = { "nvim-tree/nvim-web-devicons" },
    config = function()
      vim.g.loaded_netrw = 1
      vim.g.loaded_netrwPlugin = 1
      require("nvim-tree").setup({
        view = { width = 30 },
        renderer = {
          group_empty = true,
          icons = { show = { git = true, file = true, folder = true } },
        },
        filters = { dotfiles = false },
      })
    end,
  },


-- 🌳 Resaltado de sintaxis (Treesitter)
  {
    "nvim-treesitter/nvim-treesitter",
    build = ":TSUpdate",
    config = function()
      -- Intentamos cargar el módulo de forma segura
      local status_ok, configs = pcall(require, "nvim-treesitter.configs")

      -- Si el plugin no está descargado aún, salimos sin lanzar el error rojo
      if not status_ok then
        return
      end

      -- Si ya está instalado, aplicamos la configuración
      configs.setup({
        ensure_installed = {
          "lua", "python", "javascript", "typescript",
          "bash", "json", "html", "css", "markdown",
        },
        highlight = { enable = true },
        indent    = { enable = true },
      })
    end,
  },

  -- 🔧 LSP (autocompletado e inteligencia de código)
  {
    "neovim/nvim-lspconfig",
    dependencies = {
      "williamboman/mason.nvim",          -- Instalador de LSPs
      "williamboman/mason-lspconfig.nvim",
    },
    config = function()
      require("mason").setup({
        ui = { border = "rounded" },
      })
      require("mason-lspconfig").setup({
        ensure_installed = { "lua_ls", "pyright", "ts_ls" },
        automatic_installation = true,
      })

      local lspconfig = require("lspconfig")
      local capabilities = require("cmp_nvim_lsp").default_capabilities()

      lspconfig.pyright.setup({ capabilities = capabilities })  -- Python
      lspconfig.ts_ls.setup({ capabilities = capabilities })    -- JS/TS
      lspconfig.lua_ls.setup({                                   -- Lua
        capabilities = capabilities,
        settings = { Lua = { diagnostics = { globals = { "vim" } } } },
      })
    end,
  },

  -- ✅ Autocompletado
  {
    "hrsh7th/nvim-cmp",
    dependencies = {
      "hrsh7th/cmp-nvim-lsp",
      "hrsh7th/cmp-buffer",
      "hrsh7th/cmp-path",
      "L3MON4D3/LuaSnip",
      "saadparwaiz1/cmp_luasnip",
      "rafamadriz/friendly-snippets",
    },
    config = function()
      local cmp     = require("cmp")
      local luasnip = require("luasnip")
      require("luasnip.loaders.from_vscode").lazy_load()

      cmp.setup({
        snippet = {
          expand = function(args) luasnip.lsp_expand(args.body) end,
        },
        window = {
          completion    = cmp.config.window.bordered(),
          documentation = cmp.config.window.bordered(),
        },
        mapping = cmp.mapping.preset.insert({
          ["<C-Space>"] = cmp.mapping.complete(),
          ["<CR>"]      = cmp.mapping.confirm({ select = true }),
          ["<Tab>"]     = cmp.mapping(function(fallback)
            if cmp.visible() then cmp.select_next_item()
            elseif luasnip.expand_or_jumpable() then luasnip.expand_or_jump()
            else fallback() end
          end, { "i", "s" }),
          ["<S-Tab>"]   = cmp.mapping(function(fallback)
            if cmp.visible() then cmp.select_prev_item()
            elseif luasnip.jumpable(-1) then luasnip.jump(-1)
            else fallback() end
          end, { "i", "s" }),
        }),
        sources = cmp.config.sources({
          { name = "nvim_lsp" },
          { name = "luasnip" },
          { name = "buffer" },
          { name = "path" },
        }),
      })
    end,
  },

  -- 🔭 Telescope (buscador de archivos y texto)
  {
    "nvim-telescope/telescope.nvim",
    dependencies = { "nvim-lua/plenary.nvim" },
    config = function()
      require("telescope").setup({
        defaults = {
          layout_strategy = "horizontal",
          sorting_strategy = "ascending",
        },
      })
    end,
  },

  -- 🔗 Auto cierre de paréntesis y comillas
  {
    "windwp/nvim-autopairs",
    event = "InsertEnter",
    config = function()
      require("nvim-autopairs").setup()
    end,
  },

  -- 📝 Comentarios rápidos
  {
    "numToStr/Comment.nvim",
    config = function()
      require("Comment").setup()
    end,
  },

  -- 📏 Líneas de indentación
  {
    "lukas-reineke/indent-blankline.nvim",
    main = "ibl",
    config = function()
      require("ibl").setup({
        indent = { char = "│" },
        scope  = { enabled = true },
      })
    end,
  },

  -- 🔖 Marcadores de Git en el gutter
  {
    "lewis6991/gitsigns.nvim",
    config = function()
      require("gitsigns").setup({
        signs = {
          add    = { text = "▎" },
          change = { text = "▎" },
          delete = { text = "" },
        },
      })
    end,
  },

  -- 🏷️ Pestañas de buffers
  {
    "akinsho/bufferline.nvim",
    dependencies = "nvim-tree/nvim-web-devicons",
    config = function()
      require("bufferline").setup({
        options = {
          diagnostics = "nvim_lsp",
          separator_style = "slant",
        },
      })
    end,
  },

  -- 🔔 Notificaciones bonitas
  {
    "folke/noice.nvim",
    dependencies = { "MunifTanjim/nui.nvim", "rcarriga/nvim-notify" },
    config = function()
      require("noice").setup({
        lsp = {
          override = {
            ["vim.lsp.util.convert_input_to_markdown_lines"] = true,
            ["vim.lsp.util.stylize_markdown"] = true,
          },
        },
      })
    end,
  },

})

-- =============================================
--   Atajos de teclado (Keymaps)
-- =============================================
local map = vim.keymap.set
local opts = { noremap = true, silent = true }

-- Explorador de archivos
map("n", "<leader>e", ":NvimTreeToggle<CR>",   { desc = "Abrir/cerrar explorador" })
map("n", "<leader>ef", ":NvimTreeFocus<CR>",   { desc = "Enfocar explorador" })

-- Telescope
map("n", "<leader>ff", ":Telescope find_files<CR>",  { desc = "Buscar archivos" })
map("n", "<leader>fg", ":Telescope live_grep<CR>",   { desc = "Buscar texto en proyecto" })
map("n", "<leader>fb", ":Telescope buffers<CR>",     { desc = "Ver buffers abiertos" })
map("n", "<leader>fh", ":Telescope help_tags<CR>",   { desc = "Buscar ayuda" })

-- Navegación entre ventanas (splits)
map("n", "<C-h>", "<C-w>h", { desc = "Ventana izquierda" })
map("n", "<C-j>", "<C-w>j", { desc = "Ventana abajo" })
map("n", "<C-k>", "<C-w>k", { desc = "Ventana arriba" })
map("n", "<C-l>", "<C-w>l", { desc = "Ventana derecha" })

-- Guardar y salir
map("n", "<leader>w", ":w<CR>",  { desc = "Guardar" })
map("n", "<leader>q", ":q<CR>",  { desc = "Salir" })
map("n", "<leader>Q", ":qa!<CR>",{ desc = "Salir sin guardar" })

-- Buffers (pestañas)
map("n", "<S-l>", ":bnext<CR>",     { desc = "Buffer siguiente" })
map("n", "<S-h>", ":bprevious<CR>", { desc = "Buffer anterior" })
map("n", "<leader>x", ":bdelete<CR>", { desc = "Cerrar buffer" })

-- Mover líneas en modo visual
map("v", "J", ":m '>+1<CR>gv=gv", { desc = "Mover selección abajo" })
map("v", "K", ":m '<-2<CR>gv=gv", { desc = "Mover selección arriba" })

-- Centrar pantalla al navegar
map("n", "<C-d>", "<C-d>zz", opts)
map("n", "<C-u>", "<C-u>zz", opts)
map("n", "n",     "nzzzv",   opts)
map("n", "N",     "Nzzzv",   opts)

-- LSP (cuando estés sobre código)
map("n", "gd",        vim.lsp.buf.definition,    { desc = "Ir a definición" })
map("n", "gr",        vim.lsp.buf.references,    { desc = "Ver referencias" })
map("n", "K",         vim.lsp.buf.hover,         { desc = "Documentación" })
map("n", "<leader>rn",vim.lsp.buf.rename,        { desc = "Renombrar símbolo" })
map("n", "<leader>ca",vim.lsp.buf.code_action,   { desc = "Acción de código" })
map("n", "<leader>d", vim.diagnostic.open_float, { desc = "Ver error actual" })
map("n", "[d",        vim.diagnostic.goto_prev,  { desc = "Error anterior" })
map("n", "]d",        vim.diagnostic.goto_next,  { desc = "Error siguiente" })

-- Comentar línea/selección: gcc (normal) | gc (visual)
-- (viene de Comment.nvim, sin config extra)

-- Limpiar búsqueda resaltada
map("n", "<Esc>", ":nohlsearch<CR>", opts)
