# Keiyoushi 中文漫画源 → Koma 适配清单

> 范围：Keiyoushi `extensions-source/src/zh` 当前可见的 72 个中文扩展目录。  
> 目标：将可用站点重新实现为 Koma Rust/WASM 书源，并进入本仓库的 `.koma` 发布流程。  
> 注意：目录名称相同不代表实现等效；本表是初步盘点，不代表已完成适配或已通过可用性测试。

## 适配规则

1. 优先复用 Koma 已有实现，先核对站点、搜索、列表、详情、章节、阅读图片和必要请求头。
2. Keiyoushi Kotlin/Android 扩展不能直接作为 Koma 书源安装；只迁移站点协议与解析逻辑，使用 Koma SDK 独立实现。
3. 每个新源至少通过编译、搜索、详情、章节、图片请求测试；支持分页/分类的源还需测试翻页和筛选。
4. 只有完整阅读链路通过的源才加入正式订阅索引；失效站点、登录墙、验证码或反爬导致无法验证的源单独标记。
5. 尊重上游许可证和署名要求；不把上游 APK、Kotlin 二进制或不兼容索引直接打包进 Koma。

## 初步清单

| Keiyoushi 源目录 | 初步状态 | 处理说明 |
|---|---|---|
| `acgn` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `bakamh` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `baozimanhua` | 待恢复/修复 | 包子漫画候选已从当前 main 移除；旧迁移分支保留实现，但列表页回归未通过，未进入当前订阅 |
| `baozimhorg` | 待恢复/修复 | 与包子漫画候选同站；当前 main 不含该源，旧分支列表页测试失败，暂不发布 |
| `bh3` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `bikamanhua` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `bilimanga` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `boylove` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `cartoon18` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `cmanhua` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `comicabc` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `creativecomic` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `dm5` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `dogemanga` | 候选已分析，暂缓移植 | Keiyoushi 源为 JSON 搜索接口 + HTML 详情/章节/阅读页，含 Cloudflare 429 限速；`dogemanga.com` 本轮外部访问返回 403，尚不能验证完整链路，暂不进入正式源 |
| `dongmanmanhua` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `dumanwu` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `eighteenmanhua` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `favcomic` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `guazimanhua` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `hanabimanga` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `hanime1` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `hanman18` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `hcomic` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `hentaiclub` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `hikarinagi` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `ikanmh` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `ikmmh` | 疑似已有等效源 | ikmmh-zh；需逐项核对功能与站点实现 |
| `iqiyi` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `jcomic` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `jinmantiantang` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `jisumanhua` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `jiuermanhua` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `jiulingmanhua` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `kaobeimanhua` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `komiic` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `kuaikanmanhua` | 疑似已有等效源 | kuaikan；需逐项核对功能与站点实现 |
| `longjinman` | 疑似已有等效源 | longjm；需逐项核对功能与站点实现 |
| `manben` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `mangabz` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `mangaxiaosi` | 疑似已有等效源 | mangaxiaosi-zh；需逐项核对功能与站点实现 |
| `manhuadui` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `manhuagui` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `manhuaren` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `manhuashe` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `manhuawu` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `manwa` | 待重新评估 | 漫蛙候选已从当前 main 移除；需重新核对站点入口、反爬和完整阅读链路后再决定是否恢复 |
| `manwamanhua` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `mh1234` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `mh160` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `miaoqu` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `mikagogo` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `mycomic` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `nnhanman` | 疑似已有等效源 | nnhanman-zh；需逐项核对功能与站点实现 |
| `noyacg` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `picacomic` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `relamanhua` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `roumanwu` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `rumanhua` | 当前 Fork 已有候选 | source (如漫画)；需核对是否已纳入正式构建/索引 |
| `rumanhuaunoriginal` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `sixmh` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `tencentcomics` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `terrahistoricus` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `tongli` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `toptoon` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `utifa` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `vomic` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `wnacg` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `xipimanhua` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `yiqiqimanhua` | 待分析/移植 | 检查站点状态、功能及技术可行性后适配 Koma SDK |
| `zaimanhua` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `zazhimi` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |
| `zerobyw` | Koma 官方已有同名候选 | 先核对站点、功能和当前可用性，不重复移植 |

## 当前已确认的工程边界

