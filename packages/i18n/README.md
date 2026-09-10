# @voya/i18n

支持英文（`en`）、简体中文（`zh-Hans`）和繁体中文（`zh-Hant`）。不支持的已保存语言偏好会回退到可用的浏览器语言，默认使用英文。

`src/locales/*.json` 是 Voya 直接维护的唯一语言资源，允许在功能改动中直接编辑。所有语言必须与英文 key 集完全对齐，并使用语义化 camelCase 命名空间。

`pnpm check:i18n` 检查语言 key 对齐、空值、生产源码中的未定义或动态翻译 key，以及未通过翻译资源呈现的 JSX 文本。仓库不读取 v2rayN ResX，不维护 overlay，也不提供上游语言快照生成流程。
