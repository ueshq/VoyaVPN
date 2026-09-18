# GitHub Primer token 映射(vendored from @primer/primitives@11.9.0)

本表是 `packages/ui/src/styles/globals.css` 中 token 值的来源记录。所有色值取自
`@primer/primitives@11.9.0` 的 `dist/docs/functional/themes/{light,dark}.json`
(GitHub Primer 官方 functional tokens)。升级 Primer 时重新 vendor 并更新本表,
不要凭目测微调。

策略:保留既有语义 token 名称与 `.dark` class 主题机制,仅替换值;
`var()` 别名(`--surface`、`--signal`、`--sidebar-*` 等)保持别名不动。

## shadcn 槽位

| 我方 token | Primer token | Light | Dark |
|---|---|---|---|
| `--background` / `--surface` | bgColor-default | `#ffffff` | `#0d1117` |
| `--foreground` / `--text-heading` / `--icon` | fgColor-default | `#1f2328` | `#f0f6fc` |
| `--card` / `--popover` / `--surface-dialog` | bgColor-muted(dark 抬升) | `#ffffff` | `#151b23` |
| `--primary`(纯填充) | bgColor-accent-emphasis | `#0969da` | `#1f6feb` |
| `--secondary` / `--surface-button` | button-default-bgColor-rest | `#f6f8fa` | `#212830` |
| `--muted` / `--surface-sunken` / `--sidebar`(light) | bgColor-muted | `#f6f8fa` | `#151b23` |
| `--muted-foreground` / `--text-subtle` | fgColor-muted | `#59636e` | `#9198a1` |
| `--accent`(hover 槽)/ `--surface-hovered` | control-transparent-bgColor-hover | `rgb(129 139 152/.1)` | `rgb(101 108 118/.2)` |
| `--surface-pressed` | control-transparent-bgColor-active | `rgb(129 139 152/.15)` | `rgb(101 108 118/.25)` |
| `--destructive` / `--bg-danger-bold` | bgColor-danger-emphasis | `#cf222e` | `#da3633` |
| `--border` / `--input` / `--border-dialog` | borderColor-default | `#d1d9e0` | `#3d444d` |
| `--ring` / `--border-focused` / `--border-selected` | focus-outlineColor | `#0969da` | `#1f6feb` |

## 语义家族

| 我方 token | Primer token | Light | Dark |
|---|---|---|---|
| `--text-subtlest` / `--text-disabled` | fgColor-disabled | `#818b98` | `#656c76` |
| `--text-brand` / `--link` / `--icon-brand` | fgColor-accent | `#0969da` | `#4493f8` |
| `--text-danger` / `--icon-danger` / `--border-danger` | fgColor-danger | `#d1242f` | `#f85149` |
| `--text-success` / `--icon-success` | fgColor-success | `#1a7f37` | `#3fb950` |
| `--text-warning` / `--icon-warning` | fgColor-attention | `#9a6700` | `#d29922` |
| `--text-discovery` | fgColor-done | `#8250df` | `#ab7df8` |
| `--border-bold` | borderColor-emphasis | `#818b98` | `#656c76` |
| `--border-subtle` | borderColor-muted | `rgb(209 217 224/.7)` | `rgb(61 68 77/.7)` |
| `--bg-danger` | bgColor-danger-muted | `#ffebe9` | `rgb(248 81 73/.1)` |
| `--bg-success` | bgColor-success-muted | `#dafbe1` | `rgb(46 160 67/.15)` |
| `--bg-warning` | bgColor-attention-muted | `#fff8c5` | `rgb(187 128 9/.15)` |
| `--bg-warning-bold` | bgColor-attention-emphasis | `#9a6700` | `#9e6a03` |
| `--bg-information` / `--bg-selected` | bgColor-accent-muted | `#ddf4ff` | `rgb(56 139 253/.1)` |
| `--bg-selected-strong` | base blue-2 / accent-muted 加倍 | `#b6e3ff` | `rgb(56 139 253/.25)` |
| `--bg-discovery` | bgColor-done-muted | `#fbefff` | `rgb(171 125 248/.15)` |
| `--blanket` | overlay-backdrop-bgColor | `rgb(200 209 218/.4)` | `rgb(33 40 48/.4)` |
| `--sidebar`(dark) | 近 bgColor-inset | `#f6f8fa` | `#010409` |

## VPN 域 + accent 调色板

| 我方 token | Primer token | Light | Dark |
|---|---|---|---|
| `--connected` | bgColor-success-emphasis | `#1f883d` | `#238636` |
| `--accent-blue` | fgColor-accent | `#0969da` | `#4493f8` |

`-light`/`-bg` 变体沿用原 alpha、换新基色重算。

## 其他

- 阴影:`--elevation-*` 取 Primer shadow-resting/floating 家族形制(light 墨
  `rgb(31 35 40)` / `rgb(140 149 159)`,dark 墨 `rgb(1 4 9)`,dark overlay 带
  1px `#3d444d` spread ring);`--shadow-sm/md/lg` 仅重着墨色、保持原 alpha。
- 圆角:`--radius` 7px→6px;`--radius-md/lg` 统一 6px,`--radius-xl` 12px
  (GitHub 6px 制)。
- 字体:正文与 display 固定使用 GitHub 系统栈(`-apple-system, BlinkMacSystemFont,
  "Segoe UI", "Noto Sans", …` + CJK 回退)，不再提供运行时字体族或字号设置；
  mono 栈本就与 GitHub 一致未变。
- Acrylic:CSS 面纱 light `rgb(246 248 250/.8)` / dark `rgb(1 4 9/.8)`,
  reduced-transparency 回退 `#f6f8fa`/`#010409`,与 window.rs 原生 tint
  `Color(246,248,250,200)`/`Color(1,4,9,200)` 保持同步;
  `--sidebar: transparent` 单面纱不变式未动。
- `--primary` 为纯填充色:文本消费点(sidebar 选中行、modal 选中项、
  update 表状态、button link variant)已改指 `text-brand`/`text-link`。