- Koma 书源采用 Rust/WASM crate，经 Koma SDK 构建为 `.koma`。
- 订阅索引使用 `index.json`；Keiyoushi 的 `index.pb` 不是 Koma 索引。
- 当前 main 仅保留如漫画（`source/`）；包子漫画和漫蛙已在 2026-11-14 的主分支提交中移除。旧迁移分支中仍留有包子漫画候选代码，但其分类列表回归未通过，不能视为当前正式源。
- 下一步按“已有源核验 → 高可行性缺失源移植 → 逐源回归测试 → 更新订阅索引”的顺序推进。

## 状态定义

- **Koma 官方已有同名候选**：仅表示官方工作区存在同名 crate，不代表当前站点一定可用。
- **疑似已有等效源**：名称不同但可能对应同一站点，需比对代码和实际域名。
- **当前 Fork 已有候选**：本仓库已有对应代码，但需确认 workflow 是否编译、打包并发布。
- **待分析/移植**：尚未找到可靠的 Koma 对应实现。


## 本轮复核记录（2026-11-14）

### 包子漫画：已从 main 移除，列表/首页分页仍未验收

- GitHub Actions 最近一次主分支运行 [#130](https://github.com/hgjgh14-fdg/rumanhua-koma/actions/runs/43588544470) 已完成编译和打包，但在运行时测试阶段失败，未执行索引校验和 Release 发布。
- 失败点是包子漫画 `get_manga_list`：`https://baozimh.org/manga?page=1` 请求返回 HTTP 200，但书源解析结果未达到测试要求的至少 15 条；测试因此中止。
- 同一轮中包子漫画搜索、详情、章节、阅读页、首张图片下载与解码均通过。问题目前集中在分类列表页的 DOM 解析/分页，不应把它误判为阅读接口故障。
- 目前不能声称包子漫画列表页已修复，也不能将本次构建产物发布为正式订阅版本。下一步应先取得列表页原始 HTML 与解析诊断，再调整选择器并验证第二页无重复。

### 漫画狗（Doge Manga）：源逻辑已完成初步审阅

- Keiyoushi 实现使用 `GET /_search` 获取 JSON，其中 `manga_cards` 是 HTML 片段；热门与最新通过 `s=1` 区分，搜索通过 `q` 和 `o` 分页。
- 详情位于 `/m/{id}?l=en`，章节从详情 HTML 的 `#site-manga__tab-pane-all .site-manga-thumbnail__link` 提取，阅读页位于 `/p/{chapterId}`，图片 URL 来自 `img.site-reader__image[data-page-image-url]`。
- 上游明确对请求实施 5 个许可的限速，以应对 Cloudflare 429；Koma 适配必须保留节流，不能简单高并发请求。
- 本轮对 `https://dogemanga.com` 的外部访问返回 403，尚无法验证 JSON、详情、章节和图片端点。暂不新增一个未经验证的可用源，待站点可访问后继续实现和测试。


### 新候选：ACGN 动漫戏说（优先移植）

- Keiyoushi 源目录：`acgn`；站点：`https://comic.acgn.cc`。
- 本轮公开页面检查可访问首页、分类页和漫画详情页；站点首页展示最新漫画，分类页提供分页列表，详情页提供漫画信息和章节目录。
- Keiyoushi 实现的核心路径：详情 `/manhua-{id}.htm`，章节阅读 `/view-{id}.htm`；章节图片从 `#pic_list .pic[_src]` 的 `_src` 属性读取。
- 站内搜索使用 Google Programmable Search Element：先从 `cse.js` 获取 token / libVersion，再请求 `cse.google.com/cse/element/v1`。移植时需实现分页游标和速率限制，不能将 Google 搜索接口当作站内自有 API。
- 上游限制同主机请求速率为每秒 2 次；Koma 版本必须保留低频请求，避免触发站点限制。
- 当前只完成公开页面和上游源码结构审阅，尚未编写 Koma crate，也未验证阅读图片实际下载；标记为“优先候选”，不是可用书源。

### 其他候选复核

- `manben`（漫本，`https://www.manben.com`）当前网站可访问，Keiyoushi 源使用 POST `pagerdata.ashx` 获取 JSON 列表，详情页解析 HTML，阅读页需解包 `newImgs` 脚本并为图片请求设置章节 Referer。解包逻辑是额外移植成本，优先级暂列 ACGN 之后。
- `jisumanhua`（极速漫画，`https://www.1kkk.com`）当前网站可访问，Keiyoushi 扩展仅继承 DM5 多站点实现；但其站点路径形态（如 `/manhua432/`）与 Koma 官方 DM5 源目前的 `/manhua-...` 解析不一致，不能简单复制 DM5 crate 后只替换域名。
