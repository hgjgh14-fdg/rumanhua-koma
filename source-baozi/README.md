# 包子漫画 Koma 书源

独立适配包子漫画公开页面与公开数据接口，支持多个可选站点入口。

## 实现范围
- 站内搜索，支持按页加载更多结果
- 全部漫画、人气推荐、热门更新、最新上架，以及国漫/日漫/韩漫/欧美分类
- 兼容当前详情页 `#bookmarkData` 与旧模板 `#mangachapters` 的漫画 ID 提取
- 漫画详情、公开章节目录
- 按章节 slug 打开真实阅读页，读取页面中的 `data-ms`/`data-cs`
- 阅读 API 返回图片数据解码、CDN 地址拼接与图片请求头
- 支持主站及备用入口：`baozimh.org`、`m.baozimh.org`、`bzmh.org`、`m.bzmh.org`、`m.baozimh.one`、`base.baozimh.org`；Koma 支持域名选择功能时可在书源设置中切换，旧客户端自动探测可访问入口

## 当前接口
- 默认页面：`https://baozimh.org`；其他入口使用相同页面适配逻辑，并由 CI 验证搜索、详情、章节和阅读页
- 章节目录：`https://v2.apikk.top/api/v2/manga/get?mid=...&mode=all`
- 阅读图片：`https://v2.apikk.top/api/v2/chapter/getinfo?m=...&c=...`
- 默认图片 CDN：`https://c-nd3-1.6wm.top`
- 线路 2 图片 CDN：`https://c-nd2-1.6wm.top`

CI 会验证宽泛关键词的搜索分页、多个漫画详情、章节目录，以及真实阅读图片下载和解码。仅请求公开可访问内容，不处理登录、验证码或访问控制。
