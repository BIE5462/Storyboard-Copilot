# Qwen 图像 / Grok 图像 API 调用接口说明

## 1. 文档目的

本文档是一份**独立可用**的接口说明文档，只保留下面两类模型：

| 模型类型 | 说明 |
| --- | --- |
| Qwen 图像模型 | 标准图像生成接口调用说明 |
| Grok 图像模型 | 聊天补全多模态接口调用说明 |

本文档不依赖任何项目文件路径，也不要求结合源码阅读即可使用。

## 2. 统一约定

### 2.1 认证方式

所有请求统一使用 Bearer Token：

```http
Authorization: Bearer <API_KEY>
Content-Type: application/json
```

### 2.2 两类接口风格

| 模型 | 推荐接口风格 | 说明 |
| --- | --- | --- |
| Qwen 图像 | `POST /v1/images/generations` | 更适合标准图像生成接口 |
| Grok 图像 | `POST /v1/chat/completions` | 更适合多模态聊天补全接口 |

### 2.3 统一成功结果

无论上游返回格式如何，业务层建议最终统一整理成下面的结构：

```json
{
  "success": true,
  "provider": "qwen",
  "model": "qwen-image",
  "created": 1743926400,
  "images": [
    {
      "url": "https://example.com/result.webp",
      "b64_json": null,
      "mime_type": "image/webp"
    }
  ],
  "raw_response": {}
}
```

### 2.4 统一失败结果

建议统一整理成下面的结构：

```json
{
  "success": false,
  "error": {
    "code": "MODEL_API_ERROR",
    "message": "上游模型服务调用失败",
    "status": 500,
    "raw": {}
  }
}
```

### 2.5 使用参考图时的统一调用原则

结合参考实现，建议把“参考图”统一抽象成顶层数组字段：

```json
{
  "reference_images": [
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA...",
    "https://cdn.example.com/reference-2.png"
  ]
}
```

#### 2.5.1 推荐字段定义

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `reference_images` | `string[]` | 否 | 参考图数组，建议按顺序传入 |

#### 2.5.2 推荐输入格式

`reference_images` 中每一项建议使用以下三类格式之一：

| 格式 | 示例 | 适用场景 |
| --- | --- | --- |
| Data URL | `data:image/png;base64,...` | 最稳定，适合直接内联发送到上游 |
| HTTPS URL | `https://cdn.example.com/ref.png` | 适合上游可直接拉取公网图片时使用 |
| 文件 URL | `file:///Users/demo/ref.png` | 仅适用于本地网关会先做转码或上传的场景 |

#### 2.5.3 推荐约束

| 约束项 | 建议值 |
| --- | --- |
| 参考图数量 | 建议不超过 `5` 张 |
| 图片顺序 | 按业务语义顺序固定传入，不要在服务端重新打乱 |
| 单图大小 | 建议在发送前压缩或缩放，避免请求体过大 |
| 无效图片 | 如果存在参考图，但无法转为可用 URL / Base64，应直接返回参数错误 |

#### 2.5.4 上游映射原则

统一入参是 `reference_images`，但真正发给模型时应按接口风格映射：

| 上游接口类型 | 推荐映射方式 |
| --- | --- |
| 图像生成接口型 | 映射到上游约定的图片输入字段；对于当前千海 Qwen 网关建议直接映射为顶层 `image_urls` |
| 聊天补全多模态型 | 映射到 `messages[0].content[]` 中的多个图片项 |

一句话理解：

1. 业务层统一收 `reference_images`。
2. 适配层再把它改写成不同模型真正需要的字段。

## 3. Qwen 图像模型接口说明

### 3.1 接口定义

| 项目 | 说明 |
| --- | --- |
| 方法 | `POST` |
| 路径 | `/v1/images/generations` |
| 用途 | 文生图 |
| 认证 | `Authorization: Bearer <API_KEY>` |
| 请求类型 | `application/json` |

完整请求地址示例：

```text
https://<QWEN_BASE_URL>/v1/images/generations
```

### 3.2 请求体协议

