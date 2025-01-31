from .sql import DatabaseManager
from ..util import database_config


db_path = database_config.path

db_manager = DatabaseManager(
        sync_db_url=f'sqlite:///{db_path}',  # 同步模式
        async_db_url=f'sqlite+aiosqlite:///{db_path}'  # 异步模式
)

__all__ = ["db_manager"]