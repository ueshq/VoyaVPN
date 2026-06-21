# VoyaVPN 界面优化方案（tasteskill 复审 · 代号 typed-valley）

> 本文是 `.agents/rollouts/ui-typed-valley/` 的上游证据来源（source of truth），等同审计整改里
> [docs/code-audit-2026-06.md](../code-audit-2026-06.md) 之于 `audit-remediation-2026-06`。
> 镜头：**界面 / UI / UX / 设计品味**（非安全/正确性，后者见上一轮审计整改）。
> 方法：3 个并行只读探查 agent（app-shell+设计系统 / 功能屏+UX 流 / i18n+a11y+一致性）＋我对高杠杆论断的**亲自核实**
> （读 `status-bar.tsx`、`globals.css`、`server-table.tsx`、locale 文件、grep i18n 覆盖）。凡标 ✅ 为亲自核实，📋 为探查结论。

## Context（为什么做）

VoyaVPN 是 Tauri 2 桌面代理/VPN 客户端（Clash/Xray/sing-box/mihomo 多核）。前端 React 19 + Vite +
Tailwind v4 + Radix/shadcn-ui（new-york、neutral 基色）+ lucide + i18next（8 语）+ TanStack
Query/Table/Virtual + react-hook-form/zod + zustand，源码 46 tsx / 33 ts。上一轮安全/正确性审计整改
（audit-remediation-2026-06）已收尾；本轮专审**界面**，产出可落地的优化方案。

## 总体结论 + 评分卡

**一句话**：工程底座扎实，但**界面停在「shadcn 默认基线」**——观感模板化、连接体验没有主视图、信息密度过载、
质量地板有洞、本地化有零散硬编码。这些维度恰好**都不在现有 lint/typecheck/bindings 门禁内**，于是一路绿灯长出来。

| 维度 | 评分 | 一句话 |
| --- | --- | --- |
| 设计系统合规（token/cn/CVA） | 8.5/10 | 几乎零硬编码 hex，data-slot/CVA 规整，底子好 |
| 可访问性地板 | 8/10 | focus-visible 3px ring、Radix 焦点陷阱、icon 多有 aria-label；缺 reduced-motion |
| 视觉识别 / signature | 3.5/10 | 纯 shadcn neutral/slate 默认，无品牌色、无 signature、字阶与阴影扁平 |
| 连接体验（VPN 核心） | 3/10 | 连接是 28px 图标按钮挤在 44px 底栏；无「是否受保护/连到哪/多快」主视图 |
| 信息密度 / 渐进披露 | 4.5/10 | profiles 14 列、connections 9 列，横向滚动、无列控制 |
| 状态 / 反馈一致性 | 5.5/10 | connections 有一流骨架屏，其余多为裸文字；反馈 inline/toast 混用 |
| 本地化完整度 | 6.5/10 | 8 语 833 键基建强；profiles 子域表单/菜单/测速按钮硬编码英文，locale 有英文泄漏 |

## 真实优点（整改中不得破坏）✅

- **token 合规度高**：颜色全走 `text-foreground/bg-card/border-border` 等语义 token，几乎无 `bg-[#..]` 硬编码。
- **a11y 基础**：`button.tsx` 全局 `focus-visible:ring-[3px]`、Radix 自带焦点陷阱、`IconButton` 强制 `aria-label`+`title`。
- **虚拟化到位**：profiles / connections 用 TanStack Virtual 扛千行。
- **connections 骨架屏**（`ConnectionSkeletonRows`）是全仓最佳加载态范本，应被推广。
- **i18n 基建**：8 语（en/zh-Hans/zh-Hant/fr/fa/hu/ru/de）、833 leaf key、fa 走 RTL、localStorage 持久化。
- **typed IPC 边界**（ADR 0002）：前端只经 `src/ipc/bindings.ts` 调后端，不写业务逻辑。

## 确认发现（按主题；每条带 `file:line` + 严重度）

