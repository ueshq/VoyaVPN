# 第二轮清理：冗余间接层、重复实现与冗余代码

**背景**：工作区已有一轮同主题重构（79 文件未提交，静态审计确认无编译级遗留、四个工作包基本落地）。本轮在其之上：先验证基线，再补完上轮 5 个漏网子项 + 落地新一轮排查出的高价值清理点。全程行为不变（仅一处标注的小 UX 变化），可维护性优先。

## WP0 — 基线确认

上一轮改动从未跑过完整验证，先确认是绿的再叠加：`pnpm run check:bindings`、`pnpm run check:frontend:typecheck`、`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`。有红先修。

## WP1 — Rust 机械清理（voya-app / shell）

1. **删 `AppServices::load_config`**（`crates/voya-app/src/services.rs:54`，无生产调用点，4 个测试调用 :258/:276/:404/:421）：测试模块内加等价本地 helper（`settings().load()` + `app_state().load()` + `app_config_from_settings`）。不用 `load_config_for` 替换——它会额外应用 `enforce_platform_connection_mode`，行为不等价。
2. **内联 `runtime_config_contexts`**（`runtime.rs:376-384`，纯转发且 `_target_os` 参数未用）：生产 1 处 + 测试 4 处直接调 `CoreConfigContextBuilder::new(env).build_all(..)`。
3. **删 test-only pub 方法**（上轮同类漏网）：`ShutdownLatch::started`（`lifecycle.rs:81-84`，测试改走 `begin()`）、`AutostartManager::with_adapter`（`autostart.rs:39-45`，测试改 `with_service(AutostartService::new(adapter), ..)`）。
4. **updates.rs 小件**：`"srss"` 字面量 ×2 → `const SRS_DIR_NAME` + `fn srs_dir(paths)`；内联单调用转发 `collect_srs_assets`（:79-81）。
5. **WS 常量去重**：`proxy_runtime.rs:30-32` 与 `statistics.rs:35-38` 的重连三元组（1s/30s/5s）值全同 → 上移 `backoff.rs` 共享常量，同步修订 backoff.rs 头部"bounds stay with their managers"注释。`SINGBOX_INITIAL_DELAY`（首次连接延迟，语义不同）不动。

## WP2 — voya-core 测试 fixture 收敛（零生产风险）

三个测试模块手写了同一套构造 helper（各 ~70 行，签名微漂移）：`golden.rs:866-940`、`singbox/tests.rs:1635-1730`、`fmt/tests.rs:966+` 的 `endpoint`/`raw_transport`/`tls_settings`/context 构造器/`base_remote_node`/`socks_node`。新增 `#[cfg(test)] pub(crate) mod testutil`（放 `lib.rs:33` golden 同区，满足架构门 terminal 规则）收敛，`tls_settings` 以 golden 全参签名（name+alpn+ech）为准，约 -150 行。golden 套件守护。

## WP3 — 前端机械去重

