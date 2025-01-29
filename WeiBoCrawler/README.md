- [x] 已完成
- [ ] 未完成

2025.01.28
- [ ] params flow 这个字段表示按热度还是按时间，1. 表示热度，2. 表示时间。在这里目前只有 comment_request 使用到了变化的字段，其他并没有用到，设置的是固定的。
- [x] pack.get_commment1_data.py get_commment2_data.py 这两文件中进度条有问题，进度条 desp 和 进度条 total 需要修改一下。由于没有预先设置 totol，会导致默认为 100
- [ ] 进度条不够美观，特别是 comment 请求
- [x] pack 可以重构解耦一下，使用抽象基类
- [x] 除了 get_commment1_data.py get_commment2_data.py 这两文件，异步都没怎么用，应该先创建 task 然后使用 asyncio.gather(*tasks) 注册 task
- [x] 差距为已被 修改为 差距为 一倍

2025.01.29

- [ ] 解析数据库. 在 parse 目录下 制作 process_xxx_json(TinyDB.table) -> pd.DataFrame 函数, 在这里实现一下数据库去重的逻辑(TinyDB)好像并没有去重的逻辑
- [x] 构建请求中的 headers 可以装在 client 中.（不可以，有的请求需要处理 headers）
- [x] 给 list 的 request 结果 添加 微博id 参数，与 body 保持一致.
- [ ] 前端初步搭建
- [ ] 模块的路径导入最好改用相对文件本身路径而不是使用项目路径