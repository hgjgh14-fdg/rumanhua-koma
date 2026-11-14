# ACGN 动漫戏说 Koma 书源

移植自 Keiyoushi 中文源 `acgn`，站点为 https://comic.acgn.cc 。

当前实现目标：分类/字母列表、Google Programmable Search、漫画详情、章节目录、阅读页图片及图片 Referer。站点上游实现对同主机请求限制为每秒 2 次；Koma 版本需避免并发请求。

此源仍处于候选验证阶段。只有 CI 的搜索、列表分页、详情、章节、图片下载/解码测试全部通过后，才允许进入正式订阅索引。
