# 快速开始

登录后可以通过 `/app/help?article=getting-started` 在应用内阅读本指南；登录前也可以通过公开路由 `/help/getting-started` 查看。

## 主要入口

- `/app/dashboard`：总览、运行状态、到期提醒
- `/app/exchange`：绑定币安 API 和连接测试
- `/app/strategies`：策略列表、批量动作、全部停止
- `/app/strategies/new`：创建策略草稿
- `/app/orders`：查看订单、成交、交易所活动
- `/app/billing`：会员订单和付款说明
- `/app/telegram`：绑定 Telegram 机器人
- `/app/security`：修改密码和管理 TOTP

## 首次使用路径

1. 注册账号并直接登录。
2. 进入安全中心，先检查密码和 TOTP 状态。
3. 如果你是管理员账号，先打开 `/admin-bootstrap` 完成管理员 TOTP 初始化，再登录后台。
4. 打开 `/app/exchange` 保存币安 API Key 和 Secret。
5. 运行一次连接测试。
6. 打开 `/app/billing` 创建会员支付订单。
7. 按页面显示的精确金额、链路和币种完成转账。
8. 会员生效后，进入 `/app/strategies/new` 创建草稿。
9. 先跑预检，通过后再启动策略。

## 本地环境

- 完整栈启动：`docker compose --env-file .env -f deploy/docker/docker-compose.yml up -d --build`
- 停止并清理：`docker compose --env-file .env -f deploy/docker/docker-compose.yml down -v`
- 仅 Rust 服务：`cargo run -p api-server`

## 币安 API 检查清单

- 只开启读取和交易权限
- 必须关闭提现权限
- 合约策略要求对冲模式
- 一个用户只能绑定一个币安账户