Qwen 现统一使用 `input.messages[0].content[]` 的多模态内容数组协议。

#### 3.2.1 标准请求体

```json
{
  "model": "qwen-image-max",
  "input": {
    "messages": [
      {
        "role": "user",
        "content": [
          { "image": "https://cdn.example.com/ref-1.png" },
          { "image": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA..." },
          { "text": "保留参考图中的人物发型和建筑关系，生成电影感海报" }
        ]
      }
    ]
  },
  "parameters": {
    "n": 1,
    "negative_prompt": " ",
    "prompt_extend": true,
    "watermark": false,
    "size": "1536*1024"
  }
}
```

#### 3.2.2 字段说明

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `model` | `string` | 是 | 运行时实际调用的 Qwen 模型名 |
| `input.messages[0].role` | `string` | 是 | 固定为 `user` |
| `input.messages[0].content` | `array` | 是 | 参考图与文本提示组成的内容数组 |
| `content[].image` | `string` | 否 | 单张参考图，支持 `https://...` 与 `data:image/...` |
| `content[].text` | `string` | 是 | 文本提示词，放在内容数组最后 |
| `parameters.size` | `string` | 是 | 输出尺寸，必须是精确 `W*H` |
| `parameters.n` | `number` | 否 | 生成数量，默认 `1` |
| `parameters.negative_prompt` | `string` | 否 | 反向提示词，默认 `" "` |
| `parameters.prompt_extend` | `boolean` | 否 | 是否让服务端扩展提示词，默认 `true` |
| `parameters.watermark` | `boolean` | 否 | 是否加水印，默认 `false` |

### 3.3 尺寸与参考图约束

#### 3.3.1 尺寸白名单

Qwen 仅支持下列 14 个尺寸值，统一使用 `*` 作为分隔符：

| 比例 | 可用尺寸 |
| --- | --- |
| `1:1` | `1024*1024`、`1536*1536` |
| `2:3` | `768*1152`、`1024*1536` |
| `3:2` | `1152*768`、`1536*1024` |
| `3:4` | `960*1280`、`1080*1440` |
| `4:3` | `1280*960`、`1440*1080` |
| `9:16` | `720*1280`、`1080*1920` |
| `16:9` | `1280*720`、`1920*1080` |

补充约定：

1. 历史值 `1024x1024` 可兼容并归一成 `1024*1024`。
2. 其他旧尺寸值不再支持，建议在进入上游前先回退或直接报错。
3. Qwen 交互建议只编辑 `size`，不要再向上游发送独立 `aspect_ratio`。

#### 3.3.2 参考图规则

| 项目 | 说明 |
| --- | --- |
| 最大数量 | 最终请求最多 `5` 张参考图 |
| 支持来源 | `https://...` 与 `data:image/...` |
| 不支持来源 | 空字符串、文件路径、其他协议 |
| 顺序要求 | 保持原始顺序，不在适配层重排 |
| Storyboard 规则 | 自动附加的网格参考图同样占用这 `5` 张名额 |

#### 3.3.3 纯文生图 cURL 示例

```bash
curl -X POST "https://<QWEN_BASE_URL>/v1/images/generations" \
  -H "Authorization: Bearer <QWEN_API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen-image-max",
    "input": {
      "messages": [
        {
          "role": "user",
          "content": [
            { "text": "一只机械白虎机甲站在霓虹城市屋顶上，电影感光影，超高细节，8k" }
          ]
        }
      ]
    },
    "parameters": {
      "n": 1,
      "negative_prompt": " ",
      "prompt_extend": true,
      "watermark": false,
      "size": "1024*1024"
    }
  }'
```

#### 3.3.4 多参考图 cURL 示例

```bash
curl -X POST "https://<QWEN_BASE_URL>/v1/images/generations" \
  -H "Authorization: Bearer <QWEN_API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen-image-max",
    "input": {
      "messages": [
        {
          "role": "user",
          "content": [
            { "image": "https://cdn.example.com/reference-1.png" },
            { "image": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA..." },
            { "text": "使用图一的城市照片作为底图，请勿更改真实建筑与街道，在建筑周围加入扁平壁画风角色" }
          ]
        }
      ]
    },
    "parameters": {
      "n": 1,
      "negative_prompt": " ",
      "prompt_extend": true,
      "watermark": false,
      "size": "1920*1080"
    }
  }'
```

