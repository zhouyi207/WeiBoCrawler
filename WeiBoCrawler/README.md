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

- [x] 解析数据库. 在 parse 目录下 制作 process_xxx_json(TinyDB.table) -> pd.DataFrame 函数, 在这里实现一下数据库去重的逻辑(TinyDB)好像并没有去重的逻辑
- [x] 构建请求中的 headers 可以装在 client 中.（不可以，有的请求需要处理 headers）
- [x] 给 list 的 request 结果 添加 微博id 参数，与 body 保持一致.
- [x] 前端初步搭建：数据展示.
- [x] 模块的路径导入最好改用相对文件本身路径而不是使用项目路径
- [x] drop_table_duplicates 函数 暂时使用最简单的列表去重法, 后续可以考虑使用 hash 去重等方法优化..

2025.1.30

- [x] 如果要实现更好的数据库效果，可以根据 mid 合并而不是 list body comment 分别展示，必须要实现字段统一.
- [ ] 由于是将所有请求的结果都保存在数据库，而展示的结果都是经过字段处理后的结果，需要给一个功能寻找指定数据的源数据.
- [ ] 给 uitl 添加 __all__ = []
- [x] 在下载前后检测数据表的状态，将变化的状态保存下来，方便知道新下载到哪里.
- [ ] get_body_data 中 数据表名为 id 改为需要 给定数据表名.
- [ ] 在 BaseDownloader.py 文件中添加日志功能，观察输出.
- [ ] 抽象类出现带参数的装饰器报错

向下面这样是不行的...


```python
def retry_timeout_decorator_asyncio(retry_times: int = 3) -> Callable:
    def _retry_timeout_decorator_asyncio(func: Callable) -> Callable:
        """超时重试装饰器(异步)

        Args:
            retry_times (int): 重试次数. Defaults to 3.

        Returns:
            Callable: 装饰后的函数
        """
        async def wrapper(*args, **kwargs):  # 将 wrapper 改为异步函数
            attempts = 0
            while attempts < retry_times:
                try:
                    return await func(*args, **kwargs)  # 调用异步函数并使用 await
                except httpx.TimeoutException as e:
                    attempts += 1
                    if attempts < retry_times:
                        logging.warning(f"请求超时，正在进行第 {attempts} 次重试...")
                    else:
                        logging.error(f"请求超时，重试次数已达到最大值，请检查网络连接或重试次数！错误原因{e}")
        return wrapper
    return _retry_timeout_decorator_asyncio
```


2025.01.31

- [ ] tinydb 这个玩意啊，5700条数据的时候插入一下要 1s，这是什么逆天的速度，我靠了.....想办法用其他数据库把，这玩意太影响速度了.....
- [ ] database 解耦，方便使用定义的数据库.
- [ ] 在使用 sqlalchemy 库操作数据库的时候，sessionmaker 中设置 expire_on_commit=False 可以避免在提交事务时自动刷新对象的状态，从而提高性能，但可能会出现脏读的现象，但是就我们的操作而言，单线程异步是不会出现脏读的情况的.


在设置 sessionmaker 中 expire_on_commit=True 的时候，在提交事务时自动刷新对象的状态，以异步为例子

```python
    async def async_add_records(self, records: list[ListRecord | BodyRecord | Comment1Record | Comment2Record ]) -> list[int]:
        """异步插入记录
        
        Args:
            records (list[ListRecord | BodyRecord | Comment1Record | Comment2Record ]): 记录列表
        
        Returns:
            list[int]: id列表
        """
        async with self.async_session() as session:
            try:
                session.add_all(records)
                await session.commit()
                return [record.id for record in records]
            except Exception as e:
                await session.rollback()
                logging.error(f"插入记录时出现异常: {e}", exc_info=True)
                return []
```

如果 expire_on_commit=True, 那么在提交事务时，会自动刷新对象的状态，即重新查询数据库中的数据，以确保数据的一致性。但是这里的查询是同步的，而 session 是异步会话，会出现在异步会话中调用同步功能的操作，这是一个 bug. 正确的处理方式是，使用异步去刷新 records 的状态.


```python
    async def async_add_records(self, records: list[ListRecord | BodyRecord | Comment1Record | Comment2Record ]) -> list[int]:
        """异步插入记录
        
        Args:
            records (list[ListRecord | BodyRecord | Comment1Record | Comment2Record ]): 记录列表
        
        Returns:
            list[int]: id列表
        """
        async with self.async_session() as session:
            try:
                session.add_all(records)
                await session.commit()
                # 修改的地方
                ids = []
                for record in records:
                    await session.refresh(record)
                    ids.append(record.id)
                return ids
            except Exception as e:
                await session.rollback()
                logging.error(f"插入记录时出现异常: {e}", exc_info=True)
                return []
```

这样就可以了.


- [x] tinydb 5700 条数据后要1s一条, sqlite 150000 条数据后 0.02s 一条. 我宣布我不认识 tinydb.... 
- [ ] tmd... sqlalchemy 在设置 relationship 的时候, 如果从表有多个外键，主表调用 relationship 函数中 foreign_keys 没用啊，老是报错, 只能使用 primaryjoin 函数来操作... 好煞笔.
- [ ] 我宣布 sqlalchemy 是个很傻鸟的库，妈的，定义那么多类型完全看不过来是干鸡毛，看你开源的份上作者我就不骂你了....  **peewee** 持续关注!