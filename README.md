# 如漫画 Koma 漫源

将 https://www.rumanhua2.com/ 适配为 Koma 阅读器可安装的独立书源。

> 当前仓库处于开发阶段，尚未发布可安装的 `.koma` 包或订阅索引。不要将开发分支地址当作订阅地址。

## 目标

- 搜索漫画
- 查看漫画详情
- 获取完整章节目录
- 解析章节阅读页并解密图片地址
- 通过 GitHub Releases 发布 `.koma` 与 `index.json`

## 技术依据

网站适配参考公开网页结构与 [Breeze-plugin-RuManHua](https://github.com/deretame/Breeze-plugin-RuManHua) 的公开行为说明。Koma 包格式、SDK 和订阅索引遵循 [koma-sources](https://github.com/honjow/koma-sources) 的开发规范。

站点页面中，搜索、详情、章节和阅读页是不同的请求链路。阅读图片使用页面脚本中的 packed JavaScript 数据，解密过程需要单独实现并验证，不能只把图片标签中的 URL 当作最终图片地址。

## 开发状态

| 功能 | 状态 |
|---|---|
| Koma 包结构与 SDK 接入 | 待实现 |
| 搜索 | 待实现 |
| 漫画详情 | 待实现 |
| 完整章节目录 | 待实现 |
| 阅读图片解密 | 待实现 |
| 真机验证 | 待进行 |
| Releases / 订阅索引 | 待发布 |

只有搜索、详情、章节和实际阅读图片均通过验证后，才会发布正式订阅。