### 3.4 成功返回格式

#### 3.4.1 URL 返回示例

```json
{
  "created": 1743926400,
  "data": [
    {
      "url": "https://cdn.example.com/generated/qwen-image-001.webp"
    }
  ]
}
```

#### 3.4.2 Base64 返回示例

```json
{
  "created": 1743926400,
  "data": [
    {
      "b64_json": "iVBORw0KGgoAAAANSUhEUgAA..."
    }
  ]
}
```

#### 3.4.3 字段说明

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `created` | `number` | 结果生成时间戳 |
| `data` | `array` | 图片结果数组 |
| `data[].url` | `string` | 图片访问地址 |
| `data[].b64_json` | `string` | 图片 Base64 内容 |

业务层解析规则建议：

| 顺序 | 解析方式 |
| --- | --- |
| 1 | 如果存在 `data[0].url`，直接使用 URL |
| 2 | 如果不存在 URL，但存在 `data[0].b64_json`，则转为二进制图片保存 |
| 3 | 如果两者都没有，则判定为返回格式异常 |

### 3.5 失败返回格式

#### 3.5.1 失败示例

```json
{
  "error": {
    "message": "Invalid API key",
    "type": "authentication_error",
    "code": "invalid_api_key"
  }
}
```

#### 3.5.2 处理建议

| 场景 | 建议处理 |
| --- | --- |
| `401` / `403` | 视为鉴权失败，提示 API Key 无效或过期 |
| `429` | 视为限流，建议重试并做退避 |
| `5xx` | 视为上游服务故障，记录完整响应并做降级 |
| 返回体没有 `data` | 视为格式错误，进入解析失败分支 |

## 4. Grok 图像模型接口说明

### 4.1 接口定义

| 项目 | 说明 |
| --- | --- |
| 方法 | `POST` |
| 路径 | `/v1/chat/completions` |
| 用途 | 通过聊天补全方式触发图像生成 |
| 认证 | `Authorization: Bearer <API_KEY>` |
| 请求类型 | `application/json` |

完整请求地址示例：

```text
https://<GROK_BASE_URL>/v1/chat/completions
```

### 4.2 纯文本提示生成图片

#### 4.2.1 标准请求体

```json
{
  "model": "grok-image",
  "messages": [
    {
      "role": "user",
      "content": "生成一张赛博朋克风格的夜景街道海报，蓝紫色霓虹灯，雨夜反光地面，电影构图"
    }
  ],
  "stream": false,
  "size": "1024x1024",
  "response_format": {
    "type": "url"
  }
}
```

#### 4.2.2 字段说明

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `model` | `string` | 是 | Grok 图像模型名称 |
| `messages` | `array` | 是 | 标准聊天消息数组 |
| `messages[].role` | `string` | 是 | 一般固定传 `user` |
| `messages[].content` | `string` 或 `array` | 是 | 纯文本生成时可直接传字符串；参考图场景建议传数组 |
| `stream` | `boolean` | 否 | 图像生成场景建议固定为 `false` |
| `size` | `string` | 否 | 希望生成的图片尺寸 |
| `response_format` | `object` | 否 | 期望返回类型，推荐 `{ "type": "url" }` |

说明：

1. Grok 图像调用推荐关闭流式输出，统一使用一次性返回。
2. 如果上游不接受 `size` 或 `response_format`，可移除这两个字段。

### 4.3 使用参考图时如何调用

Grok 图像更适合按“多模态聊天补全”方式携带参考图。推荐做法是把每一张参考图都放进 `messages[0].content` 数组，然后把文本提示词放在最后一个文本项中。

#### 4.3.1 推荐请求体