### ① 连接体验无「英雄」(P0，最高杠杆) ✅
- 连接/断开/重启是 `size-7`(28px) 图标按钮，与 PID badge、代理模式 4 段切换、TUN、上下行速度 ~10 个控件挤在
  44px footer：[status-bar.tsx:196-264](../../src/components/app-shell/status-bar.tsx#L196-L264)。
- `profilesLabel = t("status.profiles", { count: 0 })` —— **count 硬编码 0**，永不反映真实档案数：
  [status-bar.tsx:108](../../src/components/app-shell/status-bar.tsx#L108)。
- 全应用**无任何视图回答 VPN 客户端最该一眼回答的三件事**：是否受保护 / 连到哪个节点·地区 / 现在多快、连了多久。
  应用读起来像「配置管理器」而非「VPN 客户端」。

### ② 观感模板化、无品牌 signature (P1) ✅
- 配色纯 shadcn neutral/slate：`--primary` ≈ `#0f172b`、neutrals 全 slate、`--chart-1..5` 是 shadcn 默认
  chart 调色板，**无品牌强调色**：[globals.css:7-70](../../src/styles/globals.css#L7-L70)。
- 三个「字体」是 Inter/Manrope/system 三个**可互换 sans**，无 display/body/mono 角色分工；字阶仅 xs/sm/base/lg；
  阴影仅 `shadow-xs/sm`，层次扁平。典型「shadcn baseline，工程好但没设计」。

### ③ 信息密度过载、缺渐进披露 (P1) ✅📋
- profiles 表 14 列、强制横向滚动、重度截断、**无列可见性控制**：[server-table.tsx](../../src/features/profiles/server-table.tsx)。
- connections 表 9 列、**只能过滤不能排序**：[clash-connections-screen.tsx](../../src/features/clash/clash-connections-screen.tsx)。

### ④ 质量地板有洞 (P1–P2) ✅
- **加载/空/错误态参差**：connections 有骨架屏，profiles 等多数屏仅居中裸文字。
- **反馈不一致**：成功时而 inline `<span>` 时而 toast，无统一心智。
- **破坏性操作无二次确认**：删除档案、还原备份直接执行；`src/components/ui/` 内**无 AlertDialog**。
- **无 `prefers-reduced-motion`**：✅ 全仓 grep 为 0；动效对所有用户无条件播放。
- **logs 未虚拟化**、无时间戳、无搜索/级别过滤：[logs-screen.tsx](../../src/features/logs/logs-screen.tsx)。
- **modal-in-modal**：group-builder 嵌在 profile-dialog 内。
- **断点过激**：代理模式/TUN/核心信息在 `md:`(768px) 以下 `hidden`，小窗口丢关键控件。

### ⑤ 本地化零散硬编码 + 一致性未系统化 (P2) ✅
- profiles 子域硬编码英文：表单标签 [profile-dialog.tsx:139](../../src/features/profiles/profile-dialog.tsx#L139)
  （Remarks/Protocol/Core/Port/Group…）、右键菜单与测速按钮
  [server-table.tsx:372-407](../../src/features/profiles/server-table.tsx#L372-L407)（Fast/TCP/Real/UDP/Speed/Mixed、
  Activate/Edit/Copy/Delete/Move…）、多处 `aria-label="…"` 未走 i18n；zh-Hans 等有英文泄漏。
- 各屏重复手写 `<section className="flex h-full min-h-0 flex-col"> + 12px header`，无共享页头原语；
  间距尺度 gap-1/1.5/2/3/4 自由混用，卡片底色 `bg-background` vs `bg-muted/30` 不成文。

## 纠正探查中的过度断言（透明度）

- ❌ 「profiles 整页硬编码英文」**不成立**：✅ grep 显示 profiles 子域有 **122 个 `t()`**，整体已本地化；
  真实缺口是**表单标签 / 右键菜单 / 测速按钮 / 部分 aria-label** 等零散硬编码（见发现 ⑤）。已降级为 P2。
- ⚠️ a11y 总体良好，唯一系统性缺口是 reduced-motion（发现 ④）。

## 优化方案 A —— Safe Passage / 航道（视觉识别重塑）

把品牌锚到产品本质：隐私工具是穿越敌意网络的一条**安全航道**。保留深色海军底（安全/夜用护眼），赋予唯一
**signature：UI 随连接状态点亮**——断开时全屏冷静 slate 单色，连接时品牌强调色「点亮」（Hero 灯标 + 活动节点勾选 +
主连接键辉光）。让「品牌观感 = 应用最重要的状态」，是一处可辩护的冒险，也避开 AI 默认三连（奶油+衬线+赤陶 /
近黑+酸绿 / 报纸细线）。

紧凑 token 系统（推荐起点；具体取值落地时定稿，遵循 frontend-design「先头脑风暴→自我批判→再编码」与 Chanel
「离家前减一件配饰」）：
- **色（4–6 具名）**：`--ink` 深海军画布（在现 `#020618` 上微提 chroma，让强调色读得出）；`--signal` 极光青绿
  ≈ `oklch(0.78 0.13 185)`（"受保护"色，**克制使用**：连接态/主连接键辉光/活动节点）；`--beacon` 暖琥珀
  （connecting/warning，可复用现 chart 暖色）；neutrals 沿用 slate；destructive 留红。浅色模式 `--signal` 取更深一档保对比度。
- **字（角色化）**：`display`=Manrope（Hero 计时/速率大字，tabular-nums）、`body`=Inter、**新增 `--font-mono`**
  （logs/connections/codemirror/数据单元格）。在 [fonts.ts](../../src/config/fonts.ts) 与 `@theme` 建立 display/body/mono 三角色。
- **深度/动效**：新增 `--shadow-sm/md/lg` 阶梯 + `--glow-signal`（连接灯标辉光）；一切动效置于
  `prefers-reduced-motion: reduce` 守卫下（reduce 时辉光脉动关闭）。
- 落点：`src/styles/globals.css` 的 `:root`/`.dark`/`@supports oklch`/`@theme inline` **四块同步改**；
  按需扩 `button.tsx`(signal/glow variant)、`card.tsx`(elevation)。

## 优化方案 B —— 连接主页 Hero（新默认视图）

新增 `src/features/home/home-screen.tsx`：首屏「英雄」= 大号连接/断开主键 + 连接状态灯标（点亮 signature）+
当前节点·地区·核心 + 实时上下行 + 已连时长 + 一眼「受保护 / 未保护」。接进 shell
（[app-shell.tsx](../../src/components/app-shell/app-shell.tsx) 的 `shellTabs` 增 `home` 并设默认；
`src/stores/shell-store.ts` 默认 `activeTab`）。**复用**既有 runtime 动作/状态
（`connectActiveProfile/disconnectCore/restartCore/useRuntimeEventStore`，见
[status-bar.tsx:20-32](../../src/components/app-shell/status-bar.tsx#L20-L32)），**不重写 IPC**。底栏
`StatusBar` 瘦身为次级状态条，并修 `profilesLabel` count:0 真缺陷（接真实档案数）。

## 改进路线图（映射到 rollout 各 phase）

| Phase | 主题 | 内容 |
| --- | --- | --- |
| 01-foundation | 视觉地基（A） | globals.css token 系统 + reduced-motion；fonts 三角色；button/card 深度 |
| 02-connect-home | Hero（B） | 连接主页 + 底栏瘦身 + count:0 修复 |
| 03-density | 渐进披露（C） | profiles 列可见性 + connections 排序/列开关 |
| 04-quality-floor | 质量地板（D） | AlertDialog 二次确认 + EmptyState/骨架 + logs 虚拟化/时间戳/过滤 + 断点回收 |
| 05-i18n-consistency | 本地化与一致性（E） | profiles i18n 补全 + locale 英文泄漏修复 + 共享页头原语 |

## 验证（每域最小集 → 全量兜底）

- 任意前端改动：`pnpm typecheck`、`pnpm lint`、`pnpm test --run`。
- IPC 不变性：`pnpm bindings:check`（不得改 typed IPC 契约）。
- CSS/token 编译：`pnpm build`（Tailwind v4 仅在构建期校验）。
- 端到端/视觉（**人工**，不在 runner 内）：`pnpm dev`(127.0.0.1:1420) 或 `pnpm tauri:dev` + chrome-devtools MCP
  截图核 亮/暗 × 连接/断开 × 中/英/fa-RTL × 小窗口断点；reduced-motion 开启时辉光/动效停；对照本报告评分卡逐项核。

## 执行范围与约束

- **只动前端「界面」层**：UI/样式/交互/i18n/前端 store 接线；**不改** Rust 后端、IPC 契约、构建/发布脚本。
- 遵循 ADR 平台边界与 typed IPC：Hero 仅复用既有 IPC 命令与 runtime store，零新增后端逻辑。
- 沿用 shadcn `data-slot`/CVA 与 `cn()` 既有模式；不引入与 Safe Passage token 冲突的硬编码 hex。
- 视觉**最终观感**为人工 sign-off（runner 只保证编译/lint/test/结构断言通过），等同审计的 ops carve-out。
</content>
