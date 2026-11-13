# 书源实现说明

站点主页： https://www.rumanhua2.com/

## 已核对的公开行为

- 搜索入口：`POST /s`，表单字段 `k`。
- 漫画详情：`/{comicId}/`。
- 完整章节目录：详情页中的章节链接需要与 `POST /morechapter`（表单字段 `id`）返回的历史章节合并。
- 阅读页：`/{comicId}/{chapterId}.html`。
- 阅读图片：阅读页脚本包含 packed JavaScript；解包后可得到 `__c0rst96` 密文。页面 `.readerContainer` 的 `data-id` 选择 XOR key，再经过 Base64 解码、循环 XOR、再次 Base64 解码得到图片 URL 数组。

## 重要适配事项

1. 搜索结果、详情封面和章节图片都要保留正确的 Referer。
2. 章节合并时去重，并根据站点返回顺序生成稳定阅读顺序。
3. 阅读页的图片地址不能直接从未解密脚本中猜测。
4. 图片解密逻辑需要对当前站点页面做实测；站点脚本变化时应明确报错，不应返回空章节或伪造图片。
5. 先验证公开可访问内容；不尝试绕过登录、付费、验证码或访问控制。

## 参考

- [如漫画官网](https://www.rumanhua2.com/)
- [公开 Breeze 插件实现](https://github.com/deretame/Breeze-plugin-RuManHua)
- [Koma 书源开发规范](https://github.com/honjow/koma-sources/blob/master/docs/WRITING_A_SOURCE.md)