```json
{
  "model": "grok-image",
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "image_url",
          "image_url": {
            "url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA..."
          }
        },
        {
          "type": "image_url",
          "image_url": {
            "url": "https://cdn.example.com/reference-2.png"
          }
        },
        {
          "type": "text",
          "text": "保留参考图中的人物脸型、发色和服装剪影，生成一张赛博朋克夜景海报，霓虹灯，雨夜反射地面，电影级构图"
        }
      ]
    }
  ],
  "stream": false,
  "size": "1024x1024",
  "response_format": {
    "type": "url"
  }
}
```

#### 4.3.2 参考图调用说明

| 项目 | 说明 |
| --- | --- |
| 参考图放置位置 | `messages[0].content[]` |
| 单张图片格式 | `{ "type": "image_url", "image_url": { "url": "..." } }` |
| 提示词放置位置 | 与图片同一个 `content[]` 数组中的最后一个 `text` 项 |
| 推荐数量 | `1` 到 `5` 张 |
| 推荐顺序 | 先图后文，图片按重要性排序 |
| 返回结果 | 与普通 Grok 图像相同，仍从 `choices[0].message.content` 中解析图片地址 |

#### 4.3.3 为什么推荐这种格式

这种格式的优点是：

1. 与多模态聊天接口的常见协议兼容。
2. 图片和文字在一个消息里，语义最清晰。
3. 后续扩展多张参考图、局部编辑、风格约束时更容易维护。

#### 4.3.4 纯文本 cURL 示例

```bash
curl -X POST "https://<GROK_BASE_URL>/v1/chat/completions" \
  -H "Authorization: Bearer <GROK_API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "grok-image",
    "messages": [
      {
        "role": "user",
        "content": "生成一张赛博朋克风格的夜景街道海报，蓝紫色霓虹灯，雨夜反光地面，电影构图"
      }
    ],
    "stream": false,
    "size": "1024x1024",
    "response_format": {
      "type": "url"
    }
  }'
```

#### 4.3.5 使用参考图的 cURL 示例

```bash
curl -X POST "https://<GROK_BASE_URL>/v1/chat/completions" \
  -H "Authorization: Bearer <GROK_API_KEY>" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "grok-image",
    "messages": [
      {
        "role": "user",
        "content": [
          {
            "type": "image_url",
            "image_url": {
              "url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAA..."
            }
          },
          {
            "type": "image_url",
            "image_url": {
              "url": "https://cdn.example.com/reference-2.png"
            }
          },
          {
            "type": "text",
            "text": "保留参考图中的人物脸型、发色和服装剪影，生成一张赛博朋克夜景海报，霓虹灯，雨夜反射地面，电影级构图"
          }
        ]
      }
    ],
    "stream": false,
    "size": "1024x1024",
    "response_format": {
      "type": "url"
    }
  }'
```

### 4.4 成功返回格式

Grok 图像场景下，返回内容通常需要从 `choices[0].message.content` 中提取图片地址。

#### 4.4.1 Markdown 图片格式示例

```json
{
  "id": "chatcmpl-001",
  "object": "chat.completion",
  "created": 1743926400,
  "model": "grok-image",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "![generated image](https://cdn.example.com/grok-image-001.webp)"
      },
      "finish_reason": "stop"
    }
  ]
}
```

#### 4.4.2 Markdown 链接格式示例

```json
{
  "id": "chatcmpl-002",
  "object": "chat.completion",
  "created": 1743926405,
  "model": "grok-image",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "[下载图片](https://cdn.example.com/grok-image-002.png)"
      },
      "finish_reason": "stop"
    }
  ]
}
```

#### 4.4.3 纯 URL 格式示例

```json
{
  "id": "chatcmpl-003",
  "object": "chat.completion",
  "created": 1743926410,
  "model": "grok-image",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "https://cdn.example.com/grok-image-003.jpg"
      },
      "finish_reason": "stop"
    }
  ]
}
```

### 4.5 返回结果解析规则

建议按下面顺序解析 `choices[0].message.content`：

