# @voya/ui

source-only 的共享 shadcn 原语、设计令牌、样式和字体；可用导出以 `package.json` 的 `exports` 为准。

本包依赖 DOM/Radix，`@voya/mobile` 不得消费。

`@voya/ui/components/form-fields` 提供受控的 `TextField`、`TextAreaField`、`SelectField` 和 `CheckboxField`，统一标签关联、错误提示和禁用状态。调用方提供翻译后的标签及选择项，选择框的空值仍以 `""` 传入和返回；本包不依赖桌面 IPC 或表单业务类型。
