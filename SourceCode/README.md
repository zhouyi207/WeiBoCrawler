2025.01.28
1. params flow 这个字段表示按热度还是按时间，1. 表示热度，2. 表示时间。在这里目前只有 comment_request 使用到了变化的字段，其他并没有用到，设置的是固定的。
2. pack.get_commment1_data.py get_commment2_data.py 这两文件中进度条有问题，进度条 desp 和 进度条 total 需要修改一下。由于没有预先设置 totol，会导致默认为 100
3. pack 可以重构解耦一下
4. 除了 get_commment1_data.py get_commment2_data.py 这两文件，异步都没怎么用，应该先创建 task 然后使用 asyncio.gather(*tasks) 注册 task
5. 差距为已被

