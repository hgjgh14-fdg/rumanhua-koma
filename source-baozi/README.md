# 包子漫画 Koma 书源

独立适配 https://baozimh.org/ 的公开页面与公开数据接口。

当前实现范围：
- 站内搜索与首页分类列表
- 漫画详情与公开章节目录
- 章节图片列表与图片 Referer

主要站点接口：
- 页面：`https://baozimh.org`
- 目录 API：`https://api-get-v2.mgsearcher.com/api/manga/get?mid=...&mode=all`
- 阅读 API：`https://api-get-v2.mgsearcher.com/api/chapter/getinfo?m=...&c=...`
- 图片 CDN：`https://f40-1-4.g-mh.online`

仅请求站点公开可访问内容；不处理登录、验证码或访问控制。站点接口和 CDN 可能变化，发布前必须通过 CI 的搜索、详情、目录、阅读页链路测试。