1. **焦点恢复工具**：`packages/ui/src/lib/focus.ts` 导出 `focusIfConnected(el)` + `restoreFocus(trigger, fallback)`，替换 6 处内联副本（`connections-panel.tsx:478`、`logs-panel.tsx:354`、`use-node-editor.ts:37/94`、`server-table-dialogs.tsx:65/92`）。`node-group-card.tsx:55`（无连接检查、回退 body）语义不同，保持。
2. **SearchInput 组件**（packages/ui）：统一 3 份内联搜索框（`server-table.tsx:28-48`、`connections-panel.tsx:232-244`、`logs-panel.tsx:186-199`），以 server-table 的显式清空按钮 + Escape 清空为正典。⚠️ 唯一行为变化：connections/logs 从原生 `type="search"` 清空变为显式 X 按钮（可用性更好），测试跟随更新。
3. **use-check-update-dialog.ts**：本地 `withWorking(kind, run)` 收编 4× `setWorking`/try/catch/finally 样板；初始 status 拉取 useQuery 化（删 `statusGenerationRef` 手写世代守卫，新增 queryKeys 条目）。
4. **use-traffic-mode.ts:36**：手写 `useRuntimeActionStore(runtimeActionPending)` 订阅改用已存在的 `useRuntimeBusy()`（`stores/runtime-action.ts:178`）；:74 事件内即时守卫保留。
5. **tun-provider-text.ts 归位**：`features/home/` → `components/app-shell/`（与 `core-state-labels` 同层的纯文案模块；settings 反向依赖 home 是错层），2 个导入点更新。
6. **settings-save-queue 归位**：`features/settings/` → `src/lib/save-queue.ts`（通用 QueryClient 写队列，updates 只用 `settled()`），2 个非测试导入点 + 测试更新。
7. **OUTBOUND_LABEL_KEYS 合一**：`connections-panel.tsx:54-65` 的 `routeLabel()` 与 `connection-details.tsx:179-183` 的 `OUTBOUND_KEYS` 改用 `features/routing/rule-outbound.ts:9-13` 正典（proxy→routing 已有 import 先例）。
8. **proxy-groups-panel.tsx:42-50/88-97**：两个相同的 toastError 适配器 → 本地 `runWithToast` 闭包。
9. **settings SelectField**：`advanced-tab.tsx:118/139`、`core-tab.tsx:127` 三处同形 optionLabel 查找 → `SelectField` 直接接受标签映射。
10. **use-node-subscriptions.ts**：`updateSubscription`/`updateAllSubscriptions` 合并为 `updateMany(ids: string[] | null)`（后端本就接受 null=全部）；三组 ref+state 防重入统一为一个 `Set<string | "all">` 机制，约 -35 行（有测试）。

## WP4 — 前端组件级去重

1. **策略组成员渲染合一**：`policy-groups-section.tsx:231-265` 与 `proxy-groups-panel.tsx:130-169` 的 selector 按钮/静态 chip 两分支逐字重复（panel 内注释自认"与 Nodes 页相同"）→ 抽 `PolicyGroupMemberChip`（差异参数化：截断类、容器标签、data-testid）。延迟 Map 推导数据源不同（live 回退测速 vs 纯 live），保留各自逻辑。
2. **MoreMenu 共享外壳**：5 份无文字"⋯"溢出菜单（`server-table-menus.tsx:173-191`、`routing-rule-menu.tsx:72-84`、`node-group-card.tsx:142-204`、`connections-panel.tsx:265-283`、`logs-panel.tsx:220-254`）→ packages/ui 新组件（props：trigger 尺寸、className、disabled、aria-label；保持 `modal={false}` 语义）。带文字的 3 份变体差异大，不强行收编。
3. **RowActionMenus 对偶组件**：`server-table-menus.tsx:115-142/162-194` 与 `routing-rule-menu.tsx:44-61/63-85` 四个 ContextMenu+Menubar 成对函数同构，与 2 配合成一个共享对偶组件。
4. **3 个漏网手写滚动对话框**迁移 `ScrollableDialogContent`（已支持 `width` 字符串 prop，无需新变体）：`speedtest-settings-dialog.tsx:31`（40rem）、`connection-details.tsx:104`（35rem）、`logs-panel.tsx:349`（35rem），类名不变。

## 明确不做（已评估）

`tun/macos.rs` 769/800 行预防性拆分（非去重，另行处理）；AppState 访问器（风格选择）；profiles/routing move 索引合并（语义不同，上轮已备案）；其余跨 feature import（上轮"维持现状"决策不变）；`emit_core_state`/`emit_statistics_zero` 单调用包装（具名事件构造器有语义价值）。

## 验证

- 每个 WP 后：`cargo fmt` + clippy + 改动 crate 的单测 / 受影响 `*.test.tsx`（vitest 单文件）
- WP4 UI 组件改动后跑 `pnpm run check:frontend:smoke:mock`
- 最终 `pnpm run verify:local` 全量门（本地需 cargo-machete 0.9.2）
- 本轮无 IPC 契约变更（bindings 不动）、无 locale 变更
- 不主动 commit，完成后按区域汇总

预计净删 350-450 行；除 WP3.2 标注的清空按钮外全部行为不变。