| 优先级 | 解析规则 | 示例 |
| --- | --- | --- |
| 1 | 匹配 Markdown 图片 | `![alt](https://xxx/image.webp)` |
| 2 | 匹配 Markdown 链接 | `[下载](https://xxx/image.png)` |
| 3 | 匹配正文中的裸 URL | `https://xxx/image.jpg` |
| 4 | 如果正文本身就是 URL | 直接作为图片地址 |
| 5 | 如果以上都失败 | 视为无法解析图片结果 |

推荐解析正则：

```javascript
const markdownImageRegex = /!\[.*?\]\((https?:\/\/[^\s)]+)\)/;
const markdownLinkRegex = /\[.*?\]\((https?:\/\/[^\s)]+)\)/;
const rawUrlRegex = /(https?:\/\/[^\s)]+\.(?:webp|png|jpg|jpeg|gif))/i;
```

推荐解析代码：

```javascript
function extractGrokImageUrl(content) {
  if (!content || typeof content !== "string") {
    return null;
  }

  const markdownImageMatch = content.match(/!\[.*?\]\((https?:\/\/[^\s)]+)\)/);
  if (markdownImageMatch && markdownImageMatch[1]) {
    return markdownImageMatch[1];
  }

  const markdownLinkMatch = content.match(/\[.*?\]\((https?:\/\/[^\s)]+)\)/);
  if (markdownLinkMatch && markdownLinkMatch[1]) {
    return markdownLinkMatch[1];
  }

  const rawUrlMatch = content.match(/(https?:\/\/[^\s)]+\.(?:webp|png|jpg|jpeg|gif))/i);
  if (rawUrlMatch && rawUrlMatch[1]) {
    return rawUrlMatch[1];
  }

  if (content.trim().startsWith("http://") || content.trim().startsWith("https://")) {
    return content.trim();
  }

  return null;
}
```

### 4.6 失败返回格式

#### 4.6.1 失败示例

```json
{
  "error": {
    "message": "Model not found",
    "type": "invalid_request_error",
    "code": "model_not_found"
  }
}
```

#### 4.6.2 处理建议

| 场景 | 建议处理 |
| --- | --- |
| `choices` 为空 | 判定为响应格式错误 |
| `message.content` 为空 | 判定为响应内容为空 |
| `message.content` 无法提取 URL | 判定为结果解析失败 |
| `401` / `403` | 判定为鉴权失败 |
| `429` | 判定为限流，建议延迟重试 |
| `5xx` | 判定为上游服务异常 |

## 5. 最终接入建议

### 5.1 接口选型

| 模型 | 推荐上游接口 | 参考图传法 |
| --- | --- | --- |
| Qwen 图像 | `/v1/images/generations` | 当前千海网关兼容顶层 `prompt` + `image_urls` 旧协议 |
| Grok 图像 | `/v1/chat/completions` | `messages[0].content[]` 中混合图片项和文本项 |

### 5.2 最小联调 checklist

| 检查项 | Qwen 图像 | Grok 图像 |
| --- | --- | --- |
| Bearer Token 是否正确 | 是 | 是 |
| 模型名是否正确 | 是 | 是 |
| 请求路径是否正确 | `/v1/images/generations` | `/v1/chat/completions` |
| 参考图是否可访问或可编码 | 是 | 是 |
| 参考图数量是否控制在 5 张以内 | 是 | 是 |
| 返回体是否包含可解析图片结果 | `data[].url` 或 `b64_json` | `message.content` 中可提取 URL |
| 是否统一转换为业务输出格式 | 是 | 是 |

### 5.3 最终结论

| 模型 | 结论 |
| --- | --- |
| Qwen 图像 | 统一走 `/v1/images/generations`；当前千海网关兼容顶层 `prompt` + `image_urls`，并继续使用精确 `W*H` 尺寸 |
| Grok 图像 | 统一走 `/v1/chat/completions`；参考图场景把图片放进 `messages[0].content[]`，文本说明紧跟其后 |

如果只记两条规则：

1. `Qwen 图像`：当前千海网关上仍建议使用顶层 `prompt` + `image_urls`，并通过精确 `W*H` 指定尺寸。
2. `Grok 图像`：更像“多模态聊天型”，参考图建议放进 `messages[].content` 里的图片项。
