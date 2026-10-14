# 颜喵插件 SDK 1.0.0

原生宿主注入 `window.yanmew`，接口类型见 `index.d.ts`。作者不导入 Tauri，不自行声明插件身份，不调用 `__TAURI_INTERNALS__`；宿主会拒绝其他命令。

## 存档与运行

`game.ready()` 只通知页面准备完成。`game.getContext()` 返回当前运行 ID、版本、权限摘要及已合并的设置默认值；`walletPermissionGranted` 是本次打开时的钱包授权快照，`permissions` 是查询时的当前授权。两者同时允许才可以提交新的钱包请求，中途授权需要下次打开才能结算。每次打开创建一条运行记录，重复入口聚焦已有窗口。正常关闭结束本次运行并保留存档；启动时将崩溃遗留的 running 记录标为 error。

运行记录不是游戏的一局，更不是长期进度。连续养成、剧情、角色等使用 `storage.*` 保存；按局游戏也自行保存局次，日次数限制按插件自己的进度规则实现。`game.complete()` 可选，表示本次运行已正常完成，之后不能提交新的钱包变化。`game.close()` 或窗口关闭不清除私有存档。

存档键最多 128 字节，由小写字母、数字、下划线、短横线组成，以点分段。单值最大 1 MiB，合计最大 10 MiB，最多 1024 个键。设置只接受 manifest 的 `settings_schema`（最多 64 项），支持 boolean/integer/number/string/enum，默认值和覆盖值同样校验。

## 钱包和网络

所有金额及数据库 ID 返回十进制字符串。请求 `delta` 为 JavaScript 安全范围内整数；请求 ID 最多 128 字节、展示原因最多 200 字。每个独立操作使用唯一 `request_id`；失败重试复用相同 ID 和业务参数；不得为每次重试产生新 ID。已经提交的请求在关闭或升级后仍可携带原运行 ID、数量与原因核对原账单，不再次结算；未提交的新请求只能使用当前运行 ID。先结算钱包再 complete。

打开时未授权钱包的运行不能提交喵币变化；中途授权从下次打开生效，撤权立即阻止新请求。插件自己的进度和战绩不能代替宿主账单。拒绝钱包权限时不得提交喵币变动。支持纯玩的插件可以继续游玩；玩法明确依赖钱包的插件应解释所需权限并停止开始，例如自带我的家。

`network.fetch({url, method, headers, body})` 返回标准 Response。只允许已申请且获允许的 HTTPS origin、公网 IP、最多 3 次重定向。超时 30 秒、同时最多 4 个请求、请求正文和解压后响应各 1 MiB；请求最多 32 个头、合计 16 KiB。禁止 Cookie、Host、代理认证、转发和逐跳头。可提交插件自己向用户取得的服务凭据；宿主不附加任何颜喵密钥。跨来源重定向移除自定义头，带正文的跨来源跳转除 303 外拒绝。

## 升级与数据迁移

插件版本为 SemVer，协议要求 `api_version: "1.0.0"`。存储版本是递增正整数，改变存档或不兼容设置结构时提高 `storage_version`，并通过 `migration` 指定包内 JS ES module。模块必须导出：

```js
export async function migrate(snapshot, targetVersion) {
  // snapshot = { storage_version, settings: {键:值}, data: {键:值} }
  const result = structuredClone(snapshot);
  // 在这里实现所有需要兼容的旧版本到目标版本转换。
  result.storage_version = targetVersion;
  return result;
}
```

迁移只获得自己的完整数据副本，无钱包、网络或存储写入接口。返回完整目标快照，省略的键会在成功后删除。迁移时间上限 15 秒；缺少迁移、异常、超时、无效结果或并发修改均保留原插件与存档。升级前关闭正在运行的插件，成功后再打开新版。不得使用迁移操作结算历史喵币。

## 浏览器开发

`mock-host.js` 的 `createMockHost({id,version,settings})` 返回模拟 SDK，可赋给开发页的 `window.yanmew`。存档按插件 ID 保存到浏览器 localStorage；模拟器不开放真实钱包和网络，也不承诺与宿主容量校验完全一致。这些权限、CSP、迁移隔离和原生关闭必须在真实颜喵宿主验收。
