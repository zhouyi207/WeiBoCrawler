from .sql import DatabaseManager

db_manager = DatabaseManager(
        sync_db_url='sqlite:///test.db',  # 同步模式
        async_db_url='sqlite+aiosqlite:///test.db'  # 异步模式
)

__all__ = ["db_manager"]