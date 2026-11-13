# 包子漫画 Koma 书源

独立适配 https://baozimh.org/ 的公开页面与公开数据接口。

## 实现范围
- 站内搜索与首页分类列表
- 漫画详情与公开章节目录
- 按章节公开 slug 打开真实阅读页，读取页面中的 data-ms/data-cs
- 阅读 API 返回图片数据解码、CDN 地址拼接与图片请求头

## 当前接口
- 页面：`https://baozimh.org`
- 章节目录：`https://v2.apikk.top/api/v2/manga/get?mid=...&mode=all`
- 阅读图片：`https://v2.apikk.top/api/v2/chapter/getinfo?m=...&c=...`
- 默认图片 CDN：`https://c-nd3-1.6wm.top`
- 线路 2 图片 CDN：`https://c-nd2-1.6wm.top`

CI 不再仅检查图片 URL 是否存在，还会使用阅读器请求头实际下载首张图片，并验证图片文件可解码。仅请求公开可访问内容，不处理登录、验证码或访问控制。
