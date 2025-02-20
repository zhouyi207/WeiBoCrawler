from .sql import DatabaseManager , BodyRecord, Comment1Record, Comment2Record, RecordFrom
from ..util import database_config


db_path = database_config.path

db = DatabaseManager(
        sync_db_url=f'sqlite:///{db_path}',  # 同步模式
        async_db_url=f'sqlite+aiosqlite:///{db_path}'  # 异步模式
)

__all__ = ["db", "BodyRecord", "Comment1Record", "Comment2Record", "RecordFrom"]