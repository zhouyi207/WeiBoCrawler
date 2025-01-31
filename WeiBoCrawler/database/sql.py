import logging
from sqlalchemy import select, inspect, create_engine
from sqlalchemy.orm import sessionmaker
from sqlalchemy.ext.asyncio import create_async_engine, AsyncSession, async_sessionmaker
from .sql_record import Base, BodyRecord, Comment1Record, Comment2Record, RecordFrom

logging.basicConfig(level=logging.INFO, encoding="utf-8")

logging.info("数据库初始化")

class DatabaseManager:
    """数据库的增删改查

    """
    def __init__(self, sync_db_url: str, async_db_url: str):
        """初始化数据库

        Args:
            sync_db_url (str): 同步的数据库连接字符串
            async_db_url (str): 异步的数据库连接字符串
        """
        # 引擎
        self.sync_engine = create_engine(sync_db_url)
        self.async_engine = create_async_engine(async_db_url)

        # 会话工厂
        self.sync_session = sessionmaker(self.sync_engine, expire_on_commit=False)
        self.async_session = async_sessionmaker(self.async_engine, class_=AsyncSession, expire_on_commit=False)

    def sync_create_tables(self):
        """同步创建表
        
        """
        Base.metadata.create_all(self.sync_engine)

    async def async_create_tables(self):
        """异步创建表
        
        """
        async with self.async_engine.begin() as conn:
            await conn.run_sync(Base.metadata.create_all)


    def sync_add_records(self, records: list[ BodyRecord | Comment1Record | Comment2Record ]) -> list[int]:
        """同步插入记录

        Args:
            records (list[ BodyRecord | Comment1Record | Comment2Record ]): 记录列表

        Returns:
            list[int]: id列表
        """
        with self.sync_session() as session:
            try:
                session.add_all(records)
                session.commit()
                return [record.id for record in records]
            except Exception as e:
                session.rollback()
                logging.error(f"插入记录时出现异常: {e}", exc_info=True)
                return []

    async def async_add_records(self, records: list[ BodyRecord | Comment1Record | Comment2Record ]) -> list[int]:
        """异步插入记录
        
        Args:
            records (list[ BodyRecord | Comment1Record | Comment2Record ]): 记录列表
        
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

    def sync_get_records_by_ids(self, model:  BodyRecord | Comment1Record | Comment2Record , ids: list[int]) -> list[ BodyRecord | Comment1Record | Comment2Record ]:
        """同步查询记录
        
        Args:
            model ( BodyRecord | Comment1Record | Comment2Record ): 搜索类
            ids (list[int]): 搜索id列表

        Returns:
            list[ BodyRecord | Comment1Record | Comment2Record ]: 搜索列表
        """
        with self.sync_session() as session:
            return session.query(model).filter(model.id.in_(ids)).all()

    async def async_get_records_by_ids(self, model:  BodyRecord | Comment1Record | Comment2Record , ids: list[int]) -> list[ BodyRecord | Comment1Record | Comment2Record ]:
        """异步查询记录
        
        Args:
            model ( BodyRecord | Comment1Record | Comment2Record ): 搜索类
            ids (list[int]): 搜索id列表

        Returns:
            list[ BodyRecord | Comment1Record | Comment2Record ]: 搜索列表
        """
        async with self.async_session() as session:
            stmt = select(model).where(model.id.in_(ids))
            result = await session.execute(stmt)
            return result.scalars().all()

    def sync_update_record(self, model:  BodyRecord | Comment1Record | Comment2Record , record_id: int, **kwargs) ->  BodyRecord | Comment1Record | Comment2Record :
        """同步更新记录
        
        Args:
            model ( BodyRecord | Comment1Record | Comment2Record ): 更新类
            record_id (int): 更新id
            kwargs: 更新的字段和值

        Returns:
             BodyRecord | Comment1Record | Comment2Record : 更新类
        """
        with self.sync_session() as session:
            record = session.get(model, record_id)
            if record:
                for key, value in kwargs.items():
                    setattr(record, key, value)
                try:
                    session.commit()
                except Exception as e:
                    session.rollback()
                    logging.error(f"更新记录时出现异常: {e}", exc_info=True)
            return record

    async def async_update_record(self, model:  BodyRecord | Comment1Record | Comment2Record , record_id: int, **kwargs) ->  BodyRecord | Comment1Record | Comment2Record :
        """异步更新记录
        
        Args:
            model ( BodyRecord | Comment1Record | Comment2Record ): 更新类
            record_id (int): 更新id
            kwargs: 更新的字段和值

        Returns:
             BodyRecord | Comment1Record | Comment2Record : 更新记录
        """
        async with self.async_session() as session:
            record = await session.get(model, record_id)
            if record:
                for key, value in kwargs.items():
                    setattr(record, key, value)
                try:
                    await session.commit()
                except Exception as e:
                    await session.rollback()
                    logging.error(f"更新记录时出现异常: {e}", exc_info=True)
            return record

    def sync_delete_record(self, model:  BodyRecord | Comment1Record | Comment2Record , record_id: int) ->  BodyRecord | Comment1Record | Comment2Record :
        """同步删除记录
        
        Args:
            model ( BodyRecord | Comment1Record | Comment2Record ): 删除类
            record_id (int): 删除id
        
        Returns:
             BodyRecord | Comment1Record | Comment2Record : 删除记录
        """
        with self.sync_session() as session:
            record = session.get(model, record_id)
            if record:
                try:
                    session.delete(record)
                    session.commit()
                except Exception as e:
                    session.rollback()
                    logging.error(f"删除记录时出现异常: {e}", exc_info=True)
            return record

    async def async_delete_record(self, model:  BodyRecord | Comment1Record | Comment2Record , record_id: int) ->  BodyRecord | Comment1Record | Comment2Record :
        """异步删除记录
        
        Args:
            model ( BodyRecord | Comment1Record | Comment2Record ): 删除类
            record_id (int): 删除id
        """
        async with self.async_session() as session:
            record = await session.get(model, record_id)
            if record:
                try:
                    await session.delete(record)
                    await session.commit()
                except Exception as e:
                    await session.rollback()
                    logging.error(f"删除记录时出现异常: {e}", exc_info=True)
            return record

    def sync_get_table_names(self) -> list[str]:
        """同步获取表名
        
        Returns:
            list[str]: 表名列表
        """
        inspector = inspect(self.sync_engine)
        return inspector.get_table_names()

    async def async_get_table_names(self) -> list[str]:
        """异步获取表名

        Returns:
            list[str]: 表名列表
        """
        inspector = inspect(self.sync_engine)
        return inspector.get_table_names()


__all__ = [BodyRecord, Comment1Record, Comment2Record, RecordFrom, DatabaseManager]